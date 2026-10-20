//! Nocta's place in the operating system: the tray icon, starting with Windows, system-wide
//! shortcuts and commands passed on the command line.
use crate::state::DesktopState;
use nocta_core::{cli, models::AppConfig, CoreError, CoreResult};
use std::{
    collections::HashMap,
    sync::{atomic::AtomicBool, Mutex},
};
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager,
};
use tauri_plugin_autostart::ManagerExt as _;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

/// The window's close button hides Nocta instead of quitting (kept in sync with the setting).
pub static CLOSE_TO_TRAY: AtomicBool = AtomicBool::new(false);
/// Argument Windows passes when it starts Nocta at sign-in.
pub const AUTOSTART_ARG: &str = "--autostart";

#[derive(Clone, Debug)]
enum HotkeyAction {
    Show,
    CloseRoblox,
    ToggleAfk,
    Preset(String),
}

static HOTKEYS: Mutex<Option<HashMap<u32, HotkeyAction>>> = Mutex::new(None);

pub fn show(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
        crate::memory::follow_visibility(&window);
    }
}

pub fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "show", "Open Nocta", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit Nocta", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &quit])?;
    let mut tray = TrayIconBuilder::with_id("nocta")
        .tooltip("Nocta")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => show(app),
            "quit" => crate::quit(app.clone()),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    Ok(())
}

/// Turns starting with Windows on or off to match the setting.
pub fn apply_autostart(app: &AppHandle, enabled: bool) -> CoreResult<()> {
    let launch = app.autolaunch();
    let current = launch.is_enabled().unwrap_or(false);
    let result = match (enabled, current) {
        (true, false) => launch.enable(),
        (false, true) => launch.disable(),
        _ => Ok(()),
    };
    result.map_err(|error| {
        CoreError::Validation(format!(
            "Starting with Windows could not be changed: {error}"
        ))
    })
}

/// Our `Ctrl+Alt+N` style in the form the shortcut library reads (it calls Win "Super").
fn shortcut(text: &str) -> CoreResult<Shortcut> {
    let normalized = text
        .split('+')
        .map(|part| match part.trim().to_ascii_lowercase().as_str() {
            "win" => "super".to_owned(),
            other => other.to_owned(),
        })
        .collect::<Vec<_>>()
        .join("+");
    normalized
        .parse::<Shortcut>()
        .map_err(|_| CoreError::Validation(format!("\"{text}\" is not a shortcut Nocta can use.")))
}

/// Registers the shortcuts from the settings, replacing the previous ones. A shortcut another
/// app already owns is reported; the others still work.
pub fn register_hotkeys(app: &AppHandle, config: &AppConfig) -> CoreResult<()> {
    let manager = app.global_shortcut();
    let _ = manager.unregister_all();
    let mut wanted: Vec<(String, HotkeyAction)> = vec![
        (config.hotkeys.show.clone(), HotkeyAction::Show),
        (
            config.hotkeys.close_roblox.clone(),
            HotkeyAction::CloseRoblox,
        ),
        (config.hotkeys.toggle_afk.clone(), HotkeyAction::ToggleAfk),
    ];
    wanted.extend(config.launch_presets.iter().map(|preset| {
        (
            preset.hotkey.clone(),
            HotkeyAction::Preset(preset.id.clone()),
        )
    }));
    let mut actions = HashMap::new();
    let mut failed = Vec::new();
    for (text, action) in wanted {
        if text.trim().is_empty() {
            continue;
        }
        let parsed = shortcut(&text)?;
        if actions.contains_key(&parsed.id()) {
            return Err(CoreError::Validation(format!(
                "{text} is set for two actions."
            )));
        }
        match manager.register(parsed) {
            Ok(()) => {
                actions.insert(parsed.id(), action);
            }
            Err(_) => failed.push(text),
        }
    }
    *HOTKEYS.lock().unwrap() = Some(actions);
    if failed.is_empty() {
        Ok(())
    } else {
        Err(CoreError::Validation(format!(
            "Another app already uses {}. Choose a different shortcut.",
            failed.join(", ")
        )))
    }
}

pub fn on_hotkey(app: &AppHandle, pressed: &Shortcut, state: ShortcutState) {
    if state != ShortcutState::Pressed {
        return;
    }
    let action = HOTKEYS
        .lock()
        .unwrap()
        .as_ref()
        .and_then(|actions| actions.get(&pressed.id()).cloned());
    let Some(action) = action else {
        return;
    };
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let state = app.state::<DesktopState>();
        let result: CoreResult<String> = async {
            match action {
                HotkeyAction::Show => {
                    show(&app);
                    Ok(String::new())
                }
                HotkeyAction::CloseRoblox => {
                    let closed = state.launcher.instances.close_all_roblox().await?;
                    Ok(format!("Shortcut: closed {closed} Roblox clients."))
                }
                HotkeyAction::ToggleAfk => {
                    let _guard = state.mutation.lock().await;
                    let mut config = state.storage.config().await?;
                    config.afk_mode = !config.afk_mode;
                    state.storage.save_config(&config).await?;
                    Ok(format!(
                        "Shortcut: AFK mode {}.",
                        if config.afk_mode { "on" } else { "off" }
                    ))
                }
                HotkeyAction::Preset(id) => {
                    let config = state.storage.config().await?;
                    let preset = config
                        .launch_presets
                        .iter()
                        .find(|preset| preset.id == id)
                        .cloned()
                        .ok_or(CoreError::NotFound)?;
                    let tasks = state.automation.run_preset(&preset, config).await?;
                    Ok(format!(
                        "Shortcut: started preset \"{}\" ({} accounts).",
                        preset.name,
                        tasks.len()
                    ))
                }
            }
        }
        .await;
        let (level, message) = match result {
            Ok(message) if message.is_empty() => return,
            Ok(message) => ("INFO", message),
            Err(error) => ("ERROR", format!("Shortcut failed: {error}")),
        };
        let _ = state.accounts.log(level, "APP", &message, None).await;
        let _guard = state.mutation.lock().await;
        let _ = state.publish(&app).await;
    });
}

/// Runs a `--preset`/`--launch` command; the result goes to the activity log. Returns whether
/// the arguments held a command.
pub fn run_command_line(app: &AppHandle, args: &[String]) -> bool {
    let command = match cli::parse(args) {
        Ok(Some(command)) => command,
        Ok(None) => return false,
        Err(error) => {
            log_later(app, "ERROR", format!("Command line: {error}"));
            return true;
        }
    };
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let state = app.state::<DesktopState>();
        let result = match state.storage.config().await {
            Ok(config) => state.automation.run_cli(command, config).await,
            Err(error) => Err(error),
        };
        let (level, message) = match result {
            Ok(message) => ("INFO", message),
            Err(error) => ("ERROR", format!("Command line: {error}")),
        };
        let _ = state.accounts.log(level, "APP", &message, None).await;
        let _guard = state.mutation.lock().await;
        let _ = state.publish(&app).await;
    });
    true
}

fn log_later(app: &AppHandle, level: &'static str, message: String) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let state = app.state::<DesktopState>();
        let _ = state.accounts.log(level, "APP", &message, None).await;
        let _guard = state.mutation.lock().await;
        let _ = state.publish(&app).await;
    });
}
