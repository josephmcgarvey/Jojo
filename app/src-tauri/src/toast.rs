//! Windows toast notifications for session state changes (spec 7.4).
//!
//! The frontend decides when a toast is due (rules 1, 2, 4 and 6, in
//! `app/src/lib/state/toasts.ts`); this module only shows it. Clicking a
//! toast brings the window to the front and emits `TOAST_ACTIVATED` with the
//! session id, which the frontend uses to focus the session's pane (rule 3).

use tauri::{AppHandle, Emitter, Manager};

/// The event emitted when the user clicks a toast. Payload: the session id.
pub const TOAST_ACTIVATED: &str = "toast-activated";

/// The AppUserModelID toasts are shown under. Windows shows a toast only for
/// an id registered with a Start menu shortcut: the NSIS installer creates
/// one for the bundle identifier. A development build has no shortcut, so it
/// borrows PowerShell's id, as tauri-plugin-notification does.
fn app_user_model_id(app: &AppHandle) -> String {
    if cfg!(debug_assertions) {
        tauri_winrt_notification::Toast::POWERSHELL_APP_ID.to_owned()
    } else {
        app.config().identifier.clone()
    }
}

/// Shows one toast. `session_id` is passed back through `TOAST_ACTIVATED`
/// when the toast is clicked.
pub fn show(app: &AppHandle, session_id: String, title: &str, body: &str) -> Result<(), String> {
    let handle = app.clone();
    tauri_winrt_notification::Toast::new(&app_user_model_id(app))
        .title(title)
        .text1(body)
        .on_activated(move |_| {
            if let Some(window) = handle.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
            let _ = handle.emit(TOAST_ACTIVATED, session_id.clone());
            Ok(())
        })
        .show()
        .map_err(|e| format!("the toast could not be shown: {e}"))
}
