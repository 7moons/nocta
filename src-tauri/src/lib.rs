mod commands;
mod display;
mod memory;
mod state;
mod system;

use std::sync::atomic::{AtomicBool, Ordering};
use tauri::Manager;

static SHUTTING_DOWN: AtomicBool = AtomicBool::new(false);

pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter("nocta=info,nocta_core=info,nocta_desktop_lib=info")
        .init();
    let safe_display = display::requested();
    display::apply(safe_display);
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, args, _| {
            // A second start with a command runs it here; a plain one shows the window.
            if !system::run_command_line(app, &args) {
                system::show(app);
            }
        }))
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec![system::AUTOSTART_ARG]),
        ))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    system::on_hotkey(app, shortcut, event.state())
                })
                .build(),
        )
        .plugin(tauri_plugin_dialog::init())
        .plugin({
            // In the data folder, so a portable copy leaves nothing in the user's app data.
            let window_state = tauri_plugin_window_state::Builder::default();
            match display::data_directory() {
                Some(directory) => window_state
                    .with_filename(directory.join(".window-state.json").to_string_lossy()),
                None => window_state,
            }
            .build()
        })
        .invoke_handler(tauri::generate_handler![
            commands::workspace::get_snapshot,
            commands::accounts::import_accounts,
            commands::accounts::update_account,
            commands::accounts::account_action,
            commands::accounts::save_folder,
            commands::accounts::delete_folder,
            commands::accounts::move_folder,
            commands::accounts::rename_folder_group,
            commands::workspace::patch_config,
            commands::accounts::copy_accounts,
            commands::workspace::export_backup,
            commands::workspace::restore_backup,
            commands::accounts::start_session_tasks,
            commands::accounts::stop_tasks,
            commands::accounts::cancel_task,
            commands::launch::resolve_launch_target,
            commands::launch::list_servers,
            commands::launch::cancel_server_list,
            commands::launch::get_place_info,
            commands::launch::search_games,
            commands::launch::expand_invite_link,
            commands::launch::launch_game,
            commands::launch::cancel_launch,
            commands::launch::list_instances,
            commands::launch::close_instance,
            commands::launch::close_all_owned_instances,
            commands::launch::close_external_instance,
            commands::launch::close_all_roblox,
            commands::legacy::legacy_detect,
            commands::legacy::legacy_pick,
            commands::legacy::legacy_import,
            commands::client::client_check_latest,
            commands::client::client_install,
            commands::client::client_cancel_install,
            commands::client::client_remove,
            commands::client::client_cleanup,
            commands::accounts::update_account_details,
            commands::accounts::set_totp,
            commands::accounts::totp_code,
            commands::accounts::export_accounts,
            commands::accounts::sign_in_account,
            commands::social::friends_of,
            commands::social::refresh_presence,
            commands::social::user_avatars,
            commands::social::find_user,
            commands::automation::run_preset,
            commands::automation::test_webhook,
            commands::accounts::refresh_robux,
            commands::client::allowed_fast_flags,
            commands::launch::arrange_windows,
            commands::launch::system_memory,
            commands::workspace::ui_ready,
            commands::workspace::open_homepage,
            commands::workspace::set_safe_display,
            commands::accounts::reorder_accounts,
            commands::social::sign_out_other_sessions,
            commands::social::privacy_settings,
            commands::social::set_privacy,
            commands::social::set_display_name,
            commands::social::quick_login_check,
            commands::social::quick_login_approve,
            commands::social::befriend_accounts,
        ])
        .setup(move |app| {
            let mut state =
                tauri::async_runtime::block_on(state::DesktopState::open(app.handle()))?;
            state.runtime.safe_display = safe_display;
            display::watch(app.handle().clone(), safe_display);
            let config = tauri::async_runtime::block_on(state.storage.config())?;
            // Background work for auto-rejoin and schedules; spawned inside the runtime.
            tauri::async_runtime::block_on(async {
                nocta_core::launch::rejoin::Rejoin::new(state.launcher.clone()).spawn();
                state.automation.spawn_scheduler();
                state.client_care.spawn_upkeep();
                state
                    .launcher
                    .instances
                    .platform()
                    .free_resize(config.free_window_resize);
            });
            system::CLOSE_TO_TRAY.store(config.close_to_tray, Ordering::SeqCst);
            app.manage(state);
            let updates = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                loop {
                    let state = updates.state::<state::DesktopState>();
                    tokio::select! {
                        _ = state.sessions.tasks.changed.notified() => {},
                        _ = state.launcher.instances.changed.notified() => {},
                        _ = state.launcher.client.changed.notified() => {},
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                    // Without the command lock: commands are not held up by progress updates.
                    if SHUTTING_DOWN.load(Ordering::SeqCst) {
                        break;
                    }
                    if let Err(error) = state.publish(&updates).await {
                        tracing::warn!(%error, "Task update delivery failed");
                    }
                }
            });
            system::build_tray(app.handle())?;
            if let Err(error) = system::register_hotkeys(app.handle(), &config) {
                tracing::warn!(%error, "Shortcuts not registered");
            }
            let args: Vec<String> = std::env::args().collect();
            // Started by Windows at sign-in: stay in the tray when the user asked for that.
            let hidden =
                args.iter().any(|arg| arg == system::AUTOSTART_ARG) && config.start_minimized;
            if let Some(window) = app.get_webview_window("main") {
                window.set_decorations(false)?;
                window.set_always_on_top(config.always_on_top)?;
                if !hidden {
                    window.show()?;
                }
                memory::follow_visibility(&window);
            }
            system::run_command_line(app.handle(), &args);
            Ok(())
        })
        .on_window_event(|window, event| {
            // Minimizing, restoring and hiding change how much memory the WebView may keep.
            if matches!(
                event,
                tauri::WindowEvent::Resized(_) | tauri::WindowEvent::Focused(_)
            ) {
                if let Some(main) = window.app_handle().get_webview_window("main") {
                    memory::follow_visibility(&main);
                }
            }
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if SHUTTING_DOWN.load(Ordering::SeqCst) {
                    return;
                }
                api.prevent_close();
                if system::CLOSE_TO_TRAY.load(Ordering::SeqCst) {
                    let _ = window.hide();
                    if let Some(main) = window.app_handle().get_webview_window("main") {
                        memory::follow_visibility(&main);
                    }
                    return;
                }
                quit(window.app_handle().clone());
            }
        })
        .run(tauri::generate_context!())
        .expect("Nocta failed to start");
}

/// Stops everything Nocta runs and exits. Game clients keep running.
pub(crate) fn quit(app: tauri::AppHandle) {
    if SHUTTING_DOWN.swap(true, Ordering::SeqCst) {
        return;
    }
    tauri::async_runtime::spawn(async move {
        let state = app.state::<state::DesktopState>();
        let _guard = state.mutation.lock().await;
        state.sessions.tasks.shutdown();
        state.launcher.shutdown().await;
        state.monitor_stop.cancel();
        if let Some(monitor) = state.monitor.lock().await.take() {
            let _ = monitor.await;
        }
        state.sessions.shutdown().await;
        state.storage.close().await;
        app.exit(0);
    });
}
