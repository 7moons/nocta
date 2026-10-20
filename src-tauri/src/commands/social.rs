//! Friends, online status and avatars from Roblox.
use crate::state::DesktopState;
use nocta_core::{error::CommandError, models::*};
use std::collections::HashMap;
use tauri::{AppHandle, State};
use tokio_util::sync::CancellationToken;

#[tauri::command]
pub async fn friends_of(
    state: State<'_, DesktopState>,
    account_id: u32,
) -> Result<Vec<FriendEntry>, CommandError> {
    Ok(state
        .launcher
        .presence
        .friends_of(account_id, &CancellationToken::new())
        .await?)
}

/// Fetches whether the accounts are online and in which game; returns how many were updated.
#[tauri::command]
pub async fn refresh_presence(
    app: AppHandle,
    state: State<'_, DesktopState>,
) -> Result<u32, CommandError> {
    let updated = state
        .launcher
        .presence
        .refresh_presence(&CancellationToken::new())
        .await?;
    let _guard = state.mutation.lock().await;
    state.publish(&app).await?;
    Ok(updated)
}

/// Headshot URLs keyed by Roblox user ID.
#[tauri::command]
pub async fn user_avatars(
    state: State<'_, DesktopState>,
    user_ids: Vec<String>,
) -> Result<HashMap<String, String>, CommandError> {
    let ids: Vec<u64> = user_ids
        .iter()
        .filter_map(|id| id.parse().ok())
        .take(200)
        .collect();
    Ok(state
        .launcher
        .social
        .avatars(&ids, &CancellationToken::new())
        .await?)
}

/// The Roblox user with this exact username (to check a player to follow).
#[tauri::command]
pub async fn find_user(
    state: State<'_, DesktopState>,
    username: String,
) -> Result<RobloxUser, CommandError> {
    Ok(state
        .launcher
        .social
        .user_by_name(&username, &CancellationToken::new())
        .await?)
}

/// Signs the account out everywhere else and keeps Nocta's session with the new cookie.
#[tauri::command]
pub async fn sign_out_other_sessions(
    app: AppHandle,
    state: State<'_, DesktopState>,
    id: u32,
) -> Result<(), CommandError> {
    state
        .launcher
        .account_tools
        .sign_out_other_sessions(id, &CancellationToken::new())
        .await?;
    let _guard = state.mutation.lock().await;
    state.publish(&app).await?;
    Ok(())
}

#[tauri::command]
pub async fn privacy_settings(
    state: State<'_, DesktopState>,
    id: u32,
) -> Result<Vec<PrivacySetting>, CommandError> {
    Ok(state
        .launcher
        .account_tools
        .privacy_settings(id, &CancellationToken::new())
        .await?)
}

/// Sets one privacy setting on these accounts; returns how many changed.
#[tauri::command]
pub async fn set_privacy(
    app: AppHandle,
    state: State<'_, DesktopState>,
    ids: Vec<u32>,
    key: String,
    value: String,
) -> Result<u32, CommandError> {
    let changed = state
        .launcher
        .account_tools
        .set_privacy(&ids, &key, &value, &CancellationToken::new())
        .await?;
    let _guard = state.mutation.lock().await;
    state.publish(&app).await?;
    Ok(changed)
}

#[tauri::command]
pub async fn set_display_name(
    app: AppHandle,
    state: State<'_, DesktopState>,
    id: u32,
    name: String,
) -> Result<(), CommandError> {
    state
        .launcher
        .account_tools
        .set_display_name(id, &name, &CancellationToken::new())
        .await?;
    let _guard = state.mutation.lock().await;
    state.publish(&app).await?;
    Ok(())
}

/// Which device wants to sign in with this Quick Login code; nothing is approved yet.
#[tauri::command]
pub async fn quick_login_check(
    state: State<'_, DesktopState>,
    id: u32,
    code: String,
) -> Result<QuickLoginDevice, CommandError> {
    Ok(state
        .launcher
        .account_tools
        .quick_login_check(id, &code, &CancellationToken::new())
        .await?)
}

#[tauri::command]
pub async fn quick_login_approve(
    app: AppHandle,
    state: State<'_, DesktopState>,
    id: u32,
    code: String,
) -> Result<(), CommandError> {
    state
        .launcher
        .account_tools
        .quick_login_approve(id, &code, &CancellationToken::new())
        .await?;
    let _guard = state.mutation.lock().await;
    state.publish(&app).await?;
    Ok(())
}

/// Makes the user's own accounts friends with each other or with one main account.
#[tauri::command]
pub async fn befriend_accounts(
    app: AppHandle,
    state: State<'_, DesktopState>,
    ids: Vec<u32>,
    mode: BefriendMode,
) -> Result<BefriendSummary, CommandError> {
    let summary = state
        .launcher
        .account_tools
        .befriend_accounts(&ids, mode, &CancellationToken::new())
        .await?;
    let _guard = state.mutation.lock().await;
    state.publish(&app).await?;
    Ok(summary)
}
