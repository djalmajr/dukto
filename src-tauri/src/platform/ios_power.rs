use objc2::MainThreadMarker;
use objc2_ui_kit::UIApplication;

/// Keep the screen awake while Dukto is in the foreground so iOS does not
/// suspend an active local-network transfer.
pub fn set_idle_timer_disabled(app: &tauri::AppHandle, disabled: bool) -> tauri::Result<()> {
    app.run_on_main_thread(move || {
        let main_thread = MainThreadMarker::new().expect("iOS UI callback must run on main thread");
        UIApplication::sharedApplication(main_thread).setIdleTimerDisabled(disabled);
    })
}
