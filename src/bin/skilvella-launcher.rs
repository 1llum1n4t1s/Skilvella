#![cfg_attr(windows, windows_subsystem = "windows")]

use std::{
    env, io,
    process::{Command, ExitCode, Stdio},
};

fn launch() -> io::Result<()> {
    let executable = env::current_exe()?;
    let directory = executable.parent().ok_or_else(|| {
        io::Error::new(io::ErrorKind::NotFound, "Launcher directory was not found")
    })?;
    let target = directory.join(if cfg!(windows) {
        "skilvella.exe"
    } else {
        "skilvella"
    });
    let mut command = Command::new(target);
    command
        .current_dir(directory)
        .args(env::args_os().skip(1))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // 親のコンソールを継承せず、コンソール用の子でもウィンドウを作らない。
        command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    command.spawn()?;
    Ok(())
}

fn main() -> ExitCode {
    match launch() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("Cannot start Skilvella: {error}");
            ExitCode::FAILURE
        }
    }
}
