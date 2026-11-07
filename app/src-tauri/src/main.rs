// Hide the console window in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use jojo_app::bridge::Bridge;
use jojo_app::daemon_manager::DaemonPaths;
use jojo_app::settings::{self, Settings};
use std::sync::Arc;
use tauri::ipc::{Channel, InvokeBody, InvokeResponseBody, Request};
use tauri::{Manager, State};

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
    bridge.restart.notify_one();
}

#[tauri::command]
fn get_settings() -> Settings {
    settings::load(&settings::default_path())
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
        .invoke_handler(tauri::generate_handler![subscribe, send_frame, restart_daemon, get_settings])
        .setup(move |app| {
            let source = app.path().resource_dir()?.join("daemon");
            let paths = DaemonPaths::for_app(source);
            tauri::async_runtime::spawn(jojo_app::connection::run(paths, bridge.clone()));
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("run Jojo");
}
