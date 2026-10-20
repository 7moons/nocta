//! Launch targets, servers, game info and running Roblox clients.
use crate::state::DesktopState;
use nocta_core::{error::CommandError, models::*, CoreError};
use tauri::{AppHandle, State};

#[tauri::command]
pub async fn resolve_launch_target(
    state: State<'_, DesktopState>,
    account_id: Option<u32>,
    target: LaunchTargetInput,
) -> Result<LaunchTargetSummary, CommandError> {
    Ok(state
        .launcher
        .resolve_summary(account_id, &target, &state.storage.config().await?)
        .await?)
}

/// Direct game link for a pasted invite: share links are expanded (and remembered) with any
/// signed-in account; other links come back unchanged.
#[tauri::command]
pub async fn expand_invite_link(
    state: State<'_, DesktopState>,
    url: String,
) -> Result<String, CommandError> {
    let parsed = state.launcher.web.validate_invite_url(&url)?;
    Ok(state
        .launcher
        .expand_share_link(&parsed, None, &tokio_util::sync::CancellationToken::new())
        .await?
        .unwrap_or(url))
}

#[tauri::command]
pub async fn list_servers(
    state: State<'_, DesktopState>,
    request_key: String,
    place_id: String,
    account_id: Option<u32>,
    include_vip: bool,
    options: Option<nocta_core::models::ServerListOptions>,
) -> Result<ServerListResponse, CommandError> {
    Ok(state
        .launcher
        .list_servers(
            &request_key,
            &place_id,
            account_id,
            include_vip,
            &options.unwrap_or_default(),
        )
        .await?)
}

#[tauri::command]
pub async fn cancel_server_list(
    state: State<'_, DesktopState>,
    request_key: String,
) -> Result<(), CommandError> {
    state.launcher.servers.cancel(&request_key);
    Ok(())
}

#[tauri::command]
pub async fn get_place_info(
    state: State<'_, DesktopState>,
    place_id: String,
) -> Result<PlaceInfo, CommandError> {
    Ok(state
        .launcher
        .places
        .get(&place_id, &tokio_util::sync::CancellationToken::new())
        .await?)
}

#[tauri::command]
pub async fn search_games(
    state: State<'_, DesktopState>,
    query: String,
) -> Result<Vec<GameSearchResult>, CommandError> {
    Ok(state
        .launcher
        .places
        .search(&query, &tokio_util::sync::CancellationToken::new())
        .await?)
}

#[tauri::command]
pub async fn launch_game(
    app: AppHandle,
    state: State<'_, DesktopState>,
    ids: Vec<u32>,
    target: LaunchTargetInput,
) -> Result<Vec<u32>, CommandError> {
    let _guard = state.mutation.lock().await;
    let tasks = state
        .launcher
        .start(&ids, target, state.storage.config().await?)
        .await?;
    state.publish(&app).await?;
    Ok(tasks)
}

#[tauri::command]
pub async fn cancel_launch(
    app: AppHandle,
    state: State<'_, DesktopState>,
    id: u32,
) -> Result<(), CommandError> {
    if state
        .sessions
        .tasks
        .get(id)
        .ok_or(CoreError::NotFound)?
        .kind
        != TaskKind::LaunchGame
    {
        return Err(CoreError::Validation("This task is not a game launch.".into()).into());
    }
    state.sessions.tasks.cancel(id)?;
    let _guard = state.mutation.lock().await;
    state.publish(&app).await?;
    Ok(())
}

/// Puts the windows of running clients in a grid; returns how many were placed.
#[tauri::command]
pub async fn system_memory(
    state: State<'_, DesktopState>,
) -> Result<Option<SystemMemory>, CommandError> {
    Ok(state.launcher.instances.system_memory())
}

#[tauri::command]
pub async fn arrange_windows(state: State<'_, DesktopState>) -> Result<u32, CommandError> {
    let config = state.storage.config().await?;
    Ok(state.client_care.arrange_windows(&config).await)
}

#[tauri::command]
pub async fn list_instances(
    state: State<'_, DesktopState>,
) -> Result<Vec<GameInstance>, CommandError> {
    Ok(state.launcher.instances.snapshot())
}

#[tauri::command]
pub async fn close_instance(
    app: AppHandle,
    state: State<'_, DesktopState>,
    id: String,
) -> Result<(), CommandError> {
    state.launcher.instances.close(&id).await?;
    let _guard = state.mutation.lock().await;
    state.publish(&app).await?;
    Ok(())
}

#[tauri::command]
pub async fn close_all_owned_instances(
    app: AppHandle,
    state: State<'_, DesktopState>,
) -> Result<u32, CommandError> {
    let closed = state.launcher.instances.close_all().await?;
    let _guard = state.mutation.lock().await;
    state.publish(&app).await?;
    Ok(closed)
}

#[tauri::command]
pub async fn close_external_instance(
    app: AppHandle,
    state: State<'_, DesktopState>,
    id: String,
) -> Result<(), CommandError> {
    state.launcher.instances.close_external(&id).await?;
    let _guard = state.mutation.lock().await;
    state.publish(&app).await?;
    Ok(())
}

/// Closes every Roblox client, including ones Nocta did not start, then retries multi-instance
/// when it is enabled, since a running client is what keeps its singleton lock busy.
#[tauri::command]
pub async fn close_all_roblox(
    app: AppHandle,
    state: State<'_, DesktopState>,
) -> Result<u32, CommandError> {
    let closed = state.launcher.instances.close_all_roblox().await?;
    let _guard = state.mutation.lock().await;
    let retry = if state.storage.config().await?.multi_instance {
        state.launcher.instances.set_multi_instance(true).err()
    } else {
        None
    };
    state.publish(&app).await?;
    match retry {
        Some(error) => Err(error.into()),
        None => Ok(closed),
    }
}
