//! Launch presets and the Discord webhook. Schedules run in the background on their own.
use crate::state::DesktopState;
use nocta_core::{error::CommandError, CoreError};
use tauri::{AppHandle, State};

#[tauri::command]
pub async fn run_preset(
    app: AppHandle,
    state: State<'_, DesktopState>,
    id: String,
) -> Result<Vec<u32>, CommandError> {
    let _guard = state.mutation.lock().await;
    let config = state.storage.config().await?;
    let preset = config
        .launch_presets
        .iter()
        .find(|preset| preset.id == id)
        .cloned()
        .ok_or(CoreError::NotFound)?;
    let tasks = state.automation.run_preset(&preset, config).await?;
    state.publish(&app).await?;
    Ok(tasks)
}

#[tauri::command]
pub async fn test_webhook(state: State<'_, DesktopState>, url: String) -> Result<(), CommandError> {
    Ok(state.launcher.test_webhook(&url).await?)
}
