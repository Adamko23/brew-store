#[cfg(target_os = "macos")]
pub fn set_dock_icon() -> Result<(), String> {
    use objc2::ClassType;
    use objc2_app_kit::{NSApplication, NSImage};
    use objc2_foundation::{MainThreadMarker, NSData};

    let main_thread = MainThreadMarker::new().ok_or(crate::i18n::t(
        "Ikonu Docku treba nastaviť na hlavnom vlákne.",
    ))?;
    let data = NSData::with_bytes(include_bytes!("../assets/brew-store-dock.png"));
    let image = NSImage::initWithData(NSImage::alloc(), &data)
        .ok_or(crate::i18n::t("macOS nedokázal načítať ikonu aplikácie."))?;
    let application = NSApplication::sharedApplication(main_thread);
    // SAFETY: called after Iced creates NSApplication, on the verified main
    // thread, with a live, non-null NSImage built from our bundled PNG.
    unsafe {
        application.setApplicationIconImage(Some(&image));
    }
    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub fn set_dock_icon() -> Result<(), String> {
    Ok(())
}

#[cfg(target_os = "macos")]
pub fn locale_preferences() -> (Vec<String>, String) {
    use objc2_foundation::NSLocale;
    // SAFETY: Foundation returns immutable locale/language values with owned
    // retain semantics. No UI objects or unchecked casts are involved.
    unsafe {
        let languages = NSLocale::preferredLanguages();
        let values = (0..languages.count())
            .map(|index| languages.objectAtIndex(index).to_string())
            .collect();
        let separator = NSLocale::currentLocale().decimalSeparator().to_string();
        (values, separator)
    }
}

#[cfg(not(target_os = "macos"))]
pub fn locale_preferences() -> (Vec<String>, String) {
    (vec!["en".into()], ".".into())
}
