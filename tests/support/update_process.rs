use std::{
    io,
    process::{Child, Command},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

#[cfg(windows)]
struct FixtureJob(windows_sys::Win32::Foundation::HANDLE);
#[cfg(windows)]
impl FixtureJob {
    fn close(&mut self) -> io::Result<()> {
        if !self.0.is_null() {
            if unsafe { windows_sys::Win32::Foundation::CloseHandle(self.0) } == 0 {
                return Err(io::Error::last_os_error());
            }
            self.0 = std::ptr::null_mut();
        }
        Ok(())
    }
}
#[cfg(windows)]
impl Drop for FixtureJob {
    fn drop(&mut self) {
        let _ = self.close();
    }
}

// 直接の子を回収するまで保持し、その子専用の隔離資源で再起動先も所有する。
pub struct FixtureHelper {
    pub(super) child: Child,
    stopped: bool,
    pub cleanup: Arc<Mutex<Option<io::Result<()>>>>,
    #[cfg(unix)]
    pub may_relaunch: bool,
    #[cfg(windows)]
    job: Option<FixtureJob>,
}
impl FixtureHelper {
    pub fn spawn(command: &mut Command) -> io::Result<Self> {
        #[cfg(windows)]
        let job = unsafe {
            use windows_sys::Win32::System::JobObjects::{
                CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
                JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
                SetInformationJobObject,
            };
            let handle = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if handle.is_null() {
                return Err(io::Error::last_os_error());
            }
            let job = FixtureJob(handle);
            let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            if SetInformationJobObject(
                handle,
                JobObjectExtendedLimitInformation,
                (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                std::mem::size_of_val(&limits) as u32,
            ) == 0
            {
                return Err(io::Error::last_os_error());
            }
            job
        };
        #[cfg(unix)]
        unsafe {
            use std::os::unix::process::CommandExt;
            let mut action: libc::sigaction = std::mem::zeroed();
            if libc::sigaction(libc::SIGCHLD, std::ptr::null(), &mut action) != 0 {
                return Err(io::Error::last_os_error());
            }
            if action.sa_sigaction != libc::SIG_DFL || action.sa_flags & libc::SA_NOCLDWAIT != 0 {
                return Err(io::Error::other(
                    "Fixture helper requires unreaped child ownership",
                ));
            }
            command.pre_exec(|| {
                if libc::setsid() < 0 {
                    return Err(io::Error::last_os_error());
                }
                Ok(())
            });
        }
        let child = command.spawn()?;
        #[cfg(windows)]
        {
            use std::os::windows::io::AsRawHandle;
            use windows_sys::Win32::System::JobObjects::AssignProcessToJobObject;
            let mut child = child;
            if unsafe { AssignProcessToJobObject(job.0, child.as_raw_handle()) } == 0 {
                let error = io::Error::last_os_error();
                let _ = child.kill();
                let _ = child.wait();
                return Err(error);
            }
            Ok(Self {
                child,
                stopped: false,
                cleanup: Arc::default(),
                job: Some(job),
            })
        }
        #[cfg(unix)]
        Ok(Self {
            child,
            stopped: false,
            cleanup: Arc::default(),
            may_relaunch: false,
        })
    }

    pub fn status(&mut self) -> io::Result<Option<bool>> {
        #[cfg(windows)]
        return self
            .child
            .try_wait()
            .map(|status| status.map(|status| status.success()));
        #[cfg(unix)]
        unsafe {
            self.observe(libc::WNOHANG)
        }
    }

    #[cfg(unix)]
    unsafe fn observe(&self, options: libc::c_int) -> io::Result<Option<bool>> {
        loop {
            let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
            if unsafe {
                libc::waitid(
                    libc::P_PID,
                    self.child.id(),
                    &mut info,
                    libc::WEXITED | libc::WNOWAIT | options,
                )
            } != 0
            {
                let error = io::Error::last_os_error();
                if error.raw_os_error() == Some(libc::EINTR) {
                    continue;
                }
                return Err(error);
            }
            let pid = unsafe { info.si_pid() };
            if pid == 0 {
                return Ok(None);
            }
            if pid != self.child.id() as libc::pid_t {
                return Err(io::Error::other("Unexpected fixture helper PID"));
            }
            return Ok(match info.si_code {
                libc::CLD_EXITED => Some(unsafe { info.si_status() } == 0),
                libc::CLD_KILLED | libc::CLD_DUMPED => Some(false),
                _ => None,
            });
        }
    }

    #[cfg(unix)]
    pub fn confirm_relaunch_exited(&mut self, pid: u32) -> io::Result<()> {
        // 通常のCI・VMでinitがzombieを回収する前提。回収しないPID1のコンテナでは
        // 終了後もgetpgidが残り、タイムアウトすることがある。
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            let group = unsafe { libc::getpgid(pid as libc::pid_t) };
            if group == -1 {
                let error = io::Error::last_os_error();
                if error.raw_os_error() != Some(libc::ESRCH) {
                    return Err(error);
                }
            }
            if group != self.child.id() as libc::pid_t {
                self.may_relaunch = false;
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "Relaunched fixture did not exit",
                ));
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }

    #[cfg(windows)]
    pub fn confirm_relaunch_exited(&mut self, _pid: u32) -> io::Result<()> {
        use windows_sys::Win32::System::JobObjects::{
            JOBOBJECT_BASIC_ACCOUNTING_INFORMATION, JobObjectBasicAccountingInformation,
            QueryInformationJobObject,
        };
        let job = self.job.as_ref().expect("Fixture job must remain owned");
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            let mut accounting: JOBOBJECT_BASIC_ACCOUNTING_INFORMATION =
                unsafe { std::mem::zeroed() };
            if unsafe {
                QueryInformationJobObject(
                    job.0,
                    JobObjectBasicAccountingInformation,
                    (&mut accounting as *mut JOBOBJECT_BASIC_ACCOUNTING_INFORMATION).cast(),
                    std::mem::size_of_val(&accounting) as u32,
                    std::ptr::null_mut(),
                )
            } == 0
            {
                return Err(io::Error::last_os_error());
            }
            if accounting.ActiveProcesses == 0 {
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "Relaunched fixture did not exit",
                ));
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }

    pub fn stop(&mut self) -> io::Result<()> {
        if self.stopped {
            return Ok(());
        }
        #[cfg(windows)]
        if let Some(job) = self.job.as_mut() {
            if unsafe { windows_sys::Win32::System::JobObjects::TerminateJobObject(job.0, 1) } == 0
            {
                return Err(io::Error::last_os_error());
            }
            job.close()?;
            self.job.take();
        }
        #[cfg(unix)]
        {
            // WNOWAITで所有を確認し、helperを先に止めて子の生成を終わらせる。
            if self.status()?.is_none()
                && let Err(error) = self.child.kill()
                && self.status()?.is_none()
            {
                return Err(error);
            }
            let deadline = Instant::now() + Duration::from_secs(30);
            while self.status()?.is_none() {
                if Instant::now() >= deadline {
                    return Err(io::Error::new(
                        io::ErrorKind::TimedOut,
                        "Fixture helper did not stop",
                    ));
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            if self.may_relaunch {
                let group = self.child.id() as libc::pid_t;
                if group <= 0 {
                    return Err(io::Error::other("Invalid fixture process group"));
                }
                if unsafe { libc::kill(-group, libc::SIGKILL) } != 0 {
                    let error = io::Error::last_os_error();
                    if error.raw_os_error() != Some(libc::ESRCH) {
                        return Err(error);
                    }
                }
            }
        }
        #[cfg(windows)]
        {
            let deadline = Instant::now() + Duration::from_secs(30);
            while self.status()?.is_none() {
                if Instant::now() >= deadline {
                    return Err(io::Error::new(
                        io::ErrorKind::TimedOut,
                        "Fixture helper did not stop",
                    ));
                }
                std::thread::sleep(Duration::from_millis(100));
            }
        }
        self.child.wait()?;
        self.stopped = true;
        Ok(())
    }
}
impl Drop for FixtureHelper {
    fn drop(&mut self) {
        let result = self.stop();
        *self
            .cleanup
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = Some(result);
    }
}
