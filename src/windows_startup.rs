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
// 検証用の `update-feed-test` ビルドは、Setupのフックが利用者の実際の自動起動（ZIP版の登録など）を
// 置き換えないよう、別の値名を使う。取り込み・置き換えの判定そのものは隔離ハイブの単体テストで確かめる。
#[cfg(not(feature = "update-feed-test"))]
const NAME: &str = "Skilvella";
#[cfg(feature = "update-feed-test")]
const NAME: &str = "Skilvella update-feed-test";

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
    write_run_value(key.0, executable)
}

/// Setup版（Velopack）のインストール直後に、サインイン時の自動起動をSetup版へ向ける。
///
/// 目的は単体版（ZIP版）からの移行。ZIP版は同じ値名 `Skilvella` で自分を登録しており、
/// 残したままだとサインイン時に旧版が先に起動して既定のポートを取り、Setup版が起動できなくなる。
/// - 値が無い: Setup版を登録する（Setup版もPC再起動後にMCPを提供できるようにする）。
/// - 値が `skilvella.exe` を指す（ZIP版・旧Setup版・Toolport経由など、同じアプリの別配置）: Setup版へ置き換える。
/// - それ以外のプログラムを指す: 利用者の設定として残す。
///
/// 利用者がタスクマネージャー等で自動起動を無効にした状態は `StartupApproved` 側に値名で記録されるため、
/// ここで Run 値を置き換えても無効のまま保たれる。
#[cfg_attr(not(feature = "windows-installer"), allow(dead_code))]
pub fn take_over_at_logon(executable: &Path) -> Result<()> {
    take_over_run_value(HKEY_CURRENT_USER, executable)
}

/// Setup版のアンインストール直前に、サインイン時の自動起動を外す。Setup版自身を指す場合だけ削除し、
/// 利用者が別の配置（ZIP版など）へ戻した登録は残す。
#[cfg_attr(not(feature = "windows-installer"), allow(dead_code))]
pub fn remove_at_logon_if_ours(executable: &Path) -> Result<()> {
    remove_run_value_if_ours(HKEY_CURRENT_USER, executable)
}

/// Setup版がポートを使えずに起動できなかったことを、メッセージボックスで知らせる。
/// 多くは単体版（ZIP版）が動いたままの場合なので、旧版の終了を案内する。
#[cfg(feature = "windows-installer")]
pub fn show_port_in_use(port: u16) {
    use skilvella::locale;
    use windows_sys::Win32::UI::WindowsAndMessaging::{MB_ICONWARNING, MB_OK, MessageBoxW};
    // 日英併記をやめ、現在の言語（SKILVELLA_LANGUAGE → OSの表示言語など）の1言語だけで案内する。
    let text = wide(&format!(
        "{}\n{}",
        locale::format(
            "ポート {port} が使用中のため、Skilvella を起動できませんでした。",
            &[("port", &port.to_string())]
        ),
        locale::text(
            "別の Skilvella（ZIP版など）が動いている可能性があります。タスクトレイのアイコンから終了してから、もう一度起動してください。"
        )
    ));
    let title = wide("Skilvella");
    unsafe {
        MessageBoxW(
            null_mut(),
            text.as_ptr(),
            title.as_ptr(),
            MB_OK | MB_ICONWARNING,
        )
    };
}

/// Run 値（REG_SZ）を読む。値が無ければ `None`。
fn read_run_value(key: HKEY) -> Result<Option<String>> {
    let name = wide(NAME);
    let mut kind = 0;
    let mut bytes = 0_u32;
    match unsafe {
        RegQueryValueExW(
            key,
            name.as_ptr(),
            std::ptr::null(),
            &mut kind,
            null_mut(),
            &mut bytes,
        )
    } {
        ERROR_SUCCESS | ERROR_MORE_DATA => {}
        ERROR_FILE_NOT_FOUND => return Ok(None),
        error => return Err(io::Error::from_raw_os_error(error as i32)).context("Read Run value"),
    }
    // 文字列以外（REG_EXPAND_SZ を含む）は自分の書いた形式ではないため、中身を解釈しない。
    if kind != REG_SZ {
        return Ok(Some(String::new()));
    }
    let mut buffer = vec![0_u16; (bytes as usize).div_ceil(2) + 1];
    let mut size = size_of_val(buffer.as_slice()) as u32;
    check(unsafe {
        RegQueryValueExW(
            key,
            name.as_ptr(),
            std::ptr::null(),
            null_mut(),
            buffer.as_mut_ptr().cast(),
            &mut size,
        )
    })
    .context("Read Run value")?;
    let end = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
    Ok(Some(String::from_utf16_lossy(&buffer[..end])))
}

/// 起動コマンドの先頭にある実行ファイルのパスを取り出す（引用符付き・無しの両方）。
fn command_executable(command: &str) -> Option<PathBuf> {
    let command = command.trim_start();
    let path = if let Some(rest) = command.strip_prefix('"') {
        &rest[..rest.find('"')?]
    } else {
        // 引用符が無い場合は、最初の空白までを実行ファイルとみなす（Windowsの解釈と同じく曖昧なため最小限）。
        command.split_whitespace().next()?
    };
    (!path.is_empty()).then(|| PathBuf::from(path))
}

/// Windowsのパスとして同じか（大文字小文字・区切り文字の違いを無視する）。
fn same_path(left: &Path, right: &Path) -> bool {
    let normalize = |path: &Path| path.to_string_lossy().replace('/', "\\").to_lowercase();
    normalize(left) == normalize(right)
}

fn take_over_run_value(root: HKEY, executable: &Path) -> Result<()> {
    let key = RunKey::open(root)?;
    if let Some(current) = read_run_value(key.0)? {
        match command_executable(&current) {
            // 既にこの実行ファイルを指していれば何もしない（引数を含め書き直さない）。
            Some(path) if same_path(&path, executable) => return Ok(()),
            // 同じアプリの別配置だけを引き継ぐ。
            Some(path)
                if path
                    .file_name()
                    .is_some_and(|name| name.eq_ignore_ascii_case("skilvella.exe")) => {}
            _ => return Ok(()),
        }
    }
    write_run_value(key.0, executable)
}

fn remove_run_value_if_ours(root: HKEY, executable: &Path) -> Result<()> {
    use windows_sys::Win32::System::Registry::RegDeleteValueW;
    let key = RunKey::open(root)?;
    let Some(current) = read_run_value(key.0)? else {
        return Ok(());
    };
    if !command_executable(&current).is_some_and(|path| same_path(&path, executable)) {
        return Ok(());
    }
    match unsafe { RegDeleteValueW(key.0, wide(NAME).as_ptr()) } {
        ERROR_SUCCESS | ERROR_FILE_NOT_FOUND => Ok(()),
        error => Err(io::Error::from_raw_os_error(error as i32)).context("Delete Run value"),
    }
}

fn write_run_value(key: HKEY, executable: &Path) -> Result<()> {
    let name = wide(NAME);
    let command = startup_command(executable)?;
    check(unsafe {
        RegSetValueExW(
            key,
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

    /// ファイルに閉じた専用ハイブを開く。実際のユーザーのRunキーには触れない。
    fn private_hive(label: &str) -> Result<RunKey> {
        let directory = env::var_os("SKILVELLA_MIGRATION_TEST_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(env::temp_dir)
            .join(format!("{label}-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&directory)?;
        let file: Vec<_> = directory
            .join("user.hiv")
            .as_os_str()
            .encode_wide()
            .chain([0])
            .collect();
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
        Ok(RunKey(root))
    }

    fn set_raw(root: HKEY, command: &str) -> Result<()> {
        let key = RunKey::open(root)?;
        let value = wide(command);
        check(unsafe {
            RegSetValueExW(
                key.0,
                wide(NAME).as_ptr(),
                0,
                REG_SZ,
                value.as_ptr().cast(),
                size_of_val(value.as_slice()) as u32,
            )
        })?;
        Ok(())
    }

    fn get_raw(root: HKEY) -> Result<Option<String>> {
        read_run_value(RunKey::open(root)?.0)
    }

    #[test]
    fn setup_takes_over_the_zip_edition_and_keeps_other_programs() -> Result<()> {
        let setup = Path::new(r"C:\Users\利用者\AppData\Local\SkilvellaApp\current\skilvella.exe");
        let expected =
            r#""C:\Users\利用者\AppData\Local\SkilvellaApp\current\skilvella.exe" --no-autostart"#;

        // 値が無ければ登録する（Setup版も再起動後に起動する）。
        let root = private_hive("takeover-empty")?;
        take_over_run_value(root.0, setup)?;
        assert_eq!(get_raw(root.0)?.as_deref(), Some(expected));

        // ZIP版（引用符付き・無しの両方）の登録はSetup版へ置き換える。
        for zip in [
            r#""C:\Users\利用者\AppData\Local\Skilvella\bin\skilvella.exe" --no-autostart"#,
            r"D:\Tools\SKILVELLA.EXE --no-autostart",
        ] {
            let root = private_hive("takeover-zip")?;
            set_raw(root.0, zip)?;
            take_over_run_value(root.0, setup)?;
            assert_eq!(get_raw(root.0)?.as_deref(), Some(expected));
        }

        // 同名でも別のプログラムを指す登録と、既に自分を指す登録（引数違いも含む）は書き換えない。
        for kept in [
            r#""D:\Other\launcher.exe" skilvella"#,
            r#""c:/users/利用者/appdata/local/skilvellaapp/current/SKILVELLA.exe" --custom"#,
            "",
        ] {
            let root = private_hive("takeover-keep")?;
            set_raw(root.0, kept)?;
            take_over_run_value(root.0, setup)?;
            assert_eq!(get_raw(root.0)?.as_deref(), Some(kept));
        }
        Ok(())
    }

    #[test]
    fn uninstall_removes_only_its_own_logon_entry() -> Result<()> {
        let setup = Path::new(r"C:\Users\利用者\AppData\Local\SkilvellaApp\current\skilvella.exe");

        let root = private_hive("uninstall-ours")?;
        take_over_run_value(root.0, setup)?;
        remove_run_value_if_ours(root.0, setup)?;
        assert_eq!(get_raw(root.0)?, None);
        // 値が無い状態での再実行も成功する。
        remove_run_value_if_ours(root.0, setup)?;

        // 利用者がZIP版へ戻した登録は残す。
        let zip = r#""C:\Users\利用者\AppData\Local\Skilvella\bin\skilvella.exe" --no-autostart"#;
        let root = private_hive("uninstall-zip")?;
        set_raw(root.0, zip)?;
        remove_run_value_if_ours(root.0, setup)?;
        assert_eq!(get_raw(root.0)?.as_deref(), Some(zip));
        Ok(())
    }

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
