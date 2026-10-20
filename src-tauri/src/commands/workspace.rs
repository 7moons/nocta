//! The app snapshot, settings and backups.
use crate::state::DesktopState;
use nocta_core::{error::CommandError, models::*, CoreError};
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

#[tauri::command]
pub async fn get_snapshot(state: State<'_, DesktopState>) -> Result<AppSnapshot, CommandError> {
    let _guard = state.mutation.lock().await;
    Ok(state.snapshot().await?)
}

#[tauri::command]
pub async fn patch_config(
    app: AppHandle,
    state: State<'_, DesktopState>,
    patch: serde_json::Value,
) -> Result<(), CommandError> {
    let _guard = state.mutation.lock().await;
    let old = state.storage.config().await?;
    let config = state.storage.patched_config(patch).await?;
    if let Some(window) = tauri::Manager::get_webview_window(&app, "main") {
        window
            .set_always_on_top(config.always_on_top)
            .map_err(|error| CoreError::Validation(error.to_string()))?;
    }
    if config.multi_instance != old.multi_instance {
        if let Err(error) = state
            .launcher
            .instances
            .set_multi_instance(config.multi_instance)
        {
            if let Some(window) = tauri::Manager::get_webview_window(&app, "main") {
                let _ = window.set_always_on_top(old.always_on_top);
            }
            return Err(error.into());
        }
    }
    if let Err(error) = state.storage.save_config(&config).await {
        if config.multi_instance != old.multi_instance {
            let _ = state
                .launcher
                .instances
                .set_multi_instance(old.multi_instance);
        }
        if let Some(window) = tauri::Manager::get_webview_window(&app, "main") {
            let _ = window.set_always_on_top(old.always_on_top);
        }
        return Err(error.into());
    }
    state.sessions.configure(&config);
    crate::system::CLOSE_TO_TRAY.store(config.close_to_tray, std::sync::atomic::Ordering::SeqCst);
    let mut problem = None;
    if config.start_with_windows != old.start_with_windows {
        if let Err(error) = crate::system::apply_autostart(&app, config.start_with_windows) {
            problem = Some(error);
        }
    }
    let hotkeys = |config: &AppConfig| {
        let presets: Vec<_> = config
            .launch_presets
            .iter()
            .map(|preset| (&preset.id, &preset.hotkey))
            .collect();
        (config.hotkeys.clone(), format!("{presets:?}"))
    };
    if hotkeys(&config) != hotkeys(&old) {
        if let Err(error) = crate::system::register_hotkeys(&app, &config) {
            problem = Some(error);
        }
    }
    if config.free_window_resize != old.free_window_resize {
        state
            .launcher
            .instances
            .platform()
            .free_resize(config.free_window_resize);
    }
    if nocta_core::launch::fps::FrameRate::cap(&config)
        != nocta_core::launch::fps::FrameRate::cap(&old)
    {
        // The cap is written only while a client starts; whatever is left goes back now.
        if let Err(error) = state.launcher.frame_rate.settle() {
            tracing::warn!(%error, "Frame-rate cap not put back");
        }
    }
    state.publish(&app).await?;
    // The other settings were saved; this one could not take effect.
    match problem {
        Some(error) => Err(error.into()),
        None => Ok(()),
    }
}

#[tauri::command]
pub async fn export_backup(
    app: AppHandle,
    state: State<'_, DesktopState>,
    password: String,
) -> Result<bool, CommandError> {
    let _guard = state.mutation.lock().await;
    let dialog_app = app.clone();
    let path = tauri::async_runtime::spawn_blocking(move || {
        dialog_app
            .dialog()
            .file()
            .add_filter("Encrypted Nocta backup", &["noctabackup"])
            .set_file_name("nocta.noctabackup")
            .blocking_save_file()
    })
    .await
    .map_err(|_| CoreError::Validation("File dialog failed.".into()))?;
    let Some(path) = path else {
        return Ok(false);
    };
    let path = path
        .into_path()
        .map_err(|_| CoreError::Validation("Choose a local file.".into()))?;
    let bytes = state.accounts.export_backup(password).await?;
    if !path
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("noctabackup"))
    {
        return Err(CoreError::Validation(
            "Save the backup with the .noctabackup extension.".into(),
        )
        .into());
    }
    tokio::fs::write(path, bytes)
        .await
        .map_err(CoreError::from)?;
    state
        .accounts
        .log("INFO", "BACKUP", "Encrypted backup exported.", None)
        .await?;
    state.publish(&app).await?;
    Ok(true)
}

#[tauri::command]
pub async fn restore_backup(
    app: AppHandle,
    state: State<'_, DesktopState>,
    password: String,
) -> Result<Option<u32>, CommandError> {
    let _guard = state.mutation.lock().await;
    state.sessions.tasks.ensure_idle(
        &state
            .accounts
            .list()
            .await?
            .iter()
            .map(|account| account.id)
            .collect::<Vec<_>>(),
    )?;
    let dialog_app = app.clone();
    let path = tauri::async_runtime::spawn_blocking(move || {
        dialog_app
            .dialog()
            .file()
            .add_filter("Encrypted Nocta backup", &["noctabackup"])
            .blocking_pick_file()
    })
    .await
    .map_err(|_| CoreError::Validation("File dialog failed.".into()))?;
    let Some(path) = path else {
        return Ok(None);
    };
    let path = path
        .into_path()
        .map_err(|_| CoreError::Validation("Choose a local file.".into()))?;
    if tokio::fs::metadata(&path)
        .await
        .map_err(CoreError::from)?
        .len()
        > 100 * 1024 * 1024
    {
        return Err(CoreError::Validation("Backup exceeds 100 MB.".into()).into());
    }
    let old = state.storage.config().await?;
    let restored = state
        .accounts
        .restore_backup_with_settings(
            tokio::fs::read(path).await.map_err(CoreError::from)?,
            password,
            |config| {
                state
                    .launcher
                    .instances
                    .set_multi_instance(config.multi_instance)?;
                if let Some(window) = tauri::Manager::get_webview_window(&app, "main") {
                    window
                        .set_always_on_top(config.always_on_top)
                        .map_err(|error| CoreError::Validation(error.to_string()))?;
                }
                Ok(())
            },
        )
        .await;
    let count = match restored {
        Ok(count) => count,
        Err(error) => {
            let _ = state
                .launcher
                .instances
                .set_multi_instance(old.multi_instance);
            if let Some(window) = tauri::Manager::get_webview_window(&app, "main") {
                let _ = window.set_always_on_top(old.always_on_top);
            }
            return Err(error.into());
        }
    };
    let config = state.storage.config().await?;
    state.sessions.configure(&config);
    state.publish(&app).await?;
    Ok(Some(count))
}

/// The interface has drawn itself; the safe display fallback is not needed.
#[tauri::command]
pub fn ui_ready() {
    crate::display::ready();
}

/// The website of 7moons, who make Nocta.
const HOMEPAGE: &str = "https://7moons.dev";

/// Opens the makers' website in the default browser. Only this fixed address can be opened, so
/// the interface cannot send the user anywhere else.
#[tauri::command]
pub fn open_homepage() -> Result<(), CommandError> {
    std::process::Command::new("explorer.exe")
        .arg(HOMEPAGE)
        .spawn()
        .map(drop)
        .map_err(|_| CoreError::Validation("The website could not be opened.".into()).into())
}

/// Turns safe display mode on or off and restarts Nocta to apply it.
#[tauri::command]
pub fn set_safe_display(app: AppHandle, on: bool) -> Result<(), CommandError> {
    crate::display::remember(on).map_err(|error| CoreError::Validation(error.to_string()))?;
    app.restart();
}
