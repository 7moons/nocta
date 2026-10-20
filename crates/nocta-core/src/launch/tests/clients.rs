//! Running clients: pacing launches by the client log, naming, profiles, window grid, keeping the
//! computer awake, and restarting lost, frozen or long-running clients.
use super::{fixture, wait_terminal};
use crate::launch::*;
use crate::{
    launch::{
        client_care::Upkeep,
        instances::Monitor,
        web::fixture::{response, serve},
    },
    models::{
        AccountDetails, ClientProfile, ClientStage, InstanceStatus, LaunchPacing, ProcessPriority,
    },
};
use std::{
    sync::atomic::Ordering,
    time::{Duration, Instant},
};

/// The responses of one launch of the account called `name`.
fn launch_as(name: &str) -> Vec<String> {
    vec![
        response(
            "200 OK",
            &[("Content-Type", "application/json")],
            &format!(r#"{{"id":1,"name":"{name}"}}"#),
        ),
        response("403 Forbidden", &[("x-csrf-token", "join-csrf")], ""),
        response("200 OK", &[], ""),
        response("403 Forbidden", &[("x-csrf-token", "ticket-csrf")], ""),
        response(
            "200 OK",
            &[("rbx-authentication-ticket", "synthetic-ticket")],
            "",
        ),
    ]
}

/// Writes a Roblox player log the way the client does.
fn write_client_log(dir: &std::path::Path, name: &str, lines: &[&str]) {
    let text: String = lines.iter().map(|line| format!("{line}\n")).collect();
    std::fs::write(
        dir.join(format!("0.741_{name}_Player_{name}_last.log")),
        text,
    )
    .unwrap();
}

const LOG_JOIN: &str = "2026-10-16T15:03:03.639Z,126.6,4f64,6 [FLog::Output] ! Joining game '02bf9043-b6e2-4e75-960d-cde512773ccb' place 920587237 at 10.17.8.212";
const LOG_IN_GAME: &str = "2026-10-16T15:03:03.732Z,126.7,4f64,6,Info [DFLog::NetworkClient] Connection accepted from 128.116.21.33|59133";
const LOG_LOST: &str = "2026-10-16T17:52:48.872Z,10311.8,3874,6,Info [DFLog::RbxTransportDummyClient] Disconnected from server for reason: Server: 277 (DisconnectConnectionLost)";

async fn until(what: &str, check: impl Fn() -> bool) {
    for _ in 0..1000 {
        if check() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("timed out waiting for {what}");
}

fn details(alias: &str, is_main: bool) -> AccountDetails {
    AccountDetails {
        alias: alias.into(),
        note: String::new(),
        color: None,
        pinned: false,
        is_main,
        fields: Vec::new(),
        afk_point: None,
    }
}

fn base_config(pacing: LaunchPacing) -> AppConfig {
    AppConfig {
        place_id: "920587237".into(),
        multi_instance: false,
        pre_launch_delay_secs: 0,
        bulk_action_delay_secs: 0,
        launch_pacing: pacing,
        ..Default::default()
    }
}

#[tokio::test]
async fn the_next_account_starts_once_the_previous_client_is_in_game() {
    let mut responses = launch_as("Alice");
    responses.extend(launch_as("Bob"));
    let (base, server) = serve(responses).await;
    let (launcher, platform, accounts) = fixture(&base).await;
    accounts
        .import("Bob:pw:synthetic-cookie-2".into(), None, false)
        .await
        .unwrap();
    let logs = tempfile::tempdir().unwrap();
    launcher
        .client_logs
        .set_dir(Some(logs.path().to_path_buf()));
    let config = base_config(LaunchPacing::InGame);
    accounts.save_config(&config).await.unwrap();
    for _ in 0..2 {
        let process = platform.process("RobloxPlayerBeta.exe", true, 1);
        platform.spawn_on_dispatch.lock().unwrap().push(process);
    }
    let tasks = launcher
        .start(&[1, 2], LaunchTargetInput::Configured, config)
        .await
        .unwrap();
    until("the first launch", || {
        platform.dispatched.lock().unwrap().len() == 1
    })
    .await;

    // Alice is still loading: Bob waits instead of starting after a fixed pause.
    tokio::time::sleep(Duration::from_millis(2500)).await;
    assert_eq!(platform.dispatched.lock().unwrap().len(), 1);
    let waiting = launcher.sessions.tasks.get(tasks[1]).unwrap();
    assert_eq!(
        waiting.detail,
        "Waiting for the previous account to join its game."
    );

    // Her log says she joined; Bob starts right after.
    write_client_log(logs.path(), "ALICE", &[LOG_JOIN, LOG_IN_GAME]);
    until("the second launch", || {
        platform.dispatched.lock().unwrap().len() == 2
    })
    .await;
    wait_terminal(&launcher, tasks[1]).await;
    let alice = launcher
        .instances
        .snapshot()
        .into_iter()
        .find(|instance| instance.account_id == 1)
        .unwrap();
    assert_eq!(alice.stage, ClientStage::InGame);
    server.await.unwrap();
}

#[tokio::test]
async fn running_clients_are_named_profiled_arranged_and_restarted_when_lost_or_frozen() {
    let mut responses = launch_as("Alice");
    responses.extend(launch_as("Bob"));
    let (base, server) = serve(responses).await;
    let (launcher, platform, accounts) = fixture(&base).await;
    accounts
        .import("Bob:pw:synthetic-cookie-2".into(), None, false)
        .await
        .unwrap();
    accounts
        .update_details(1, details("Main", true))
        .await
        .unwrap();
    let logs = tempfile::tempdir().unwrap();
    launcher
        .client_logs
        .set_dir(Some(logs.path().to_path_buf()));
    *platform.monitors.lock().unwrap() = vec![Monitor {
        x: 0,
        y: 0,
        width: 1920,
        height: 1040,
        primary: true,
    }];
    let config = AppConfig {
        anti_afk: true,
        auto_arrange: true,
        alt_profile: ClientProfile {
            priority: ProcessPriority::Idle,
            eco_mode: true,
            cpu_limit_percent: 20,
            window_width: 800,
            window_height: 600,
            low_memory_priority: true,
            trim_every_secs: 30,
            ram_cap_mb: 400,
        },
        // Main accounts give memory back rarely, but once right after joining a game.
        main_profile: ClientProfile {
            trim_every_secs: 600,
            ..ClientProfile::default()
        },
        bulk_action_delay_secs: 1,
        ..base_config(LaunchPacing::Fixed)
    };
    accounts.save_config(&config).await.unwrap();
    let alice_process = platform.process("RobloxPlayerBeta.exe", true, 1);
    let bob_process = platform.process("RobloxPlayerBeta.exe", true, 1);
    platform
        .spawn_on_dispatch
        .lock()
        .unwrap()
        .push(alice_process.clone());
    let tasks = launcher
        .start(&[1, 2], LaunchTargetInput::Configured, config)
        .await
        .unwrap();
    // Each client appears after its own launch.
    until("the first launch", || {
        platform.dispatched.lock().unwrap().len() == 1
    })
    .await;
    platform
        .spawn_on_dispatch
        .lock()
        .unwrap()
        .push(bob_process.clone());
    wait_terminal(&launcher, tasks[1]).await;
    write_client_log(logs.path(), "ALICE", &[LOG_JOIN, LOG_IN_GAME]);
    platform
        .memory
        .lock()
        .unwrap()
        .extend([(alice_process.pid, 1500), (bob_process.pid, 700)]);

    let mut state = Upkeep::default();
    let t0 = Instant::now();
    let at = |secs: u64| t0 + Duration::from_secs(secs);
    crate::launch::client_care::ClientCare::new(launcher.clone())
        .upkeep(&mut state, at(0))
        .await;
    let titles = platform.titles.lock().unwrap().clone();
    assert_eq!(titles[&alice_process.pid], "Roblox · Main (Alice)");
    assert_eq!(titles[&bob_process.pid], "Roblox · Bob");
    let profiles = platform.profiles.lock().unwrap().clone();
    assert_eq!(profiles.len(), 2);
    let profile_of = |pid: u32| profiles.iter().find(|(owner, _)| *owner == pid).unwrap().1;
    assert_eq!(profile_of(bob_process.pid).priority, ProcessPriority::Idle);
    assert_eq!(
        profile_of(alice_process.pid).trim_every_secs,
        600,
        "main accounts use the main profile"
    );
    assert!(profile_of(bob_process.pid).low_memory_priority);
    assert!(!profile_of(alice_process.pid).low_memory_priority);
    // Alice joined her game, so she gave back the memory loading left; Bob is still loading.
    assert_eq!(*platform.trimmed.lock().unwrap(), vec![alice_process.pid]);
    assert!(
        platform.resized.lock().unwrap().is_empty(),
        "with auto-arrange the grid sizes the windows, not the profile"
    );
    assert_eq!(
        platform.placed.lock().unwrap().len(),
        2,
        "both windows arranged"
    );
    assert!(
        platform.awake.load(Ordering::SeqCst),
        "kept awake while anti-AFK runs"
    );
    let stage_of = |account: u32| {
        launcher
            .instances
            .snapshot()
            .into_iter()
            .find(|instance| instance.account_id == account)
            .unwrap()
    };
    assert_eq!(stage_of(1).stage, ClientStage::InGame);
    assert_eq!(
        stage_of(1).memory_mb,
        Some(1500),
        "memory is shown per client"
    );
    assert_eq!(stage_of(2).memory_mb, Some(700));

    // Roblox drops Bob; after a short grace his client is closed.
    write_client_log(logs.path(), "BOB", &[LOG_JOIN, LOG_LOST]);
    crate::launch::client_care::ClientCare::new(launcher.clone())
        .upkeep(&mut state, at(15))
        .await;
    let bob = stage_of(2);
    assert_eq!(bob.stage, ClientStage::Disconnected);
    assert_eq!(
        bob.disconnect.as_deref(),
        Some("277: the connection was lost (DisconnectConnectionLost)")
    );
    crate::launch::client_care::ClientCare::new(launcher.clone())
        .upkeep(&mut state, at(40))
        .await;
    // Bob's profile gives memory back every 30 seconds; Alice's only every ten minutes.
    assert_eq!(
        *platform.trimmed.lock().unwrap(),
        vec![alice_process.pid, bob_process.pid]
    );
    assert!(platform
        .killed
        .lock()
        .unwrap()
        .iter()
        .any(|(pid, _)| *pid == bob_process.pid));

    // Alice's window stops responding; after four passes in a row it is closed too.
    platform.hung.lock().unwrap().insert(alice_process.pid);
    for pass in 0..4 {
        crate::launch::client_care::ClientCare::new(launcher.clone())
            .upkeep(&mut state, at(60 + pass * 15))
            .await;
    }
    assert!(platform
        .killed
        .lock()
        .unwrap()
        .iter()
        .any(|(pid, _)| *pid == alice_process.pid));
    crate::launch::client_care::ClientCare::new(launcher.clone())
        .upkeep(&mut state, at(200))
        .await;
    assert!(
        !platform.awake.load(Ordering::SeqCst),
        "no clients, no keeping awake"
    );
    let text = serde_json::to_string(&accounts.logs().await.unwrap()).unwrap();
    assert!(
        text.contains("Roblox client was disconnected (277: the connection was lost"),
        "{text}"
    );
    assert!(text.contains("Roblox client stopped responding"), "{text}");
    server.await.unwrap();
}

#[tokio::test]
async fn alts_restart_on_a_timer_but_main_accounts_keep_running() {
    let mut responses = launch_as("Alice");
    responses.extend(launch_as("Bob"));
    responses.extend(launch_as("Bob"));
    let (base, server) = serve(responses).await;
    let (launcher, platform, accounts) = fixture(&base).await;
    accounts
        .import("Bob:pw:synthetic-cookie-2".into(), None, false)
        .await
        .unwrap();
    accounts.update_details(1, details("", true)).await.unwrap();
    let config = AppConfig {
        restart_alts_minutes: 30,
        bulk_action_delay_secs: 1,
        ..base_config(LaunchPacing::Fixed)
    };
    accounts.save_config(&config).await.unwrap();
    // Each client appears after its own launch: Alice, Bob, then Bob again after the restart.
    let spawn = || {
        let process = platform.process("RobloxPlayerBeta.exe", true, 1);
        platform.spawn_on_dispatch.lock().unwrap().push(process);
    };
    spawn();
    let tasks = launcher
        .start(&[1, 2], LaunchTargetInput::Configured, config)
        .await
        .unwrap();
    until("the first launch", || {
        platform.dispatched.lock().unwrap().len() == 1
    })
    .await;
    spawn();
    wait_terminal(&launcher, tasks[1]).await;
    spawn();
    let mut state = Upkeep::default();
    let t0 = Instant::now();
    crate::launch::client_care::ClientCare::new(launcher.clone())
        .upkeep(&mut state, t0)
        .await;
    assert!(
        platform.killed.lock().unwrap().is_empty(),
        "nothing is old enough yet"
    );
    for instance in launcher.instances.snapshot() {
        launcher.instances.backdate(&instance.id, 31 * 60);
    }
    crate::launch::client_care::ClientCare::new(launcher.clone())
        .upkeep(&mut state, t0 + Duration::from_secs(15))
        .await;
    assert_eq!(
        platform.killed.lock().unwrap().len(),
        1,
        "only the alt restarts"
    );
    until("the alt's new client", || {
        platform.dispatched.lock().unwrap().len() == 3
    })
    .await;
    let running: Vec<u32> = launcher
        .instances
        .snapshot()
        .into_iter()
        .filter(|instance| instance.status != InstanceStatus::Exited)
        .map(|instance| instance.account_id)
        .collect();
    assert!(running.contains(&1) && running.contains(&2), "{running:?}");
    let text = serde_json::to_string(&accounts.logs().await.unwrap()).unwrap();
    assert!(
        text.contains("Roblox client ran for 30 minutes; restarting it."),
        "{text}"
    );
    server.await.unwrap();
}

#[tokio::test]
async fn window_sizes_follow_the_profile_live_down_to_a_single_pixel() {
    let (base, server) = serve(launch_as("Alice")).await;
    let (launcher, platform, accounts) = fixture(&base).await;
    let mut config = AppConfig {
        alt_profile: ClientProfile {
            window_width: 1,
            window_height: 1,
            ..ClientProfile::default()
        },
        ..base_config(LaunchPacing::Fixed)
    };
    accounts.save_config(&config).await.unwrap();
    let process = platform.process("RobloxPlayerBeta.exe", true, 1);
    platform
        .spawn_on_dispatch
        .lock()
        .unwrap()
        .push(process.clone());
    let tasks = launcher
        .start(&[1], LaunchTargetInput::Configured, config.clone())
        .await
        .unwrap();
    wait_terminal(&launcher, tasks[0]).await;
    let mut state = Upkeep::default();
    let t0 = Instant::now();
    let at = |secs: u64| t0 + Duration::from_secs(secs);
    crate::launch::client_care::ClientCare::new(launcher.clone())
        .upkeep(&mut state, at(0))
        .await;
    assert_eq!(*platform.resized.lock().unwrap(), vec![(process.pid, 1, 1)]);
    crate::launch::client_care::ClientCare::new(launcher.clone())
        .upkeep(&mut state, at(15))
        .await;
    assert_eq!(platform.resized.lock().unwrap().len(), 1, "not every pass");

    // A changed setting reaches the running client right away.
    config.alt_profile.window_width = 160;
    config.alt_profile.window_height = 90;
    accounts.save_config(&config).await.unwrap();
    crate::launch::client_care::ClientCare::new(launcher.clone())
        .upkeep(&mut state, at(30))
        .await;
    assert_eq!(
        platform.resized.lock().unwrap().last(),
        Some(&(process.pid, 160, 90))
    );
    // Set again after a minute, in case Roblox resized its window.
    crate::launch::client_care::ClientCare::new(launcher.clone())
        .upkeep(&mut state, at(95))
        .await;
    assert_eq!(platform.resized.lock().unwrap().len(), 3);

    // Both sides or neither.
    config.alt_profile.window_height = 0;
    assert!(accounts.save_config(&config).await.is_err());
    server.await.unwrap();
}
