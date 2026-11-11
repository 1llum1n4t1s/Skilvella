use anyhow::{Context, Result, ensure};
use std::{
    env, io,
    mem::size_of_val,
    os::windows::ffi::{OsStrExt, OsStringExt},
    path::{Path, PathBuf},
    ptr::null_mut,
};
use windows_sys::Win32::{
    Foundation::{ERROR_FILE_NOT_FOUND, ERROR_MORE_DATA, ERROR_SUCCESS, INVALID_HANDLE_VALUE},
    System::{
        Console::{
            ATTACH_PARENT_PROCESS, AttachConsole, GetStdHandle, STD_ERROR_HANDLE, STD_OUTPUT_HANDLE,
        },
        Registry::{
            HKEY, HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE, REG_OPTION_NON_VOLATILE,
            REG_SZ, RegCloseKey, RegCreateKeyExW, RegQueryValueExW, RegSetValueExW,
        },
    },
};

pub const NO_AUTOSTART: &str = "--no-autostart";
const RUN: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const NAME: &str = "Skilvella";

pub fn attach_parent_console() {
    // Explorerからの起動ではコンソールを作らない。CLIの既存の出力先も保持する。
    unsafe {
        if [STD_OUTPUT_HANDLE, STD_ERROR_HANDLE]
            .into_iter()
            .all(|which| {
                let handle = GetStdHandle(which);
                handle.is_null() || handle == INVALID_HANDLE_VALUE
            })
        {
            AttachConsole(ATTACH_PARENT_PROCESS);
        }
    }
}

pub fn register_at_logon() -> Result<()> {
    let executable = env::current_exe()?;
    // Toolportの版別キャッシュを直接登録すると更新後も旧版を起動してしまう。
    if skilvella::update::is_toolport_cache_executable(&executable) {
        return Ok(());
    }
    ensure_run_value(HKEY_CURRENT_USER, &executable)
}

pub fn register_start_menu() -> Result<()> {
    use windows::Win32::System::Com::CoTaskMemFree;
    use windows::Win32::UI::Shell::{FOLDERID_Programs, KF_FLAG_DEFAULT, SHGetKnownFolderPath};

    let executable = env::current_exe()?;
    // 自動更新で実体が切り替わるToolportの版別キャッシュは固定の起動先にしない。
    if skilvella::update::is_toolport_cache_executable(&executable) {
        return Ok(());
    }
    let programs = unsafe { SHGetKnownFolderPath(&FOLDERID_Programs, KF_FLAG_DEFAULT, None) }?;
    let directory = unsafe {
        let directory = PathBuf::from(std::ffi::OsString::from_wide(programs.as_wide()));
        CoTaskMemFree(Some(programs.0.cast()));
        directory
    };
    create_program_shortcut(&directory.join("Skilvella.lnk"), &executable)
}

fn create_program_shortcut(shortcut: &Path, executable: &Path) -> Result<()> {
    use windows::{
        Win32::{
            System::Com::{
                CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
                CoUninitialize, IPersistFile,
            },
            UI::Shell::{IShellLinkW, ShellLink},
        },
        core::{Interface, PCWSTR},
    };
    // 既存のショートカットや同名の項目は利用者の設定として扱う。
    match std::fs::symlink_metadata(shortcut) {
        Ok(_) => return Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    struct Apartment;
    impl Drop for Apartment {
        fn drop(&mut self) {
            unsafe { CoUninitialize() };
        }
    }
    let parent = executable
        .parent()
        .context("Executable directory missing")?;
    let wide_path =
        |path: &Path| -> Vec<u16> { path.as_os_str().encode_wide().chain([0]).collect() };
    let target = wide_path(executable);
    let working_dir = wide_path(parent);
    let filename = wide_path(shortcut);
    // COMの参照を解放してから、同じスレッドでCOMを終了する。
    unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()?;
        let _apartment = Apartment;
        let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)?;
        link.SetPath(PCWSTR(target.as_ptr()))?;
        link.SetWorkingDirectory(PCWSTR(working_dir.as_ptr()))?;
        link.cast::<IPersistFile>()?
            .Save(PCWSTR(filename.as_ptr()), true)?;
    }
    Ok(())
}

fn ensure_run_value(root: HKEY, executable: &Path) -> Result<()> {
    let key = RunKey::open(root)?;
    let name = wide(NAME);
    match unsafe {
        RegQueryValueExW(
            key.0,
            name.as_ptr(),
            std::ptr::null(),
            null_mut(),
            null_mut(),
            null_mut(),
        )
    } {
        // Windowsのスタートアップ設定や既存の登録内容を尊重する。
        ERROR_SUCCESS | ERROR_MORE_DATA => return Ok(()),
        ERROR_FILE_NOT_FOUND => {}
        error => return Err(io::Error::from_raw_os_error(error as i32)).context("Read Run value"),
    }

    let command = startup_command(executable)?;
    check(unsafe {
        RegSetValueExW(
            key.0,
            name.as_ptr(),
            0,
            REG_SZ,
            command.as_ptr().cast(),
            size_of_val(command.as_slice()) as u32,
        )
    })
    .context("Write Run value")
}

fn startup_command(executable: &Path) -> Result<Vec<u16>> {
    // REG_SZを使い、環境変数やシェルを介さず、空白・非ASCIIを含む絶対パスを渡す。
    let path = executable.as_os_str().encode_wide();
    let command: Vec<_> = "\""
        .encode_utf16()
        .chain(path)
        .chain("\" ".encode_utf16())
        .chain(NO_AUTOSTART.encode_utf16())
        .chain([0])
        .collect();
    ensure!(
        executable.is_absolute() && command.len() <= 261,
        "Invalid or too long startup command"
    );
    Ok(command)
}

struct RunKey(HKEY);

impl RunKey {
    fn open(root: HKEY) -> Result<Self> {
        let mut key = null_mut();
        check(unsafe {
            RegCreateKeyExW(
                root,
                wide(RUN).as_ptr(),
                0,
                std::ptr::null(),
                REG_OPTION_NON_VOLATILE,
                KEY_QUERY_VALUE | KEY_SET_VALUE,
                std::ptr::null(),
                &mut key,
                null_mut(),
            )
        })
        .context("Open the current user's Run key")?;
        Ok(Self(key))
    }
}

impl Drop for RunKey {
    fn drop(&mut self) {
        unsafe { RegCloseKey(self.0) };
    }
}

fn check(result: u32) -> io::Result<()> {
    match result {
        ERROR_SUCCESS => Ok(()),
        error => Err(io::Error::from_raw_os_error(error as i32)),
    }
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain([0]).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_menu_shortcut_preserves_an_existing_entry() -> Result<()> {
        let directory = env::var_os("SKILVELLA_MIGRATION_TEST_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(env::temp_dir)
            .join(format!("start-menu-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&directory)?;
        let shortcut = directory.join("Skilvella.lnk");
        create_program_shortcut(&shortcut, &env::current_exe()?)?;
        let original = std::fs::read(&shortcut)?;
        // Shell Link Headerのサイズは0x4c。COMがリンクを保存したことを確認する。
        assert!(original.starts_with(&[0x4c, 0, 0, 0]));
        create_program_shortcut(&shortcut, Path::new(r"D:\別の配置\skilvella.exe"))?;
        assert_eq!(std::fs::read(shortcut)?, original);
        Ok(())
    }
    use windows_sys::Win32::System::Registry::{
        KEY_ALL_ACCESS, REG_PROCESS_APPKEY, RegLoadAppKeyW,
    };

    #[test]
    fn logon_runs_the_same_executable_without_registering_again() {
        let executable = Path::new(r"D:\アプリ フォルダー\skilvella.exe");
        assert_eq!(
            startup_command(executable).unwrap(),
            wide(r#""D:\アプリ フォルダー\skilvella.exe" --no-autostart"#),
        );
        assert!(startup_command(Path::new("skilvella.exe")).is_err());
    }

    #[test]
    fn logon_registration_creates_value_and_preserves_existing_entry() -> Result<()> {
        let directory = env::var_os("SKILVELLA_MIGRATION_TEST_ROOT")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(env::temp_dir)
            .join(format!("startup-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&directory)?;
        let file: Vec<_> = directory
            .join("user.hiv")
            .as_os_str()
            .encode_wide()
            .chain([0])
            .collect();
        // ファイルに閉じた専用ハイブで、実際のユーザーのRunキーを変更せずに検証する。
        let mut root = null_mut();
        check(unsafe {
            RegLoadAppKeyW(
                file.as_ptr(),
                &mut root,
                KEY_ALL_ACCESS,
                REG_PROCESS_APPKEY,
                0,
            )
        })?;
        let root = RunKey(root);
        let executable = Path::new(r"D:\アプリ フォルダー\skilvella.exe");
        ensure_run_value(root.0, executable)?;
        ensure_run_value(root.0, Path::new(r"D:\別の配置\skilvella.exe"))?;

        let key = RunKey::open(root.0)?;
        let expected = startup_command(executable)?;
        let mut actual = vec![0_u16; expected.len()];
        let mut bytes = size_of_val(actual.as_slice()) as u32;
        let mut kind = 0;
        check(unsafe {
            RegQueryValueExW(
                key.0,
                wide(NAME).as_ptr(),
                std::ptr::null(),
                &mut kind,
                actual.as_mut_ptr().cast(),
                &mut bytes,
            )
        })?;
        assert_eq!(kind, REG_SZ);
        assert_eq!(bytes as usize, size_of_val(expected.as_slice()));
        assert_eq!(actual, expected);
        Ok(())
    }
}
