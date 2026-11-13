//! Windows版の実行ファイルにアプリアイコンが埋め込まれていることを検証する。
//!
//! スタートメニューのショートカットはアイコンを個別に持たず、リンク先の実行ファイルの
//! アイコンを表示する。`build.rs` が作る `.res` が壊れていてもリンク自体は成功しうるため、
//! Windowsのシェルが実際にアイコンを取り出せるかで確認する。
#![cfg(windows)]

use std::os::windows::ffi::OsStrExt;
use windows_sys::Win32::UI::{
    Shell::ExtractIconExW,
    WindowsAndMessaging::{DestroyIcon, HICON},
};

#[test]
fn executable_embeds_the_application_icon() {
    let path: Vec<u16> = std::path::Path::new(env!("CARGO_BIN_EXE_skilvella"))
        .as_os_str()
        .encode_wide()
        .chain([0])
        .collect();
    // 第2引数に-1を渡すと、実行ファイル内のアイコングループ数を返す。
    let groups = unsafe {
        ExtractIconExW(
            path.as_ptr(),
            -1,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            0,
        )
    };
    assert_eq!(groups, 1);

    // 大小どちらの寸法でも、先頭のアイコンを実際に読み込めることを確認する。
    let mut large: HICON = std::ptr::null_mut();
    let mut small: HICON = std::ptr::null_mut();
    let extracted = unsafe { ExtractIconExW(path.as_ptr(), 0, &mut large, &mut small, 1) };
    assert!(extracted >= 1 && !large.is_null() && !small.is_null());
    unsafe {
        DestroyIcon(large);
        DestroyIcon(small);
    }
}
