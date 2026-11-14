//! 管理画面の「参照」ボタンから開く、OS標準のフォルダー選択ダイアログ。
//!
//! 管理画面はローカルのブラウザーで表示するため、ブラウザーからはフォルダーの
//! 絶対パスを取得できない。代わりにSkilvella本体（同じPCで動くサーバー）が
//! OSのダイアログを開き、選ばれたパスを画面へ返す。登録自体は従来どおり
//! CSRF検査付きのフォームPOSTで行い、ここではパスを選ぶだけにとどめる。
//!
//! - Windows: `IFileOpenDialog`（`FOS_PICKFOLDERS`）。専用スレッドでSTAを初期化し、
//!   最前面の所有ウィンドウを付けてブラウザーの背後に隠れないようにする。
//! - macOS: `osascript` の `choose folder`。
//! - Linux: `zenity`、なければ `kdialog`。どちらも無ければ未対応として返す。
//!
//! デスクトップのない環境（SSH接続・ヘッドレス起動）では開けないため、画面側は
//! パスの直接入力へフォールバックする。

use anyhow::Result;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

/// ダイアログの結果。
#[derive(Debug)]
pub enum Picked {
    /// フォルダーが選ばれた。
    Folder(PathBuf),
    /// 利用者がキャンセルした。
    Cancelled,
    /// このOS・セッションではダイアログを開けない。
    Unsupported,
    /// 別のダイアログを表示中。
    Busy,
}

/// 同時に複数のダイアログを開かない（複数タブからの連打で画面が積み重なるのを防ぐ）。
static OPEN: AtomicBool = AtomicBool::new(false);

struct OpenGuard;
impl Drop for OpenGuard {
    fn drop(&mut self) {
        OPEN.store(false, Ordering::Release);
    }
}

/// サービスの終了・再起動時に、ダイアログの結果を待っているHTTP要求を解放する通知。
static RELEASE: tokio::sync::Notify = tokio::sync::Notify::const_new();

/// フォルダー選択ダイアログを開き、閉じるまで待つ（同期版）。
pub fn pick_folder(title: &str) -> Result<Picked> {
    if OPEN
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return Ok(Picked::Busy);
    }
    let _guard = OpenGuard;
    platform::pick_folder(title)
}

/// フォルダー選択ダイアログを開き、閉じるまで非同期に待つ。
///
/// ダイアログは利用者が閉じるまで戻らないため、`spawn_blocking` ではなく切り離した
/// 専用スレッドで開く。tokioのランタイムは終了時に `spawn_blocking` の完了を待つので、
/// そこで開くと終了・更新・再起動がダイアログを閉じるまで止まってしまう。
/// 終了時は [`release_waiters`] で待機中の要求を `Cancelled` として返し、
/// HTTPの正常終了（処理中の要求の完了待ち）も妨げない。
pub async fn pick_folder_async(title: String) -> Result<Picked> {
    let (sender, receiver) = tokio::sync::oneshot::channel();
    // 通知の登録をスレッド起動前に済ませ、直後の解放要求も取りこぼさない。
    let released = RELEASE.notified();
    tokio::pin!(released);
    released.as_mut().enable();
    std::thread::Builder::new()
        .name("skilvella-folder-picker".into())
        .spawn(move || {
            let _ = sender.send(pick_folder(&title));
        })?;
    tokio::select! {
        result = receiver => result.unwrap_or_else(|_| Err(anyhow::anyhow!("Folder dialog thread stopped"))),
        _ = released => Ok(Picked::Cancelled),
    }
}

/// サービスの終了・再起動の前に呼び、ダイアログの結果を待つHTTP要求をすべて返させる。
/// 開いているダイアログ自体は利用者が閉じるまで残る。Windowsでは同じプロセス内のスレッドなので
/// プロセスの終了とともに閉じるが、macOS・Linuxでは別プロセスとして起動した
/// `osascript`・`zenity`・`kdialog` が親の終了後も残ることがある。
pub fn release_waiters() {
    RELEASE.notify_waiters();
}

#[cfg(windows)]
mod platform {
    use super::Picked;
    use anyhow::{Context, Result};
    use std::os::windows::ffi::OsStringExt;
    use std::path::PathBuf;

    pub fn pick_folder(title: &str) -> Result<Picked> {
        let title = title.to_owned();
        // COMのSTAは呼び出しスレッドに結び付くため、tokioのスレッドプールを汚さないよう
        // 専用スレッドで初期化・終了する。
        std::thread::Builder::new()
            .name("skilvella-folder-dialog".into())
            .spawn(move || show(&title))
            .context("Cannot start folder dialog thread")?
            .join()
            .map_err(|_| anyhow::anyhow!("Folder dialog thread panicked"))?
    }

    fn show(title: &str) -> Result<Picked> {
        use windows::Win32::Foundation::{ERROR_CANCELLED, HWND};
        use windows::Win32::System::Com::{
            CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE,
            CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize,
        };
        use windows::Win32::UI::Shell::{
            FOS_FORCEFILESYSTEM, FOS_PATHMUSTEXIST, FOS_PICKFOLDERS, FileOpenDialog,
            IFileOpenDialog, SIGDN_FILESYSPATH,
        };
        use windows::core::{HRESULT, PCWSTR};

        struct Apartment;
        impl Drop for Apartment {
            fn drop(&mut self) {
                unsafe { CoUninitialize() };
            }
        }
        let wide_title: Vec<u16> = title.encode_utf16().chain([0]).collect();
        unsafe {
            CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE).ok()?;
            let _apartment = Apartment;
            let owner = Owner::new();
            let dialog: IFileOpenDialog =
                CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER)?;
            let options = dialog.GetOptions()?;
            dialog
                .SetOptions(options | FOS_PICKFOLDERS | FOS_FORCEFILESYSTEM | FOS_PATHMUSTEXIST)?;
            dialog.SetTitle(PCWSTR(wide_title.as_ptr()))?;
            let hwnd = owner.as_ref().map(|owner| HWND(owner.0 as _));
            if let Err(error) = dialog.Show(hwnd) {
                if error.code() == HRESULT::from_win32(ERROR_CANCELLED.0) {
                    return Ok(Picked::Cancelled);
                }
                return Err(error.into());
            }
            let item = dialog.GetResult()?;
            let name = item.GetDisplayName(SIGDN_FILESYSPATH)?;
            let path = PathBuf::from(std::ffi::OsString::from_wide(name.as_wide()));
            CoTaskMemFree(Some(name.0.cast()));
            Ok(Picked::Folder(path))
        }
    }

    /// ダイアログの所有者にする、画面中央の見えない最前面ウィンドウ。
    ///
    /// Skilvellaはブラウザーから操作される常駐プロセスで、前面に出る権限を持たない。
    /// 所有者なしで開くとブラウザーの背後やタスクバーの点滅だけになるため、
    /// 最前面の所有者を付けてダイアログも最前面に表示する。作れなくても所有者なしで続行する。
    struct Owner(windows_sys::Win32::Foundation::HWND);
    impl Owner {
        fn new() -> Option<Self> {
            use windows_sys::Win32::UI::WindowsAndMessaging::{
                CreateWindowExW, GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN, SetForegroundWindow,
                WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP, WS_VISIBLE,
            };
            let class: Vec<u16> = "STATIC".encode_utf16().chain([0]).collect();
            unsafe {
                let x = GetSystemMetrics(SM_CXSCREEN) / 2;
                let y = GetSystemMetrics(SM_CYSCREEN) / 2;
                let hwnd = CreateWindowExW(
                    WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
                    class.as_ptr(),
                    std::ptr::null(),
                    WS_POPUP | WS_VISIBLE,
                    x,
                    y,
                    0,
                    0,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null(),
                );
                if hwnd.is_null() {
                    return None;
                }
                // 前面化は許可されない場合もあるが、最前面の所有者で表示は確保できる。
                SetForegroundWindow(hwnd);
                Some(Self(hwnd))
            }
        }
    }
    impl Drop for Owner {
        fn drop(&mut self) {
            unsafe { windows_sys::Win32::UI::WindowsAndMessaging::DestroyWindow(self.0) };
        }
    }
}

#[cfg(target_os = "macos")]
mod platform {
    use super::Picked;
    use anyhow::{Result, bail};
    use std::path::PathBuf;
    use std::process::Command;

    pub fn pick_folder(title: &str) -> Result<Picked> {
        // AppleScriptの文字列に埋め込むため、引用符と円記号をエスケープする。
        let prompt = title.replace('\\', "\\\\").replace('"', "\\\"");
        let output = match Command::new("/usr/bin/osascript")
            .arg("-e")
            .arg("activate")
            .arg("-e")
            .arg(format!(
                "POSIX path of (choose folder with prompt \"{prompt}\")"
            ))
            .output()
        {
            Ok(output) => output,
            Err(_) => return Ok(Picked::Unsupported),
        };
        if output.status.success() {
            let path = String::from_utf8(output.stdout)?;
            let path = path.trim_end_matches(['\n', '\r']);
            // `POSIX path of` はフォルダー末尾に `/` を付けるため、ルート以外は取り除く。
            let path = if path.len() > 1 {
                path.trim_end_matches('/')
            } else {
                path
            };
            return Ok(Picked::Folder(PathBuf::from(path)));
        }
        let stderr = String::from_utf8_lossy(&output.stderr);
        // -128 は利用者のキャンセル。
        if stderr.contains("-128") {
            return Ok(Picked::Cancelled);
        }
        // GUIセッションが無い（SSH接続など）場合は画面側で直接入力へ切り替える。
        if stderr.contains("-1713") || stderr.contains("-10810") || stderr.contains("connection") {
            return Ok(Picked::Unsupported);
        }
        bail!("osascript failed: {}", stderr.trim())
    }
}

#[cfg(not(any(windows, target_os = "macos")))]
mod platform {
    use super::Picked;
    use anyhow::Result;
    use std::path::PathBuf;
    use std::process::Command;

    pub fn pick_folder(title: &str) -> Result<Picked> {
        // 表示先のディスプレイが無ければ、ダイアログ用のコマンドを起動しない。
        if std::env::var_os("DISPLAY").is_none() && std::env::var_os("WAYLAND_DISPLAY").is_none() {
            return Ok(Picked::Unsupported);
        }
        // kdialogは開始フォルダーの指定が必須のため、ホームを渡す。
        let home = std::env::var("HOME").unwrap_or_else(|_| "/".into());
        let candidates: [(&str, Vec<String>); 2] = [
            (
                "zenity",
                vec![
                    "--file-selection".into(),
                    "--directory".into(),
                    format!("--title={title}"),
                ],
            ),
            (
                "kdialog",
                vec![
                    "--getexistingdirectory".into(),
                    home,
                    "--title".into(),
                    title.into(),
                ],
            ),
        ];
        for (program, args) in candidates {
            let output = match Command::new(program).args(&args).output() {
                Ok(output) => output,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => return Err(error.into()),
            };
            // どちらも、キャンセル時は終了コード1で何も出力しない。
            if !output.status.success() {
                return Ok(Picked::Cancelled);
            }
            let path = String::from_utf8(output.stdout)?;
            let path = path.trim_end_matches(['\n', '\r']);
            if path.is_empty() {
                return Ok(Picked::Cancelled);
            }
            return Ok(Picked::Folder(PathBuf::from(path)));
        }
        Ok(Picked::Unsupported)
    }
}
