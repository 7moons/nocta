//! The frame-rate cap on real clients: one account joins a game through Nocta's own client. CPU
//! use stands in for the frame rate (a client at 5 FPS does a fraction of the work of one at 60).
//! Roblox's settings file is shared with the user's own clients, so Nocta's cap may be there only
//! while its client starts; afterwards the file must hold the user's value again, also after the
//! user closes a capped client (Roblox saves its settings, cap included, on the way out; seen by
//! hand with WM_CLOSE, simulated here).
//!
//! What this is built on, measured here on 2026-10-18 with Nocta's client: a client started with
//! cap 5 kept it after the file went back to -1 once it was in game (19 % CPU against 40 % without
//! a cap); put back as soon as its log appeared, the cap was lost (38 %).
//!
//! `NOCTA_SHARE_DATA=%APPDATA%\app.nocta.desktop cargo test -p nocta-core --lib launch::tests::live_fps -- --ignored --nocapture --test-threads 1`
use crate::launch::client_logs::logs_dir;
use crate::launch::instances::InstanceManager;
use crate::launch::{fps, Launcher};
use crate::roblox::web::{RobloxWeb, SIGNED_OUT};
use crate::{
    accounts::Accounts,
    models::{
        AccountHealth, AppConfig, ClientStage, InstanceStatus, LaunchTargetInput, TaskStatus,
    },
    sessions::Sessions,
    storage::Storage,
    vault::Vault,
};
use reqwest::Url;
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};
use tokio_util::sync::CancellationToken;

/// Natural Disaster Survival: always has a public server.
const PLACE: &str = "189707";

fn cpu_ms(pid: u32) -> Option<u64> {
    use windows_sys::Win32::{
        Foundation::{CloseHandle, FILETIME},
        System::Threading::{GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION},
    };
    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if handle.is_null() {
        return None;
    }
    let mut times = [FILETIME::default(); 4];
    let [created, exited, kernel, user] = &mut times;
    let ok = unsafe { GetProcessTimes(handle, created, exited, kernel, user) };
    unsafe { CloseHandle(handle) };
    let ticks = |time: &FILETIME| ((time.dwHighDateTime as u64) << 32) | time.dwLowDateTime as u64;
    (ok != 0).then(|| (ticks(&times[2]) + ticks(&times[3])) / 10_000)
}

/// Average CPU use of `pid` over `secs`, in percent of one core.
async fn cpu_percent(pid: u32, secs: u64) -> f64 {
    let start = cpu_ms(pid).expect("client running");
    let wall = Instant::now();
    tokio::time::sleep(Duration::from_secs(secs)).await;
    let end = cpu_ms(pid).expect("client still running");
    (end - start) as f64 / wall.elapsed().as_millis() as f64 * 100.0
}

fn cap_in(file: &Path) -> Option<i32> {
    fps::read_cap(&std::fs::read_to_string(file).ok()?)
}

fn process_alive(pid: u32) -> bool {
    roblox_pids().contains(&pid)
}

/// Roblox takes about 20 s to close after its window is closed.
fn wait_exit(pid: u32) {
    let started = Instant::now();
    while process_alive(pid) {
        assert!(
            started.elapsed() < Duration::from_secs(120),
            "client {pid} did not exit"
        );
        std::thread::sleep(Duration::from_millis(500));
    }
    println!("  exited after {} ms", started.elapsed().as_millis());
}

fn roblox_pids() -> HashSet<u32> {
    let out = std::process::Command::new("tasklist")
        .args([
            "/FI",
            "IMAGENAME eq RobloxPlayerBeta.exe",
            "/FO",
            "CSV",
            "/NH",
        ])
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|line| line.split("\",\"").nth(1)?.parse().ok())
        .collect()
}

/// Closes Roblox processes started during the test; the user's own are left alone.
fn close_new(before: &HashSet<u32>) {
    for pid in roblox_pids().difference(before) {
        let _ = std::process::Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/F"])
            .output();
    }
}

struct Setup {
    launcher: Arc<Launcher>,
    account: u32,
    config: AppConfig,
    _copy: tempfile::TempDir,
}

/// A launcher like the app's, on a copy of the user's data and Nocta's installed client.
async fn setup(data: &Path) -> Setup {
    let copy = tempfile::tempdir().unwrap();
    for file in ["nocta.sqlite", "nocta.sqlite-wal", "nocta.sqlite-shm"] {
        if data.join(file).exists() {
            std::fs::copy(data.join(file), copy.path().join(file)).unwrap();
        }
    }
    let storage = Storage::open(&copy.path().join("nocta.sqlite"))
        .await
        .unwrap();
    let vault = Arc::new(Vault::open(&storage.vault_id().await.unwrap(), false).unwrap());
    let accounts = Accounts::new(storage, vault);
    let config = accounts.config().await.unwrap();
    let sessions = Sessions::new(
        accounts.clone(),
        Arc::new(crate::browser::BrowserManager::new(
            None,
            copy.path().join("profiles"),
        )),
        &config,
    )
    .unwrap();
    let launcher = Launcher::new(
        accounts.clone(),
        sessions,
        Arc::new(RobloxWeb::new().unwrap()),
        InstanceManager::new(
            InstanceManager::native(),
            Arc::new(tokio::sync::Notify::new()),
        ),
        // The real install: no download. Its state file is backed up by the test.
        Arc::new(
            crate::launch::client::ClientInstall::new(
                data.join("roblox"),
                Arc::new(tokio::sync::Notify::new()),
            )
            .unwrap(),
        ),
    );
    let cancel = CancellationToken::new();
    let mut candidates = accounts.list().await.unwrap();
    candidates.retain(|account| account.deleted_at.is_none() && account.has_cookie);
    candidates.sort_by_key(|account| account.health != AccountHealth::Healthy);
    let mut account = None;
    for candidate in candidates.iter().take(20) {
        let cookie = accounts.secrets(candidate.id).await.unwrap().cookie.clone();
        let url = Url::parse("https://users.roblox.com/v1/users/authenticated").unwrap();
        match launcher.web.get_json_authed(url, &cookie, &cancel).await {
            Ok(_) => {
                account = Some(candidate.id);
                break;
            }
            Err(crate::CoreError::Validation(message)) if message == SIGNED_OUT => {}
            Err(_) => {}
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    Setup {
        launcher,
        account: account.expect("a signed-in account"),
        config,
        _copy: copy,
    }
}

/// Launches one client with `cap` and waits until it is in game. Returns its instance ID, its
/// process ID and the highest cap seen in the settings file while it started.
async fn launch(setup: &Setup, file: &Path, cap: u32) -> (String, u32, Option<i32>) {
    let mut config = setup.config.clone();
    config.fps_cap = cap;
    config.afk_mode = false;
    let started = Instant::now();
    let tasks = setup
        .launcher
        .start(
            &[setup.account],
            LaunchTargetInput::Public {
                place_id: PLACE.into(),
            },
            config,
        )
        .await
        .unwrap();
    let mut seen = None;
    loop {
        seen = seen.max(cap_in(file));
        let instance = setup
            .launcher
            .instances
            .snapshot()
            .into_iter()
            .rfind(|instance| {
                instance.account_id == setup.account && instance.status != InstanceStatus::Exited
            });
        if let Some(instance) = instance.filter(|instance| instance.stage == ClientStage::InGame) {
            println!("  in game after {} ms", started.elapsed().as_millis());
            return (instance.id, instance.pid.unwrap(), seen);
        }
        if let Some(task) = setup.launcher.sessions.tasks.get(tasks[0]) {
            assert!(
                task.status != TaskStatus::Failed,
                "launch failed: {}",
                task.detail
            );
        }
        assert!(
            started.elapsed() < Duration::from_secs(180),
            "never in game"
        );
        setup.launcher.refresh_client_logs().await;
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "joins a game with a real account twice"]
async fn live_frame_rate_cap_reaches_nocta_clients_and_leaves_the_users_file_alone() {
    let data = PathBuf::from(std::env::var("NOCTA_SHARE_DATA").expect("NOCTA_SHARE_DATA"));
    let dir = fps::settings_dir().unwrap();
    let file = fps::settings_file(&dir).expect("Roblox settings");
    let backup = std::fs::read(&file).unwrap();
    let state = data.join("roblox").join(".fps-cap-original");
    let state_backup = std::fs::read(&state).ok();
    let original = cap_in(&file).unwrap_or(-1);
    println!("{} has FramerateCap {original}", file.display());
    let before = roblox_pids();
    let setup = setup(&data).await;
    setup.launcher.frame_rate.use_settings(Some(dir));
    setup.launcher.client_logs.set_dir(logs_dir());
    let stop = CancellationToken::new();
    let _monitor = setup.launcher.instances.start_monitor(stop.clone());

    let result = std::panic::AssertUnwindSafe(async {
        println!("capped at 5");
        let (capped_id, pid, seen) = launch(&setup, &file, 5).await;
        assert_eq!(
            seen,
            Some(5),
            "the cap was in the file while the client started"
        );
        tokio::time::sleep(Duration::from_secs(5)).await;
        assert_eq!(cap_in(&file), Some(original), "the user's value is back");
        let capped = cpu_percent(pid, 20).await;
        println!("  CPU {capped:.1}%, file at {original}");

        setup.launcher.instances.close(&capped_id).await.unwrap();
        wait_exit(pid);
        // A client the user closes saves its settings, cap included, on the way out (seen with
        // WM_CLOSE by hand: the file went from -1 to 5). Here the file is written the same way.
        let xml = std::fs::read_to_string(&file).unwrap();
        std::fs::write(&file, fps::write_cap(&xml, 5).unwrap()).unwrap();
        // What the exit hook and the upkeep tick do.
        setup.launcher.frame_rate.settle().unwrap();
        assert_eq!(cap_in(&file), Some(original), "put back after the close");
        tokio::time::sleep(Duration::from_secs(5)).await;

        println!("no cap (control)");
        let (id, pid, seen) = launch(&setup, &file, 0).await;
        assert_eq!(seen, Some(original), "the file was not touched");
        tokio::time::sleep(Duration::from_secs(5)).await;
        let free = cpu_percent(pid, 20).await;
        println!("  CPU {free:.1}%");
        setup.launcher.instances.close(&id).await.unwrap();
        wait_exit(pid);
        println!("SUMMARY capped {capped:.1}% | uncapped {free:.1}%");
        (capped, free)
    });
    let outcome = futures::FutureExt::catch_unwind(result).await;
    stop.cancel();
    close_new(&before);
    std::fs::write(&file, &backup).unwrap();
    match state_backup {
        Some(bytes) => std::fs::write(&state, bytes).unwrap(),
        None => {
            let _ = std::fs::remove_file(&state);
        }
    }
    let (capped, free) = outcome.unwrap_or_else(|panic| std::panic::resume_unwind(panic));
    assert!(capped < free * 0.6, "the client kept Nocta's cap");
}
