// Hide the console window in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use jojo_app::bridge::Bridge;
use jojo_app::daemon_manager::DaemonPaths;
use jojo_app::layout::{self, LayoutFile};
use jojo_app::settings::{self, Settings};
use jojo_app::shells::{self, ShellOption};
use std::sync::Arc;
use tauri::ipc::{Channel, InvokeBody, InvokeResponseBody, Request};
use tauri::webview::NewWindowResponse;
use tauri::{Manager, State, WebviewWindowBuilder};

#[tauri::command]
fn subscribe(events: Channel<InvokeResponseBody>, bridge: State<'_, Arc<Bridge>>) {
    bridge.set_sink(Box::new(move |bytes| {
        let _ = events.send(InvokeResponseBody::Raw(bytes));
    }));
}

#[tauri::command]
fn send_frame(request: Request<'_>, bridge: State<'_, Arc<Bridge>>) -> Result<(), String> {
    let InvokeBody::Raw(body) = request.body() else {
        return Err("send_frame expects raw frame bytes".into());
    };
    bridge.send(body.clone()).map_err(str::to_owned)
}

#[tauri::command]
fn restart_daemon(bridge: State<'_, Arc<Bridge>>) {
    bridge.request_restart();
}

#[tauri::command]
fn get_settings() -> Settings {
    settings::load(&settings::default_path())
}

/// The shell programs installed on this PC, for the "New shell" menus
/// (see `jojo_app::shells`).
#[tauri::command]
fn available_shells() -> Vec<ShellOption> {
    shells::available().to_vec()
}

#[tauri::command]
fn get_layout() -> LayoutFile {
    layout::load(&layout::default_path())
}

/// Async so the file write runs on a blocking worker thread instead of the
/// main thread. The frontend chains its calls, so two saves never overlap.
#[tauri::command]
async fn save_layout(layout: LayoutFile) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || jojo_app::layout::save(&jojo_app::layout::default_path(), &layout))
        .await
        .map_err(|e| format!("the layout writer thread failed: {e}"))?
        .map_err(|e| format!("could not write layout.json: {e}"))
}

/// Opens a link clicked in a terminal in the default browser. Only
/// `http://` and `https://` URLs are accepted (see `jojo_app::links`).
/// ShellExecuteW can block, for example while Windows asks which app should
/// open the link; a synchronous command runs on the main thread, which runs
/// the window's event loop, so the call is made on a blocking worker thread.
#[tauri::command]
async fn open_url(url: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || jojo_app::links::open_url(&url))
        .await
        .map_err(|e| format!("the link opener thread failed: {e}"))?
}

/// Places the snap-layout input sink over the HTML maximize button (see
/// `jojo_app::snap`). The frontend sends the button's rectangle in
/// physical pixels whenever the window or the button changes size.
#[tauri::command]
fn set_maximize_button_rect(x: i32, y: i32, width: i32, height: i32) {
    #[cfg(windows)]
    jojo_app::snap::set_button_rect(x, y, width, height);
    #[cfg(not(windows))]
    let _ = (x, y, width, height);
}

fn main() {
    let bridge = Arc::new(Bridge::default());
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .manage(bridge.clone())
        .invoke_handler(tauri::generate_handler![
            subscribe,
            send_frame,
            restart_daemon,
            get_settings,
            available_shells,
            get_layout,
            save_layout,
            open_url,
            set_maximize_button_rect
        ])
        .setup(move |app| {
            // The main window has `"create": false` in tauri.conf.json so it
            // can be built here with a new-window handler. Every request to
            // open a new webview window (window.open, target="_blank") is
            // denied; links leave the app only through the open_url command.
            let config = app.config().app.windows.iter().find(|w| w.label == "main").ok_or("no main window in tauri.conf.json")?.clone();
            // enable_clipboard_access makes wry allow WebView2's clipboard-read
            // permission, which navigator.clipboard.readText (Paste) needs.
            let window = WebviewWindowBuilder::from_config(app.handle(), &config)?
                .on_new_window(|_, _| NewWindowResponse::Deny)
                .enable_clipboard_access()
                .build()?;
            // Snap layouts add a flyout to the maximize button; they are not
            // needed for the button to work. If the sink cannot be created,
            // a click on the HTML button still maximizes through the window
            // API, only without the flyout.
            #[cfg(windows)]
            if let Ok(hwnd) = window.hwnd() {
                let _ = jojo_app::snap::install(app.handle(), hwnd.0 as isize);
            }
            #[cfg(not(windows))]
            drop(window);
            let source = app.path().resource_dir()?.join("daemon");
            let paths = DaemonPaths::for_app(source);
            tauri::async_runtime::spawn(jojo_app::connection::run(paths, bridge.clone()));
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("run Jojo");
}
