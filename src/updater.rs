//! Sparkle owns update scheduling, signature verification and installation.
//! Keep native objects on the main thread; no Iced polling/render timer is used.

#[cfg(target_os = "macos")]
pub fn start() -> Result<(), String> {
    use objc2::rc::{Allocated, Retained};
    use objc2::runtime::{AnyClass, AnyObject, Bool};
    use objc2::{msg_send, msg_send_id, sel};
    use objc2_app_kit::{NSApplication, NSMenu, NSMenuItem};
    use objc2_foundation::{MainThreadMarker, NSBundle, NSString};
    use std::cell::RefCell;

    struct Updater {
        _framework: Retained<NSBundle>,
        _controller: Retained<AnyObject>,
    }
    thread_local! {
        static UPDATER: RefCell<Option<Updater>> = const { RefCell::new(None) };
    }

    if UPDATER.with(|updater| updater.borrow().is_some()) {
        return Ok(());
    }
    let executable = std::env::current_exe().map_err(|error| error.to_string())?;
    let Some(contents) = executable.parent().and_then(std::path::Path::parent) else {
        return Ok(());
    };
    // Plain cargo run/test binaries are intentionally usable without Sparkle.
    if contents.file_name() != Some(std::ffi::OsStr::new("Contents")) {
        return Ok(());
    }
    let framework = contents.join("Frameworks/Sparkle.framework");
    if !framework.is_dir() {
        return Err(crate::i18n::t("V aplikácii chýba aktualizačný modul Sparkle.").into());
    }
    let main_thread = MainThreadMarker::new().ok_or("App updates require the main thread.")?;
    let path = NSString::from_str(
        framework
            .to_str()
            .ok_or("The update framework path is not valid UTF-8.")?,
    );
    // SAFETY: loads only the framework inside this signed app bundle. The
    // release builder verifies its pinned upstream archive and code signature.
    // All AppKit/Sparkle calls run on the verified main thread, after NSApp and
    // its first window exist. The bundle and controller stay retained for life.
    unsafe {
        let bundle = NSBundle::bundleWithPath(&path).ok_or("Cannot find Sparkle bundle.")?;
        if !bundle.load() {
            return Err("Cannot load the Sparkle update framework.".into());
        }
        let class = AnyClass::get("SPUStandardUpdaterController")
            .ok_or("Sparkle updater class is unavailable.")?;
        let controller: Allocated<AnyObject> = msg_send_id![class, alloc];
        let controller: Retained<AnyObject> = msg_send_id![controller,
            initWithStartingUpdater: Bool::YES,
            updaterDelegate: std::ptr::null::<AnyObject>(),
            userDriverDelegate: std::ptr::null::<AnyObject>()
        ];

        let application = NSApplication::sharedApplication(main_thread);
        let main_menu = application.mainMenu().unwrap_or_else(|| {
            let menu = NSMenu::new(main_thread);
            application.setMainMenu(Some(&menu));
            menu
        });
        let root = main_menu.itemAtIndex(0).unwrap_or_else(|| {
            let item = NSMenuItem::new(main_thread);
            main_menu.addItem(&item);
            item
        });
        let app_menu = root.submenu().unwrap_or_else(|| {
            let menu = NSMenu::new(main_thread);
            root.setSubmenu(Some(&menu));
            menu
        });
        let item = NSMenuItem::new(main_thread);
        item.setTitle(&NSString::from_str(crate::i18n::t(
            "Skontrolovať aktualizácie aplikácie…",
        )));
        item.setAction(Some(sel!(checkForUpdates:)));
        item.setTarget(Some(&controller));
        let position = app_menu.numberOfItems().min(2);
        app_menu.insertItem_atIndex(&item, position);
        app_menu.insertItem_atIndex(&NSMenuItem::separatorItem(main_thread), position + 1);

        // A single initial background check is supported by Sparkle. Afterwards
        // its own daily scheduler applies. Respect preferences changed by users.
        let updater: *mut AnyObject = msg_send![&*controller, updater];
        let automatic: Bool = msg_send![updater, automaticallyChecksForUpdates];
        if automatic.as_bool() {
            let _: () = msg_send![updater, checkForUpdatesInBackground];
        }
        UPDATER.with(|updater| {
            *updater.borrow_mut() = Some(Updater {
                _framework: bundle,
                _controller: controller,
            });
        });
    }
    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub fn start() -> Result<(), String> {
    Ok(())
}
