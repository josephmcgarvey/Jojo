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

/// A remote location added with a username and password instead of a host
/// from ~/.ssh/config (the Add remote location dialog's "Username and
/// password").
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct PasswordSignIn {
    address: String,
    port: Option<u16>,
    user: String,
    password: String,
    /// Install an SSH key with the password and sign in with the key from
    /// then on; the password is kept only when the server refuses the key.
    setup_key: bool,
}

/// Where the password of a location is kept, for messages.
const CREDENTIAL_STORE: &str = if cfg!(windows) { "Windows Credential Manager" } else { "the Keychain" };

/// Spec 8.2: installs jojod on `host`, reporting each step on `progress`,
/// then adds the machine to machines.toml and connects to it. With
/// `sign_in`, the location is reached with a username and password; with
/// `setup_key`, Jojo first installs its key (see `install::setup_key`).
#[tauri::command]
async fn add_machine(
    app: tauri::AppHandle,
    hub: State<'_, Arc<Hub>>,
    host: String,
    label: String,
    allow_restart: bool,
    sign_in: Option<PasswordSignIn>,
    progress: Channel<InstallStep>,
) -> Result<Machine, String> {
    let report = |step: InstallStep| {
        let _ = progress.send(step);
    };
    let path = machines::default_path();
    let id = jojo_proto::SessionId::new().to_string();
    // The target the install runs against, and whether its password must be saved.
    let (target, save_password) = match sign_in {
        Some(sign_in) => {
            let address = sign_in.address.trim().to_owned();
            let user = sign_in.user.trim().to_owned();
            if address.is_empty() || address.starts_with('-') || address.contains(['@', ' ']) {
                return Err("Enter the server's host name or IP address".into());
            }
            if user.is_empty() || user.starts_with('-') || user.contains(['@', ' ']) {
                return Err("Enter the username to sign in with".into());
            }
            if sign_in.password.is_empty() {
                return Err("Enter the password".into());
            }
            let with_password = jojo_app::ssh::SshTarget {
                host: format!("{user}@{address}"),
                port: sign_in.port.filter(|&p| p != 22),
                identity_file: None,
                password: Some(sign_in.password),
            };
            if sign_in.setup_key {
                let key = jojo_app::install::default_jojo_key().ok_or("cannot find your home folder for ~/.ssh")?;
                match jojo_app::install::setup_key(&with_password, &key, report).await {
                    Ok(with_key) => (with_key, false),
                    Err(why) => {
                        report(InstallStep {
                            step: "key".into(),
                            status: "warning".into(),
                            message: format!("Signing in with the password, saved in {CREDENTIAL_STORE}, because {why}"),
                        });
                        (with_password, true)
                    }
                }
            } else {
                (with_password, true)
            }
        }
        None => {
            let host = host.trim().to_owned();
            if host.is_empty() || host.starts_with('-') || host.chars().any(char::is_whitespace) {
                return Err("Enter a host from ~/.ssh/config or user@host".into());
            }
            // Adding a listed host again (Upgrade jojod) uses its saved sign-in.
            let listed = machines::load(&path).into_iter().find(|m| m.ssh_host.as_deref() == Some(host.as_str()));
            (listed.and_then(|m| m.ssh_target()).unwrap_or_else(|| jojo_app::ssh::SshTarget::host(host)), false)
        }
    };
    let host = target.host.clone();
    let daemon_dir = app.path().resource_dir().map_err(|e| e.to_string())?.join("daemon");
    jojo_app::install::install(&target, &daemon_dir, allow_restart, report).await?;
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
    if save_password {
        jojo_app::credentials::store(&id, target.password.as_deref().unwrap_or_default())?;
    }
    let machine = Machine {
        id,
        label,
        kind: MachineKind::Ssh,
        ssh_host: Some(host),
        enabled: true,
        port: target.port,
        identity_file: target.identity_file.as_ref().map(|p| p.display().to_string()),
        password: save_password,
    };
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
        if let Some(target) = machine.ssh_target() {
            let script = format!(
                "{0} shutdown; {0} uninstall-hooks; rm -f ~/.local/bin/jojod && rm -rf ~/.jojo",
                jojo_app::ssh::REMOTE_JOJOD
            );
            let out = jojo_app::ssh::run(&target, &script, b"").await.map_err(|e| e.to_string())?;
            if out.code != Some(0) {
                return Err(format!("uninstall on {} failed: {}", target.host, out.stderr.trim()));
            }
        }
    }
    hub.stop(&id);
    if machine.password {
        jojo_app::credentials::delete(&id);
    }
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
/// cannot ask it; None when the file is missing or cannot be parsed.
#[tauri::command]
fn local_running_sessions() -> Option<usize> {
    let path = jojod::paths::default_state_dir().join("state.json");
    std::fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<jojod::store::StoreData>(&bytes).ok())
        .map(|data| data.sessions.iter().filter(|s| s.state != jojo_proto::AgentState::Exited).count())
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

#[tauri::command]
fn get_snippets() -> Vec<jojo_app::snippets::Snippet> {
    jojo_app::snippets::load(&jojo_app::snippets::default_path())
}

/// Async so the file write runs on a blocking worker thread, as save_layout.
#[tauri::command]
async fn save_snippets(snippets: Vec<jojo_app::snippets::Snippet>) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || jojo_app::snippets::save(&jojo_app::snippets::default_path(), &snippets))
        .await
        .map_err(|e| format!("the snippets writer thread failed: {e}"))?
        .map_err(|e| format!("could not write snippets.json: {e}"))
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

/// Shows a notification for a session (spec 7.4). The frontend decides when
/// one is due. On Windows it is a toast, shown on a blocking worker thread
/// (a synchronous command would run on the main thread, which runs the
/// window's event loop), and clicking it emits `toast-activated` with
/// `session_id`. On macOS it is a Notification Center notification through
/// tauri-plugin-notification, which reports no clicks, so clicking it only
/// brings Jojo to the front.
#[tauri::command]
async fn notify(app: tauri::AppHandle, session_id: String, title: String, body: String) -> Result<(), String> {
    #[cfg(windows)]
    return tauri::async_runtime::spawn_blocking(move || jojo_app::toast::show(&app, session_id, &title, &body))
        .await
        .map_err(|e| format!("the toast thread failed: {e}"))?;
    #[cfg(target_os = "macos")]
    {
        use tauri_plugin_notification::NotificationExt;
        let _ = session_id;
        app.notification().builder().title(title).body(body).show().map_err(|e| format!("the notification could not be shown: {e}"))
    }
    #[cfg(all(unix, not(target_os = "macos")))]
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

/// The macOS menu bar. Tauri's default menu binds ⌘W to Close Window, which
/// would close Jojo instead of the focused pane, so the app builds its own:
/// Jojo (About, Hide, Quit), Edit (the clipboard items text fields need) and
/// Window (Minimize, Zoom, Full Screen). The app's own ⌘ shortcuts are
/// handled in the webview (app/src/lib/input/shortcuts.ts), which sees a key
/// before the menu does.
#[cfg(target_os = "macos")]
fn mac_menu(app: &tauri::AppHandle) -> tauri::Result<tauri::menu::Menu<tauri::Wry>> {
    use tauri::menu::{Menu, PredefinedMenuItem as Item, Submenu};
    let jojo = Submenu::with_items(
        app,
        "Jojo",
        true,
        &[
            &Item::about(app, None, None)?,
            &Item::separator(app)?,
            &Item::hide(app, None)?,
            &Item::hide_others(app, None)?,
            &Item::show_all(app, None)?,
            &Item::separator(app)?,
            &Item::quit(app, None)?,
        ],
    )?;
    let edit = Submenu::with_items(
        app,
        "Edit",
        true,
        &[
            &Item::undo(app, None)?,
            &Item::redo(app, None)?,
            &Item::separator(app)?,
            &Item::cut(app, None)?,
            &Item::copy(app, None)?,
            &Item::paste(app, None)?,
            &Item::select_all(app, None)?,
        ],
    )?;
    let window = Submenu::with_items(app, "Window", true, &[&Item::minimize(app, None)?, &Item::maximize(app, None)?, &Item::fullscreen(app, None)?])?;
    Menu::with_items(app, &[&jojo, &edit, &window])
}

fn main() {
    let hub = Arc::new(Hub::default());
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_dialog::init());
    #[cfg(target_os = "macos")]
    let builder = builder.plugin(tauri_plugin_notification::init());
    builder
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
            get_snippets,
            save_snippets,
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
            #[cfg(target_os = "macos")]
            app.set_menu(mac_menu(app.handle())?)?;
            let source = app.path().resource_dir()?.join("daemon");
            // ssh's password helper for locations that sign in with a password.
            jojo_app::ssh::set_askpass_program(source.join(jojo_app::daemon_manager::JOJOD_FILE));
            for machine in machines::load(&machines::default_path()) {
                hub.start(&machine, || DaemonPaths::for_app(source.clone()));
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("run Jojo");
}
