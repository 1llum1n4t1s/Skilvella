use anyhow::{Context, bail};
use objc2::rc::{Retained, autoreleasepool};
use objc2::runtime::AnyObject;
use objc2::{AnyThread, DefinedClass, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSEventMask, NSImage, NSSquareStatusItemLength,
    NSStatusBar, NSWorkspace,
};
use objc2_foundation::{
    MainThreadMarker, NSData, NSDate, NSDefaultRunLoopMode, NSObject, NSSize, NSString, NSURL,
    ns_string,
};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

define_class!(
    // 安全性: NSObject に追加のサブクラス要件はなく、AppKit と同じメインスレッドだけで扱う。
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    #[ivars = Retained<NSURL>]
    struct ManagerTarget;

    impl ManagerTarget {
        // 安全性: target/action の sender を受け取る Objective-C のシグネチャに一致する。
        #[unsafe(method(openManager:))]
        fn open_manager(&self, _sender: &AnyObject) {
            if !NSWorkspace::sharedWorkspace().openURL(self.ivars()) {
                eprintln!("Could not open the Skilvella manager in the default browser");
            }
        }
    }
);

impl ManagerTarget {
    fn new(mtm: MainThreadMarker, url: Retained<NSURL>) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(url);
        // 安全性: NSObject の init を、初期化済みの ivars を持つ割当てに対して一度だけ呼ぶ。
        unsafe { msg_send![super(this), init] }
    }
}

pub fn run(url: &str, stopped: Arc<AtomicBool>) -> anyhow::Result<()> {
    let mtm = MainThreadMarker::new().context("The macOS status item needs the main thread")?;
    autoreleasepool(|_| {
        let url = NSURL::URLWithString(&NSString::from_str(url))
            .context("Could not create the manager URL")?;
        let data = NSData::with_bytes(include_bytes!("../../static/icons/icon-32.png"));
        let image = NSImage::initWithData(NSImage::alloc(), &data)
            .context("Could not decode the status item icon")?;
        image.setSize(NSSize::new(18.0, 18.0));

        let app = NSApplication::sharedApplication(mtm);
        if !app.setActivationPolicy(NSApplicationActivationPolicy::Accessory) {
            bail!("Could not configure the macOS status application");
        }
        app.finishLaunching();

        let target = ManagerTarget::new(mtm, url);
        let status_bar = NSStatusBar::systemStatusBar();
        let status_item = status_bar.statusItemWithLength(NSSquareStatusItemLength);
        let Some(button) = status_item.button(mtm) else {
            status_bar.removeStatusItem(&status_item);
            bail!("Could not create the macOS status item button");
        };
        button.setImage(Some(&image));
        button.setToolTip(Some(ns_string!("Skilvella 管理画面を開く")));
        button.sendActionOn(NSEventMask::LeftMouseUp);
        // 安全性: target は openManager: を実装し、イベント処理が終わるまで保持する。
        unsafe {
            button.setTarget(Some(&target));
            button.setAction(Some(sel!(openManager:)));
        }

        while !stopped.load(Ordering::Acquire) {
            // 各待機の一時オブジェクトを解放し、停止要求を最大約 100ms ごとに確認する。
            autoreleasepool(|_| {
                let deadline = NSDate::dateWithTimeIntervalSinceNow(0.1);
                // 安全性: Foundation が提供する不変の既定 run-loop mode を参照する。
                let mode = unsafe { NSDefaultRunLoopMode };
                if let Some(event) = app.nextEventMatchingMask_untilDate_inMode_dequeue(
                    NSEventMask::Any,
                    Some(&deadline),
                    mode,
                    true,
                ) {
                    app.sendEvent(&event);
                }
            });
        }

        // 安全性: 有効な action を解除してから、保持している target を解放する。
        unsafe {
            button.setAction(None);
            button.setTarget(None);
        }
        status_bar.removeStatusItem(&status_item);
        Ok(())
    })
}
