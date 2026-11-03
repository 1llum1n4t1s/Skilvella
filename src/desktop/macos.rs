use crate::service::Control;
use anyhow::{Context, bail};
use objc2::rc::{Retained, autoreleasepool};
use objc2::runtime::AnyObject;
use objc2::{AnyThread, DefinedClass, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSEventMask, NSEventModifierFlags,
    NSEventTrackingRunLoopMode, NSEventType, NSImage, NSMenu, NSMenuItem, NSSquareStatusItemLength,
    NSStatusBar, NSStatusBarButton, NSWorkspace,
};
use objc2_foundation::{
    MainThreadMarker, NSData, NSDate, NSDefaultRunLoopMode, NSObject, NSRunLoop, NSSize, NSString,
    NSTimer, NSURL, ns_string,
};
use std::cell::RefCell;

struct TargetState {
    url: Retained<NSURL>,
    updates_url: Retained<NSURL>,
    control: Control,
    tracking_menu: RefCell<Option<Retained<NSMenu>>>,
}

define_class!(
    // 安全性: NSObject に追加のサブクラス要件はなく、AppKit と同じメインスレッドだけで扱う。
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    #[ivars = TargetState]
    struct ManagerTarget;

    impl ManagerTarget {
        // 安全性: target/action の sender を受け取る Objective-C のシグネチャに一致する。
        #[unsafe(method(openManager:))]
        fn open_manager(&self, _sender: &AnyObject) {
            self.open();
        }

        #[unsafe(method(restartService:))]
        fn restart_service(&self, _sender: &AnyObject) {
            self.ivars().control.restart();
        }

        #[unsafe(method(quitService:))]
        fn quit_service(&self, _sender: &AnyObject) {
            self.ivars().control.quit();
        }

        #[unsafe(method(checkUpdates:))]
        fn check_updates(&self, _sender: &AnyObject) {
            if !NSWorkspace::sharedWorkspace().openURL(&self.ivars().updates_url) {
                eprintln!("Could not open the Skilvella update settings");
            }
        }

        #[unsafe(method(checkStopped:))]
        fn check_stopped(&self, _sender: &AnyObject) {
            if self.ivars().control.is_stopped()
                && let Some(menu) = self.ivars().tracking_menu.borrow().as_ref()
            {
                menu.cancelTracking();
            }
        }

        #[unsafe(method(handleClick:))]
        fn handle_click(&self, sender: &NSStatusBarButton) {
            let app = NSApplication::sharedApplication(self.mtm());
            if let Some(event) = app.currentEvent()
                && (event.r#type() == NSEventType::RightMouseUp
                    || event.modifierFlags().contains(NSEventModifierFlags::Control))
            {
                let menu = self.menu();
                self.ivars().tracking_menu.replace(Some(menu.clone()));
                // メニュー追跡中も停止を確認し、Ctrl+Cやサーバー異常でメニューを閉じる。
                // selectorを実装したtargetとtimerは、このメインスレッドで処理完了まで保持する。
                let timer = unsafe {
                    let timer = NSTimer::timerWithTimeInterval_target_selector_userInfo_repeats(
                        0.1, self, sel!(checkStopped:), None, true,
                    );
                    NSRunLoop::currentRunLoop().addTimer_forMode(&timer, NSEventTrackingRunLoopMode);
                    timer
                };
                NSMenu::popUpContextMenu_withEvent_forView(&menu, &event, sender);
                timer.invalidate();
                self.ivars().tracking_menu.take();
            } else {
                self.open();
            }
        }
    }
);

impl ManagerTarget {
    fn open(&self) {
        if !NSWorkspace::sharedWorkspace().openURL(&self.ivars().url) {
            eprintln!("Could not open the Skilvella manager in the default browser");
        }
    }

    fn new(
        mtm: MainThreadMarker,
        url: Retained<NSURL>,
        updates_url: Retained<NSURL>,
        control: Control,
    ) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(TargetState {
            url,
            updates_url,
            control,
            tracking_menu: RefCell::new(None),
        });
        // 安全性: NSObject の init を、初期化済みの ivars を持つ割当てに対して一度だけ呼ぶ。
        unsafe { msg_send![super(this), init] }
    }

    fn menu(&self) -> Retained<NSMenu> {
        let menu = NSMenu::initWithTitle(NSMenu::alloc(self.mtm()), ns_string!("Skilvella"));
        menu.setAutoenablesItems(false);
        let restarting = self.ivars().control.is_restarting();
        for (title, action, enabled) in [
            ("管理画面を開く", sel!(openManager:), true),
            ("更新を確認", sel!(checkUpdates:), true),
            (
                if restarting {
                    "サービスを再起動中…"
                } else {
                    "サービスを再起動"
                },
                sel!(restartService:),
                !restarting,
            ),
            ("終了", sel!(quitService:), true),
        ] {
            if action == sel!(quitService:) {
                menu.addItem(&NSMenuItem::separatorItem(self.mtm()));
            }
            // 各selectorはこのtargetが実装し、同期メニュー処理が終わるまでtargetは保持される。
            let item = unsafe {
                let item = NSMenuItem::initWithTitle_action_keyEquivalent(
                    NSMenuItem::alloc(self.mtm()),
                    &NSString::from_str(title),
                    Some(action),
                    ns_string!(""),
                );
                item.setTarget(Some(self));
                item
            };
            item.setEnabled(enabled);
            menu.addItem(&item);
        }
        menu
    }
}

pub fn run(url: &str, control: Control) -> anyhow::Result<()> {
    let mtm = MainThreadMarker::new().context("The macOS status item needs the main thread")?;
    autoreleasepool(|_| {
        let updates_url = NSURL::URLWithString(&NSString::from_str(&format!(
            "{url}settings?check_updates=1#updates"
        )))
        .context("Could not create the update settings URL")?;
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

        let target = ManagerTarget::new(mtm, url, updates_url, control.clone());
        let status_bar = NSStatusBar::systemStatusBar();
        let status_item = status_bar.statusItemWithLength(NSSquareStatusItemLength);
        let Some(button) = status_item.button(mtm) else {
            status_bar.removeStatusItem(&status_item);
            bail!("Could not create the macOS status item button");
        };
        button.setImage(Some(&image));
        button.setToolTip(Some(ns_string!(
            "Skilvella — クリックで管理画面、右クリックでメニュー"
        )));
        button.sendActionOn(NSEventMask::LeftMouseUp | NSEventMask::RightMouseUp);
        // 安全性: target は handleClick: を実装し、イベント処理が終わるまで保持する。
        unsafe {
            button.setTarget(Some(&target));
            button.setAction(Some(sel!(handleClick:)));
        }

        while !control.is_stopped() {
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
