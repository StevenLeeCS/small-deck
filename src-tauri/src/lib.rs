//! XPAD Small Deck agent.
//!
//! A tiny background tray app. It:
//!   1. reads the XPAD over USB to learn which F13–F24 ("Small Deck") triggers
//!      the user mapped in the web configurator (read-only, one-shot);
//!   2. lets the user bind each of those triggers to a program/file/URL;
//!   3. registers F13–F24 as global hotkeys and launches the bound target when
//!      one fires — so it works regardless of which app is focused, exactly the
//!      way the device sends them.

mod icons;
mod launcher;
mod store;
mod xpad;

use serde::Serialize;
use store::{Entry, Mappings, Store};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, State};
use tauri_plugin_autostart::{ManagerExt, MacosLauncher};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Shortcut, ShortcutState};

/// The 12 Small Deck trigger keys: global-shortcut code ↔ label ↔ HID usage.
const FKEYS: [(Code, &str, u8); 12] = [
    (Code::F13, "F13", 0x68),
    (Code::F14, "F14", 0x69),
    (Code::F15, "F15", 0x6A),
    (Code::F16, "F16", 0x6B),
    (Code::F17, "F17", 0x6C),
    (Code::F18, "F18", 0x6D),
    (Code::F19, "F19", 0x6E),
    (Code::F20, "F20", 0x6F),
    (Code::F21, "F21", 0x70),
    (Code::F22, "F22", 0x71),
    (Code::F23, "F23", 0x72),
    (Code::F24, "F24", 0x73),
];

#[derive(Serialize)]
struct DeviceKey {
    hid: u8,
    label: String,
}

/// Labels of the F-keys that actually registered as global hotkeys on this OS.
/// On macOS F21–F24 have no scancode and fail to register, so the UI greys them
/// out — this is the source of truth rather than hard-coding per platform.
struct SupportedKeys(Vec<String>);

/// Handles to the tray menu items so the frontend can relabel them when the
/// user switches language (the tray is built in Rust, before any UI language).
struct TrayItems {
    show: MenuItem<tauri::Wry>,
    quit: MenuItem<tauri::Wry>,
}

// ── Commands ────────────────────────────────────────────────────────────────

/// Read the connected XPAD and return the F13–F24 triggers it has mapped.
#[tauri::command]
fn read_device_fkeys() -> Result<Vec<DeviceKey>, String> {
    let hids = xpad::read_mapped_fkeys()?;
    Ok(hids
        .into_iter()
        .map(|hid| DeviceKey {
            hid,
            label: xpad::fkey_label(hid),
        })
        .collect())
}

/// Read the device's full key matrix (layout + key map), flagging Small Deck cells.
#[tauri::command]
fn read_device_matrix() -> Result<xpad::DeviceMatrix, String> {
    xpad::read_matrix()
}

/// Best-effort app icon for `path`, as a PNG data URL (None if unavailable).
#[tauri::command]
fn app_icon(path: String) -> Option<String> {
    icons::app_icon_data_url(&path)
}

/// Open a URL in the user's default browser (used by the header links).
#[tauri::command]
fn open_external(url: String) -> Result<(), String> {
    launcher::launch(&url)
}

#[tauri::command]
fn get_mappings(store: State<Store>) -> Mappings {
    store.snapshot()
}

#[tauri::command]
fn set_mapping(
    store: State<Store>,
    key: String,
    path: String,
    name: String,
    kind: String,
) -> Result<(), String> {
    store.set(key, Entry { path, name, kind })
}

#[tauri::command]
fn remove_mapping(store: State<Store>, key: String) -> Result<(), String> {
    store.remove(&key)
}

/// Launch the program bound to `key` right now (used by the "Test" button).
#[tauri::command]
fn launch_key(store: State<Store>, key: String) -> Result<(), String> {
    match store.get(&key) {
        Some(entry) => launcher::run_entry(&entry.kind, &entry.path),
        None => Err("No program bound to this key.".into()),
    }
}

#[tauri::command]
fn supported_keys(state: State<SupportedKeys>) -> Vec<String> {
    state.0.clone()
}

/// Relabel the tray menu items (called by the UI on load and on language switch).
#[tauri::command]
fn set_tray_labels(items: State<TrayItems>, show: String, quit: String) {
    let _ = items.show.set_text(show);
    let _ = items.quit.set_text(quit);
}

/// Fully quit the app (window "close" only hides to tray, so the UI needs an
/// explicit Quit beyond the tray menu).
#[tauri::command]
fn quit_app(app: AppHandle) {
    app.exit(0);
}

#[tauri::command]
fn get_autostart(app: AppHandle) -> bool {
    app.autolaunch().is_enabled().unwrap_or(false)
}

#[tauri::command]
fn set_autostart(app: AppHandle, enabled: bool) -> Result<(), String> {
    let m = app.autolaunch();
    let r = if enabled { m.enable() } else { m.disable() };
    r.map_err(|e| e.to_string())
}

// ── App ──────────────────────────────────────────────────────────────────────

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Single-instance MUST be the first plugin. A second launch just focuses
        // the running window instead of starting a rival that can't claim the
        // F13–F24 hotkeys (and would silently fail to register them).
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            show_main(app);
        }))
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_dialog::init())
        // Pass `--hidden` in the autostart launch command so a login-time start
        // comes up silently in the tray instead of popping the window.
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec!["--hidden"]),
        ))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut: &Shortcut, event| {
                    if event.state() != ShortcutState::Pressed {
                        return;
                    }
                    // Identify the fired F-key by value-comparing against the
                    // shortcuts we registered (avoids depending on Shortcut's
                    // internal field visibility across plugin versions).
                    for (code, label, _) in FKEYS {
                        if *shortcut == Shortcut::new(None, code) {
                            let store = app.state::<Store>();
                            if let Some(entry) = store.get(label) {
                                if let Err(e) = launcher::run_entry(&entry.kind, &entry.path) {
                                    eprintln!("[smalldeck] launch failed for {label}: {e}");
                                }
                            }
                            break;
                        }
                    }
                })
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            read_device_fkeys,
            read_device_matrix,
            app_icon,
            open_external,
            get_mappings,
            set_mapping,
            remove_mapping,
            launch_key,
            supported_keys,
            set_tray_labels,
            quit_app,
            get_autostart,
            set_autostart,
        ])
        .setup(|app| {
            // Persisted mappings live in the per-user app config dir.
            let cfg_dir = app.path().app_config_dir()?;
            let store = Store::load(cfg_dir.join("mappings.json"));
            app.manage(store);

            // Register all 12 F-keys as global hotkeys. Some are unavailable
            // (e.g. macOS has no scancode for F21–F24) — record which succeeded
            // so the UI can grey out the rest.
            let gs = app.global_shortcut();
            let mut supported = Vec::new();
            for (code, label, _) in FKEYS {
                match gs.register(Shortcut::new(None, code)) {
                    Ok(()) => supported.push(label.to_string()),
                    Err(e) => eprintln!("[smalldeck] could not register {label}: {e}"),
                }
            }
            app.manage(SupportedKeys(supported));

            // System-tray icon with a small menu.
            let show = MenuItem::with_id(app, "show", "Show", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &quit])?;
            // Keep handles so the UI can localize these once it knows the language.
            app.manage(TrayItems { show: show.clone(), quit: quit.clone() });
            let _tray = TrayIconBuilder::with_id("main-tray")
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip("Small Deck")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => show_main(app),
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        ..
                    } = event
                    {
                        show_main(tray.app_handle());
                    }
                })
                .build(app)?;

            // Login-time autostart passes `--hidden`: come up silently in the
            // tray rather than stealing focus with the window on every boot.
            if std::env::args().any(|a| a == "--hidden") {
                if let Some(win) = app.get_webview_window("main") {
                    let _ = win.hide();
                }
                // Started hidden (login autostart) → no Dock icon, menu-bar only.
                #[cfg(target_os = "macos")]
                let _ = app
                    .handle()
                    .set_activation_policy(tauri::ActivationPolicy::Accessory);
            }

            Ok(())
        })
        // Closing the window hides it to the tray instead of quitting, so the
        // hotkeys keep working in the background.
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let _ = window.hide();
                api.prevent_close();
                // Hidden to tray → become a menu-bar-only app (drop the Dock icon).
                #[cfg(target_os = "macos")]
                let _ = window
                    .app_handle()
                    .set_activation_policy(tauri::ActivationPolicy::Accessory);
                notify_tray_once(window.app_handle());
            }
        })
        .build(tauri::generate_context!())
        .expect("error while running XPAD Small Deck")
        .run(|_app, _event| {
            // While the window is hidden in the tray, clicking the Dock icon
            // sends a Reopen on macOS (no new process, so single-instance never
            // fires) — bring the window back.
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Reopen { .. } = _event {
                show_main(_app);
            }
        });
}

fn show_main(app: &AppHandle) {
    // Showing the window → behave like a normal app (Dock icon, Cmd-Tab).
    #[cfg(target_os = "macos")]
    let _ = app.set_activation_policy(tauri::ActivationPolicy::Regular);
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.unminimize(); // restore if minimized (Windows/Linux taskbar, macOS Dock)
        let _ = win.show();
        let _ = win.set_focus();
    }
}

/// The first time the window is closed-to-tray, let the user know it's still
/// running (otherwise people assume "close" quit it). Shown only once, tracked
/// by a marker file in the app config dir. Bilingual to avoid coupling to the
/// UI language (which lives in the webview).
fn notify_tray_once(app: &AppHandle) {
    use tauri_plugin_notification::NotificationExt;
    let Ok(dir) = app.path().app_config_dir() else { return };
    let marker = dir.join(".tray-hint-shown");
    if marker.exists() {
        return;
    }
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(&marker, b"1");
    let _ = app
        .notification()
        .builder()
        .title("Small Deck")
        .body("仍在托盘运行,可从托盘图标退出。\nStill running in the tray — quit from its icon.")
        .show();
}
