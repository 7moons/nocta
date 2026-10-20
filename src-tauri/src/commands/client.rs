//! Nocta's own Roblox install.
use crate::state::DesktopState;
use nocta_core::error::CommandError;
use tauri::{AppHandle, State};

#[tauri::command]
pub async fn client_check_latest(
    app: AppHandle,
    state: State<'_, DesktopState>,
) -> Result<nocta_core::models::LatestClient, CommandError> {
    let latest = state.launcher.client.latest(true).await?;
    let _guard = state.mutation.lock().await;
    state.publish(&app).await?;
    Ok(latest)
}

/// Starts installing a Roblox version (the latest when `guid` is empty) in the background.
/// Progress and failures reach the UI through the snapshot.
#[tauri::command]
pub async fn client_install(
    app: AppHandle,
    state: State<'_, DesktopState>,
    guid: Option<String>,
) -> Result<(), CommandError> {
    let launcher = state.launcher.clone();
    let (guid, version) = match guid.filter(|guid| !guid.trim().is_empty()) {
        Some(guid) => {
            let guid = guid.trim().to_owned();
            nocta_core::launch::client::validate_guid(&guid)?;
            (guid, None)
        }
        None => {
            let latest = launcher.client.latest(true).await?;
            (latest.guid, Some(latest.version))
        }
    };
    tauri::async_runtime::spawn(async move {
        let state = tauri::Manager::state::<DesktopState>(&app);
        let result = launcher.client.install(&guid, version).await;
        let config = state.storage.config().await;
        if let (Ok(_), Ok(config)) = (&result, &config) {
            let running = launcher.instances.running_paths().await;
            launcher.client.cleanup(config, &running);
        }
        let level = if result.is_ok() { "SUCCESS" } else { "ERROR" };
        let message = match &result {
            Ok(client) => format!("Installed Roblox {} for Nocta.", client.guid),
            Err(error) => format!("Roblox {guid} could not be installed: {error}"),
        };
        let _ = state.accounts.log(level, "CLIENT", &message, None).await;
        let _guard = state.mutation.lock().await;
        let _ = state.publish(&app).await;
    });
    Ok(())
}

#[tauri::command]
pub async fn client_cancel_install(state: State<'_, DesktopState>) -> Result<(), CommandError> {
    state.launcher.client.cancel();
    Ok(())
}

#[tauri::command]
pub async fn client_remove(
    app: AppHandle,
    state: State<'_, DesktopState>,
    guid: String,
) -> Result<(), CommandError> {
    let running = state.launcher.instances.running_paths().await;
    let _guard = state.mutation.lock().await;
    let config = state.storage.config().await?;
    state.launcher.client.remove(&guid, &config, &running)?;
    state.publish(&app).await?;
    Ok(())
}

#[tauri::command]
pub async fn client_cleanup(
    app: AppHandle,
    state: State<'_, DesktopState>,
) -> Result<u32, CommandError> {
    let running = state.launcher.instances.running_paths().await;
    let _guard = state.mutation.lock().await;
    let config = state.storage.config().await?;
    let removed = state.launcher.client.cleanup(&config, &running);
    state.publish(&app).await?;
    Ok(removed)
}

/// FastFlags Roblox still reads, for the settings editor.
#[tauri::command]
pub fn allowed_fast_flags() -> Vec<nocta_core::launch::client::fast_flags::AllowedFlag> {
    nocta_core::launch::client::fast_flags::allowed()
}
