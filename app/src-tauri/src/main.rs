// Hide the console window in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use jojo_app::bridge::LOCAL_MACHINE;
use jojo_app::hub::Hub;
use jojo_app::install::InstallStep;
use jojo_app::machines::{self, Machine, MachineKind};
use jojo_app::daemon_manager::DaemonPaths;
use jojo_app::layout::{self, LayoutFile};
use jojo_app::settings::{self, Settings};
use jojo_app::shells::{self, ShellOption};
use std::sync::Arc;
use tauri::ipc::{Channel, InvokeBody, InvokeResponseBody, Request};
use tauri::webview::NewWindowResponse;
use tauri::{Manager, State, WebviewWindowBuilder};

#[tauri::command]
fn subscribe(events: Channel<InvokeResponseBody>, hub: State<'_, Arc<Hub>>) {
    hub.frontend.set_sink(Box::new(move |bytes| {
        let _ = events.send(InvokeResponseBody::Raw(bytes));
    }));
}

#[tauri::command]
/// Sends one frame body to the machine named in the `jojo-machine` header
/// (This PC when the header is missing).
fn send_frame(request: Request<'_>, hub: State<'_, Arc<Hub>>) -> Result<(), String> {
    let InvokeBody::Raw(body) = request.body() else {
        return Err("send_frame expects raw frame bytes".into());
    };
    let machine = request.headers().get("jojo-machine").and_then(|v| v.to_str().ok()).unwrap_or(LOCAL_MACHINE);
    let bridge = hub.bridge(machine).ok_or_else(|| format!("unknown machine {machine}"))?;
    bridge.send(body.clone()).map_err(str::to_owned)
}

#[tauri::command]
/// This PC: restarts jojod. A remote machine: reconnects now.
fn restart_daemon(machine: Option<String>, hub: State<'_, Arc<Hub>>) {
    if let Some(bridge) = hub.bridge(machine.as_deref().unwrap_or(LOCAL_MACHINE)) {
        bridge.request_restart();
    }
}

/// The machines in `machines.toml`, This PC first.
#[tauri::command]
fn list_machines() -> Vec<Machine> {
    machines::load(&machines::default_path())
}

/// Host aliases from `~/.ssh/config`, for the Add machine dialog.
#[tauri::command]
fn ssh_hosts() -> Vec<String> {
    jojo_app::install::default_ssh_config()
        .and_then(|path| std::fs::read_to_string(path).ok())
        .map(|config| jojo_app::install::ssh_config_hosts(&config))
        .unwrap_or_default()
}

/// Spec 8.2: installs jojod on `host`, reporting each step on `progress`,
/// then adds the machine to machines.toml and connects to it.
#[tauri::command]
async fn add_machine(
    app: tauri::AppHandle,
    hub: State<'_, Arc<Hub>>,
    host: String,
    label: String,
    allow_restart: bool,
    progress: Channel<InstallStep>,
) -> Result<Machine, String> {
    let host = host.trim().to_owned();
    if host.is_empty() || host.starts_with('-') || host.chars().any(char::is_whitespace) {
        return Err("Enter a host from ~/.ssh/config or user@host".into());
    }
    let daemon_dir = app.path().resource_dir().map_err(|e| e.to_string())?.join("daemon");
    jojo_app::install::install(&host, &daemon_dir, allow_restart, |step| {
        let _ = progress.send(step);
    })
    .await?;
    let path = machines::default_path();
    let mut list = machines::load(&path);
    if let Some(existing) = list.iter().find(|m| m.ssh_host.as_deref() == Some(host.as_str())) {
        // Adding a host that is already listed upgrades and restarts it; the
        // existing entry (and its layout keys) stays.
        let existing = existing.clone();
        hub.stop(&existing.id);
        hub.start(&existing, || unreachable!("a remote machine has no local paths"));
        return Ok(existing);
    }
    let label = if label.trim().is_empty() { host.clone() } else { label.trim().to_owned() };
    let machine = Machine { id: jojo_proto::SessionId::new().to_string(), label, kind: MachineKind::Ssh, ssh_host: Some(host), enabled: true };
    list.push(machine.clone());
    machines::save(&path, &list).map_err(|e| format!("could not save machines.toml: {e}"))?;
    hub.start(&machine, || unreachable!("a remote machine has no local paths"));
    Ok(machine)
}

/// Spec 8.4: disconnects a remote machine and removes it from
/// machines.toml. With `uninstall`, first runs `jojod shutdown` and
/// `jojod uninstall-hooks` on the server and deletes `~/.local/bin/jojod`
/// and `~/.jojo/`.
#[tauri::command]
async fn remove_machine(hub: State<'_, Arc<Hub>>, id: String, uninstall: bool) -> Result<(), String> {
    let path = machines::default_path();
    let mut list = machines::load(&path);
    let Some(machine) = list.iter().find(|m| m.id == id && m.kind == MachineKind::Ssh).cloned() else {
        return Err("This PC cannot be removed".into());
    };
    // The uninstall runs first: when it fails (for example, the server is
    // unreachable), the machine stays listed with its connection loop.
    if uninstall {
        if let Some(host) = &machine.ssh_host {
            let script = format!(
                "{0} shutdown; {0} uninstall-hooks; rm -f ~/.local/bin/jojod && rm -rf ~/.jojo",
                jojo_app::ssh::REMOTE_JOJOD
            );
            let out = jojo_app::ssh::run(host, &script, b"").await.map_err(|e| e.to_string())?;
            if out.code != Some(0) {
                return Err(format!("uninstall on {host} failed: {}", out.stderr.trim()));
            }
        }
    }
    hub.stop(&id);
    list.retain(|m| m.id != id);
    machines::save(&path, &list).map_err(|e| format!("could not save machines.toml: {e}"))
}

#[tauri::command]
fn get_settings() -> Settings {
    settings::load(&settings::default_path())
}

/// The number of sessions This PC's jojod is running, read from its
/// state.json (spec 8.4: "Restart now (ends N sessions)"). Reading the file
/// works while the running daemon speaks another protocol, when the app
/// cannot ask it; 0 when the file is missing or unreadable.
#[tauri::command]
fn local_running_sessions() -> usize {
    let path = jojod::paths::default_state_dir().join("state.json");
    std::fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<jojod::store::StoreData>(&bytes).ok())
        .map(|data| data.sessions.iter().filter(|s| s.state != jojo_proto::AgentState::Exited).count())
        .unwrap_or(0)
}

/// Saves the settings screen's values (spec 9.10).
#[tauri::command]
fn save_settings(settings: Settings) -> Result<(), String> {
    settings::save(&settings::default_path(), &settings).map_err(|e| format!("could not save settings.json: {e}"))
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

/// Shows a Windows toast for a session (spec 7.4). The frontend decides when
/// one is due; clicking it emits `toast-activated` with `session_id`. The
/// toast is shown on a blocking worker thread: a synchronous command would
/// run on the main thread, which runs the window's event loop.
#[tauri::command]
async fn notify(app: tauri::AppHandle, session_id: String, title: String, body: String) -> Result<(), String> {
    #[cfg(windows)]
    return tauri::async_runtime::spawn_blocking(move || jojo_app::toast::show(&app, session_id, &title, &body))
        .await
        .map_err(|e| format!("the toast thread failed: {e}"))?;
    #[cfg(not(windows))]
    {
        let _ = (app, session_id, title, body);
        Ok(())
    }
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
    let hub = Arc::new(Hub::default());
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .manage(hub.clone())
        .invoke_handler(tauri::generate_handler![
            subscribe,
            send_frame,
            restart_daemon,
            list_machines,
            ssh_hosts,
            add_machine,
            remove_machine,
            get_settings,
            save_settings,
            local_running_sessions,
            available_shells,
            get_layout,
            save_layout,
            open_url,
            set_maximize_button_rect,
            notify
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
            for machine in machines::load(&machines::default_path()) {
                hub.start(&machine, || DaemonPaths::for_app(source.clone()));
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("run Jojo");
}
