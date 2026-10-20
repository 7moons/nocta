//! Accounts and folders, and the session tasks run on accounts.
use crate::state::DesktopState;
use nocta_core::{accounts::ExportFormat, error::CommandError, models::*, CoreError};
use tauri::{AppHandle, State};

#[tauri::command]
pub async fn import_accounts(
    app: AppHandle,
    webview: tauri::Webview,
    state: State<'_, DesktopState>,
    text: String,
    folder_id: Option<u32>,
    update_existing: bool,
    progress: Option<tauri::ipc::JavaScriptChannelId>,
) -> Result<ImportReport, CommandError> {
    let progress = progress.map(|id| id.channel_on::<_, ImportProgress>(webview));
    let _guard = state.mutation.lock().await;
    if update_existing {
        state.sessions.tasks.ensure_idle(
            &state
                .accounts
                .list()
                .await?
                .iter()
                .map(|account| account.id)
                .collect::<Vec<_>>(),
        )?;
    }
    let report = state
        .accounts
        .import_with_progress(text, folder_id, update_existing, |event| {
            if let Some(channel) = &progress {
                let _ = channel.send(event);
            }
        })
        .await?;
    state.publish(&app).await?;
    Ok(report)
}

#[tauri::command]
pub async fn update_account(
    app: AppHandle,
    state: State<'_, DesktopState>,
    id: u32,
    input: AccountUpdate,
    validate_session: Option<bool>,
) -> Result<(), CommandError> {
    let _guard = state.mutation.lock().await;
    state.sessions.tasks.ensure_idle(&[id])?;
    state.accounts.update(id, input).await?;
    state.launcher.resolver.invalidate_account(id);
    if validate_session.unwrap_or(false) {
        state
            .sessions
            .start(
                &[id],
                TaskKind::ValidateCookie,
                state.storage.config().await?,
            )
            .await?;
    }
    state.publish(&app).await?;
    Ok(())
}

#[tauri::command]
pub async fn account_action(
    app: AppHandle,
    state: State<'_, DesktopState>,
    ids: Vec<u32>,
    action: String,
    folder_id: Option<u32>,
) -> Result<(), CommandError> {
    let _guard = state.mutation.lock().await;
    state.sessions.tasks.ensure_idle(&ids)?;
    state.accounts.action(&ids, &action, folder_id).await?;
    state.publish(&app).await?;
    Ok(())
}

#[tauri::command]
pub async fn save_folder(
    app: AppHandle,
    state: State<'_, DesktopState>,
    id: Option<u32>,
    input: FolderInput,
) -> Result<(), CommandError> {
    let _guard = state.mutation.lock().await;
    state.accounts.save_folder(id, input).await?;
    state.publish(&app).await?;
    Ok(())
}

#[tauri::command]
pub async fn delete_folder(
    app: AppHandle,
    state: State<'_, DesktopState>,
    id: u32,
) -> Result<(), CommandError> {
    let _guard = state.mutation.lock().await;
    state.accounts.remove_folder(id).await?;
    state.publish(&app).await?;
    Ok(())
}

#[tauri::command]
pub async fn copy_accounts(
    state: State<'_, DesktopState>,
    ids: Vec<u32>,
    format: String,
) -> Result<String, CommandError> {
    let _guard = state.mutation.lock().await;
    Ok(state.accounts.copy(&ids, &format).await?)
}

#[tauri::command]
pub async fn move_folder(
    app: AppHandle,
    state: State<'_, DesktopState>,
    id: u32,
    group: Option<String>,
    before: Option<u32>,
) -> Result<(), CommandError> {
    let _guard = state.mutation.lock().await;
    state.accounts.move_folder(id, group, before).await?;
    state.publish(&app).await?;
    Ok(())
}

#[tauri::command]
pub async fn rename_folder_group(
    app: AppHandle,
    state: State<'_, DesktopState>,
    from: String,
    to: Option<String>,
) -> Result<(), CommandError> {
    let _guard = state.mutation.lock().await;
    state.accounts.rename_folder_group(&from, to).await?;
    state.publish(&app).await?;
    Ok(())
}

#[tauri::command]
pub async fn start_session_tasks(
    app: AppHandle,
    state: State<'_, DesktopState>,
    ids: Vec<u32>,
    kind: TaskKind,
) -> Result<Vec<u32>, CommandError> {
    let _guard = state.mutation.lock().await;
    let tasks = state
        .sessions
        .start(&ids, kind, state.storage.config().await?)
        .await?;
    state.publish(&app).await?;
    Ok(tasks)
}

#[tauri::command]
pub async fn stop_tasks(
    app: AppHandle,
    state: State<'_, DesktopState>,
) -> Result<(), CommandError> {
    let stopped = state.sessions.tasks.stop(false);
    let _guard = state.mutation.lock().await;
    if stopped > 0 {
        let message = format!("Stopped {stopped} running or queued operations.");
        state
            .accounts
            .log("INFO", "SESSIONS", &message, None)
            .await?;
    }
    state.publish(&app).await?;
    Ok(())
}

#[tauri::command]
pub async fn cancel_task(
    app: AppHandle,
    state: State<'_, DesktopState>,
    id: u32,
) -> Result<(), CommandError> {
    state.sessions.tasks.cancel(id)?;
    let _guard = state.mutation.lock().await;
    state.publish(&app).await?;
    Ok(())
}

#[tauri::command]
pub async fn update_account_details(
    app: AppHandle,
    state: State<'_, DesktopState>,
    id: u32,
    details: AccountDetails,
) -> Result<(), CommandError> {
    let _guard = state.mutation.lock().await;
    state.accounts.update_details(id, details).await?;
    state.publish(&app).await?;
    Ok(())
}

/// Saves the authenticator key of an account, or removes it when `key` is null.
#[tauri::command]
pub async fn set_totp(
    app: AppHandle,
    state: State<'_, DesktopState>,
    id: u32,
    key: Option<String>,
) -> Result<(), CommandError> {
    let _guard = state.mutation.lock().await;
    state.accounts.set_totp(id, key).await?;
    state.publish(&app).await?;
    Ok(())
}

#[tauri::command]
pub async fn totp_code(state: State<'_, DesktopState>, id: u32) -> Result<TotpCode, CommandError> {
    Ok(state.accounts.totp_code(id).await?)
}

/// Saves the accounts with their credentials to a CSV or JSON file the user picks.
#[tauri::command]
pub async fn export_accounts(
    app: AppHandle,
    state: State<'_, DesktopState>,
    ids: Vec<u32>,
    format: ExportFormat,
) -> Result<bool, CommandError> {
    use tauri_plugin_dialog::DialogExt;
    let (label, extension) = match format {
        ExportFormat::Csv => ("CSV", "csv"),
        ExportFormat::Json => ("JSON", "json"),
    };
    let dialog_app = app.clone();
    let path = tauri::async_runtime::spawn_blocking(move || {
        dialog_app
            .dialog()
            .file()
            .add_filter(label, &[extension])
            .set_file_name(format!("nocta-accounts.{extension}"))
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
    let _guard = state.mutation.lock().await;
    let text = state.accounts.export(&ids, format).await?;
    tokio::fs::write(&path, text.as_bytes())
        .await
        .map_err(CoreError::from)?;
    state
        .accounts
        .log(
            "INFO",
            "EXPORT",
            &format!("Exported {} accounts with their credentials.", ids.len()),
            None,
        )
        .await?;
    Ok(true)
}

/// Opens Roblox's login page in a browser and saves the signed-in session as an account.
/// Returns the account ID once the user has signed in.
#[tauri::command]
pub async fn sign_in_account(
    app: AppHandle,
    state: State<'_, DesktopState>,
    folder_id: Option<u32>,
) -> Result<u32, CommandError> {
    let config = state.storage.config().await?;
    let id = state
        .sessions
        .sign_in_new(
            folder_id,
            &config,
            &tokio_util::sync::CancellationToken::new(),
        )
        .await?;
    let _guard = state.mutation.lock().await;
    state.publish(&app).await?;
    Ok(id)
}

/// Checks the Robux balance of these accounts with their own sessions.
#[tauri::command]
pub async fn refresh_robux(
    app: AppHandle,
    state: State<'_, DesktopState>,
    ids: Vec<u32>,
) -> Result<u32, CommandError> {
    let updated = state
        .launcher
        .presence
        .refresh_robux(&ids, &tokio_util::sync::CancellationToken::new())
        .await?;
    let _guard = state.mutation.lock().await;
    state.publish(&app).await?;
    Ok(updated)
}

/// Saves the user's own order of the accounts, first to last.
#[tauri::command]
pub async fn reorder_accounts(
    app: AppHandle,
    state: State<'_, DesktopState>,
    ids: Vec<u32>,
) -> Result<(), CommandError> {
    let _guard = state.mutation.lock().await;
    state.accounts.reorder(&ids).await?;
    state.publish(&app).await?;
    Ok(())
}
