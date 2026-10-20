//! Importing accounts from the old Nocta app and from ic3w0lf's Roblox Account Manager.
use crate::state::DesktopState;
use nocta_core::{error::CommandError, models::*, CoreError};
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

/// Roblox Account Manager files in the folders people usually keep it in. Old Nocta databases
/// are picked by the user.
#[tauri::command]
pub async fn legacy_detect() -> Result<Vec<LegacySource>, CommandError> {
    let mut found = Vec::new();
    if let Some(home) = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")) {
        found.extend(nocta_core::ram_import::detect(std::path::Path::new(&home)).await);
    }
    Ok(found)
}

fn is_account_manager_file(path: &std::path::Path) -> bool {
    path.extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
}

/// Lets the user point at an old database file that is not in a usual location.
#[tauri::command]
pub async fn legacy_pick(app: AppHandle) -> Result<Option<LegacySource>, CommandError> {
    let dialog_app = app.clone();
    let path = tauri::async_runtime::spawn_blocking(move || {
        dialog_app
            .dialog()
            .file()
            .add_filter("Account database", &["sqlite", "db", "json"])
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
    Ok(Some(if is_account_manager_file(&path) {
        nocta_core::ram_import::inspect(&path).await?
    } else {
        nocta_core::legacy::inspect(&path, "Selected database").await?
    }))
}

#[tauri::command]
pub async fn legacy_import(
    app: AppHandle,
    webview: tauri::Webview,
    state: State<'_, DesktopState>,
    path: String,
    include_trash: bool,
    password: Option<String>,
    progress: Option<tauri::ipc::JavaScriptChannelId>,
) -> Result<ImportReport, CommandError> {
    let progress = progress.map(|id| id.channel_on::<_, ImportProgress>(webview));
    let path = std::path::Path::new(&path);
    let data = if is_account_manager_file(path) {
        let password = password.map(zeroize::Zeroizing::new);
        nocta_core::ram_import::read(path, password.as_deref().map(String::as_str)).await?
    } else {
        nocta_core::legacy::read(path).await?
    };
    let _guard = state.mutation.lock().await;
    let report = state
        .accounts
        .import_legacy(data, include_trash, |event| {
            if let Some(channel) = &progress {
                let _ = channel.send(event);
            }
        })
        .await?;
    state.publish(&app).await?;
    Ok(report)
}
