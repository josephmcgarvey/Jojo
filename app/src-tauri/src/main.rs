// Hide the console window in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use jojo_app::bridge::Bridge;
use jojo_app::daemon_manager::DaemonPaths;
use jojo_app::layout::{self, LayoutFile};
use jojo_app::settings::{self, Settings};
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
#[tauri::command]
fn open_url(url: String) -> Result<(), String> {
    jojo_app::links::open_url(&url)
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
            get_layout,
            save_layout,
            open_url
        ])
        .setup(move |app| {
            // The main window has `"create": false` in tauri.conf.json so it
            // can be built here with a new-window handler. Every request to
            // open a new webview window (window.open, target="_blank") is
            // denied; links leave the app only through the open_url command.
            let config = app.config().app.windows.iter().find(|w| w.label == "main").ok_or("no main window in tauri.conf.json")?.clone();
            WebviewWindowBuilder::from_config(app.handle(), &config)?.on_new_window(|_, _| NewWindowResponse::Deny).build()?;
            let source = app.path().resource_dir()?.join("daemon");
            let paths = DaemonPaths::for_app(source);
            tauri::async_runtime::spawn(jojo_app::connection::run(paths, bridge.clone()));
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("run Jojo");
}
