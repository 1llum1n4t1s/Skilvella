use std::cell::Cell;
use std::mem::size_of;
use std::ptr::{null, null_mut};
use std::time::Instant;

use crate::service::Control;
use anyhow::{Context, Result, bail};
use windows_sys::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows_sys::Win32::System::Com::{
    COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE, CoInitializeEx, CoUninitialize,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::GetDoubleClickTime;
use windows_sys::Win32::UI::Shell::{
    NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW, Shell_NotifyIconW,
    ShellExecuteW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::*;

const ICON_MESSAGE: u32 = WM_APP + 1;
const STOP_TIMER: usize = 1;
const OPEN_MANAGER: usize = 1;
const RESTART_SERVICE: usize = 2;
const QUIT: usize = 3;
const CHECK_UPDATES: usize = 4;

struct ComApartment;

impl Drop for ComApartment {
    fn drop(&mut self) {
        // 初期化に成功した同じスレッドで、対応する参照を一度だけ解放する。
        unsafe { CoUninitialize() };
    }
}

struct Icon(HICON);

impl Drop for Icon {
    fn drop(&mut self) {
        // CreateIconFromResourceExで所有したアイコンだけを解放する。
        unsafe { DestroyIcon(self.0) };
    }
}

struct WindowClass {
    name: Vec<u16>,
    instance: HINSTANCE,
    registered: bool,
}

impl Drop for WindowClass {
    fn drop(&mut self) {
        // このクラスのウィンドウは先に破棄され、クラス名もまだ有効。
        if self.registered {
            unsafe { UnregisterClassW(self.name.as_ptr(), self.instance) };
        }
    }
}

struct TrayState {
    icon: Icon,
    url: Vec<u16>,
    updates_url: Vec<u16>,
    control: Control,
    taskbar_created: u32,
    last_click: Cell<Option<Instant>>,
}

impl TrayState {
    fn open_manager(&self, window: HWND) {
        self.open_url(window, &self.url);
    }

    fn open_url(&self, window: HWND, url: &[u16]) {
        // URLはnull終端され、同期呼出しの間はstateが所有する。
        let result =
            unsafe { ShellExecuteW(window, null(), url.as_ptr(), null(), null(), SW_SHOWNORMAL) };
        if result as isize <= 32 {
            eprintln!(
                "管理画面をブラウザーで開けませんでした（Windowsコード: {}）",
                result as isize
            );
        }
    }

    fn show_menu(&self, window: HWND) -> Result<()> {
        // メニューはこのスレッドが所有し、選択・取消のどちらでも解放する。
        let menu = Menu(unsafe { CreatePopupMenu() });
        if menu.0.is_null() {
            return Err(std::io::Error::last_os_error())
                .context("トレイメニューを作成できませんでした");
        }
        let restarting = self.control.is_restarting();
        let restart_label = if restarting {
            "サービスを再起動中…"
        } else {
            "サービスを再起動"
        };
        for (id, label, flags) in [
            (OPEN_MANAGER, "管理画面を開く", MF_STRING),
            (CHECK_UPDATES, "更新を確認", MF_STRING),
            (
                RESTART_SERVICE,
                restart_label,
                MF_STRING | if restarting { MF_GRAYED } else { 0 },
            ),
            (0, "", MF_SEPARATOR),
            (QUIT, "終了", MF_STRING),
        ] {
            if unsafe { AppendMenuW(menu.0, flags, id, wide(label).as_ptr()) } == 0 {
                return Err(std::io::Error::last_os_error())
                    .context("トレイメニュー項目を作成できませんでした");
            }
        }
        let mut position = POINT::default();
        if unsafe { GetCursorPos(&mut position) } == 0 {
            return Err(std::io::Error::last_os_error())
                .context("メニューの表示位置を取得できませんでした");
        }
        // 通知アイコンのメニューは所有ウィンドウを前面にし、外側クリックで閉じられるようにする。
        let selected = unsafe {
            SetForegroundWindow(window);
            let selected = TrackPopupMenu(
                menu.0,
                TPM_RETURNCMD | TPM_NONOTIFY | TPM_RIGHTBUTTON,
                position.x,
                position.y,
                0,
                window,
                null(),
            );
            PostMessageW(window, WM_NULL, 0, 0);
            selected as usize
        };
        match selected {
            OPEN_MANAGER => self.open_manager(window),
            CHECK_UPDATES => self.open_url(window, &self.updates_url),
            RESTART_SERVICE => self.control.restart(),
            QUIT => self.control.quit(),
            _ => {}
        }
        Ok(())
    }

    fn notification(&self, window: HWND) -> NOTIFYICONDATAW {
        let mut data = NOTIFYICONDATAW {
            cbSize: size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: window,
            uID: 1,
            uFlags: NIF_ICON | NIF_MESSAGE | NIF_TIP,
            uCallbackMessage: ICON_MESSAGE,
            hIcon: self.icon.0,
            ..Default::default()
        };
        for (target, value) in data
            .szTip
            .iter_mut()
            .take(127)
            .zip("Skilvella\n".encode_utf16().chain(self.url.iter().copied()))
        {
            *target = value;
        }
        data
    }

    fn add_icon(&self, window: HWND) -> Result<()> {
        // 通知構造体とアイコンは同期API呼出しの間、有効なまま保持する。
        if unsafe { Shell_NotifyIconW(NIM_ADD, &self.notification(window)) } == 0 {
            bail!("Windowsのタスクトレイにアイコンを登録できませんでした");
        }
        Ok(())
    }
}

struct Menu(HMENU);

impl Drop for Menu {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { DestroyMenu(self.0) };
        }
    }
}

struct TrayWindow<'a> {
    handle: HWND,
    state: &'a TrayState,
}

impl Drop for TrayWindow<'_> {
    fn drop(&mut self) {
        // 状態・アイコン・ウィンドウクラスより先に通知とウィンドウを破棄する。
        unsafe {
            Shell_NotifyIconW(NIM_DELETE, &self.state.notification(self.handle));
            KillTimer(self.handle, STOP_TIMER);
            DestroyWindow(self.handle);
        }
    }
}

pub fn run(url: &str, control: Control) -> Result<()> {
    if control.is_stopped() {
        return Ok(());
    }
    // ShellExecuteWがCOMを使うURLハンドラーにも対応できるようSTAを用意する。
    let result = unsafe {
        CoInitializeEx(
            null(),
            (COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) as u32,
        )
    };
    if result < 0 {
        bail!("ブラウザー起動用のCOMを初期化できませんでした: {result:#x}");
    }
    let _com = ComApartment;
    // null指定は現在の実行ファイルを表し、ハンドルの解放は不要。
    let instance = unsafe { GetModuleHandleW(null()) };
    if instance.is_null() {
        return Err(std::io::Error::last_os_error()).context("実行ファイルを取得できませんでした");
    }
    let mut class = WindowClass {
        name: wide("SkilvellaTrayWindow"),
        instance,
        registered: false,
    };
    let definition = WNDCLASSW {
        lpfnWndProc: Some(window_proc),
        hInstance: instance,
        lpszClassName: class.name.as_ptr(),
        ..Default::default()
    };
    // クラス名とコールバックは登録中有効で、ウィンドウ生成前には状態を参照しない。
    if unsafe { RegisterClassW(&definition) } == 0 {
        return Err(std::io::Error::last_os_error())
            .context("タスクトレイのウィンドウクラスを登録できませんでした");
    }
    class.registered = true;
    let taskbar_message = wide("TaskbarCreated");
    // Explorerが通知する登録メッセージ名はnull終端されている。
    let taskbar_created = unsafe { RegisterWindowMessageW(taskbar_message.as_ptr()) };
    if taskbar_created == 0 {
        return Err(std::io::Error::last_os_error())
            .context("タスクバーの通知を登録できませんでした");
    }
    let state = Box::new(TrayState {
        icon: load_icon()?,
        url: wide(url),
        updates_url: wide(&format!("{url}settings?check_updates=1#updates")),
        control,
        taskbar_created,
        last_click: Cell::new(None),
    });
    // Boxのアドレスはウィンドウ破棄まで不変。非表示のトップレベルウィンドウで
    // TaskbarCreatedのブロードキャストも受け取る。
    let handle = unsafe {
        CreateWindowExW(
            0,
            class.name.as_ptr(),
            class.name.as_ptr(),
            0,
            0,
            0,
            0,
            0,
            null_mut(),
            null_mut(),
            instance,
            (&*state as *const TrayState).cast(),
        )
    };
    if handle.is_null() {
        return Err(std::io::Error::last_os_error())
            .context("タスクトレイのウィンドウを作成できませんでした");
    }
    let window = TrayWindow {
        handle,
        state: &state,
    };
    state.add_icon(window.handle)?;
    // コールバックなしのタイマーは同じスレッドのメッセージループで処理される。
    if unsafe { SetTimer(window.handle, STOP_TIMER, 200, None) } == 0 {
        return Err(std::io::Error::last_os_error())
            .context("終了監視タイマーを開始できませんでした");
    }
    let mut message = MSG::default();
    loop {
        // MSGは有効な書込み先で、登録済みウィンドウのメッセージを同じスレッドで配送する。
        let result = unsafe { GetMessageW(&mut message, null_mut(), 0, 0) };
        if result == -1 {
            return Err(std::io::Error::last_os_error())
                .context("タスクトレイの通知を取得できませんでした");
        }
        if result == 0 {
            return Ok(());
        }
        unsafe {
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
}

unsafe extern "system" fn window_proc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    // WM_NCCREATEのCREATESTRUCTWはOS提供。保存するBoxの参照先はrunが
    // DestroyWindow完了まで所有し、このスレッド以外から参照しない。
    unsafe {
        if message == WM_NCCREATE {
            let creation = &*(lparam as *const CREATESTRUCTW);
            SetWindowLongPtrW(window, GWLP_USERDATA, creation.lpCreateParams as isize);
        }
        let pointer = GetWindowLongPtrW(window, GWLP_USERDATA) as *const TrayState;
        if !pointer.is_null() {
            let state = &*pointer;
            if message == state.taskbar_created {
                if let Err(error) = state.add_icon(window) {
                    eprintln!("{error:#}");
                }
                return 0;
            }
            match message {
                ICON_MESSAGE if lparam as u32 == WM_LBUTTONUP => {
                    let now = Instant::now();
                    let interval = u128::from(GetDoubleClickTime());
                    // 左ボタンを離した通知だけで開き、ダブルクリックの後半もまとめる。
                    if state
                        .last_click
                        .get()
                        .is_none_or(|last| now.duration_since(last).as_millis() > interval)
                    {
                        state.last_click.set(Some(now));
                        state.open_manager(window);
                    }
                    return 0;
                }
                ICON_MESSAGE if matches!(lparam as u32, WM_RBUTTONUP | WM_CONTEXTMENU) => {
                    if let Err(error) = state.show_menu(window) {
                        eprintln!("{error:#}");
                    }
                    return 0;
                }
                WM_TIMER if wparam == STOP_TIMER => {
                    if state.control.is_stopped() {
                        EndMenu();
                        PostQuitMessage(0);
                    }
                    return 0;
                }
                WM_CLOSE => {
                    state.control.quit();
                    return 0;
                }
                WM_NCDESTROY => {
                    SetWindowLongPtrW(window, GWLP_USERDATA, 0);
                }
                _ => {}
            }
        }
        DefWindowProcW(window, message, wparam, lparam)
    }
}

fn load_icon() -> Result<Icon> {
    let bytes = include_bytes!("../../static/icons/skilvella.ico");
    // ICOのディレクトリから小アイコンの表示寸法に最も近い画像を選ぶ。
    let count = u16::from_le_bytes([bytes[4], bytes[5]]) as usize;
    // システムメトリックの取得はポインターを参照しない。
    let width = unsafe { GetSystemMetrics(SM_CXSMICON) }.max(1);
    let height = unsafe { GetSystemMetrics(SM_CYSMICON) }.max(1);
    let entry = bytes[6..]
        .as_chunks::<16>()
        .0
        .iter()
        .take(count)
        .min_by_key(|entry| {
            let size = if entry[0] == 0 {
                256
            } else {
                i32::from(entry[0])
            };
            (size - width).abs()
        })
        .context("同梱アイコンに画像がありません")?;
    let length = u32::from_le_bytes(entry[8..12].try_into()?) as usize;
    let offset = u32::from_le_bytes(entry[12..16].try_into()?) as usize;
    let image = bytes
        .get(
            offset
                ..offset
                    .checked_add(length)
                    .context("同梱アイコンのサイズが不正です")?,
        )
        .context("同梱アイコンの画像範囲が不正です")?;
    // ICO内の検査済み画像範囲を渡す。LR_SHAREDを使わず所有権を取得する。
    let icon = unsafe {
        CreateIconFromResourceEx(
            image.as_ptr(),
            length as u32,
            1,
            0x0003_0000,
            width,
            height,
            0,
        )
    };
    if icon.is_null() {
        return Err(std::io::Error::last_os_error()).context("同梱アイコンを読み込めませんでした");
    }
    Ok(Icon(icon))
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}
