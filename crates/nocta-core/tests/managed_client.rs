//! End-to-end check of Nocta's own Roblox client against the real Roblox setup CDN.
//! Downloads the current client (about 230 MB) into a temporary folder, starts it through the same
//! launch path accounts use (opening the Roblox app, no game join), confirms Nocta tracks the
//! process, and closes it. Run with: cargo test -p nocta-core --test managed_client -- --ignored

use nocta_core::{
    launch::{
        client::{ClientInstall, EXECUTABLE},
        instances::{InstanceManager, LaunchRecord},
    },
    models::{AppConfig, InstanceStatus, LaunchMode},
    secret::SecretString,
};
use std::{sync::Arc, time::Duration};
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;

#[tokio::test(flavor = "multi_thread")]
#[ignore = "downloads the real Roblox client and starts it"]
async fn installs_and_launches_the_real_roblox_client() {
    let root = tempfile::tempdir().unwrap();
    let client =
        Arc::new(ClientInstall::new(root.path().to_path_buf(), Arc::new(Notify::new())).unwrap());

    let latest = client.latest(true).await.unwrap();
    println!("latest Roblox: {} ({})", latest.version, latest.guid);
    let started = std::time::Instant::now();
    let installed = client
        .install(&latest.guid, Some(latest.version.clone()))
        .await
        .unwrap();
    println!(
        "installed {} MB in {:.1}s",
        installed.size_mb,
        started.elapsed().as_secs_f32()
    );
    let dir = client.version_dir(&latest.guid);
    for required in [
        EXECUTABLE,
        "AppSettings.xml",
        "content/fonts",
        "ssl",
        "shaders",
    ] {
        assert!(dir.join(required).exists(), "missing {required}");
    }
    assert!(!root.path().join(".staging").join(&latest.guid).exists());

    // Automatic mode sees the latest version installed and launches it without downloading again.
    let mut config = AppConfig {
        managed_client: true,
        client_low_graphics: true,
        client_graphics_api: nocta_core::models::ClientGraphicsApi::Vulkan,
        ..AppConfig::default()
    };
    let executable = client
        .ready(&config, &CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(executable, dir.join(EXECUTABLE));

    let manager = InstanceManager::new(InstanceManager::native(), Arc::new(Notify::new()));
    let instance = manager
        .launch(
            LaunchRecord {
                account_id: 0,
                username: "managed-client-check",
                place_id: "0",
                mode: LaunchMode::Public,
            },
            SecretString::new("roblox-player:1+launchmode:app"),
            Some(&executable),
            &CancellationToken::new(),
        )
        .await
        .unwrap();
    println!(
        "instance: {:?} pid {:?} — {}",
        instance.status, instance.pid, instance.detail
    );
    assert_eq!(
        instance.status,
        InstanceStatus::Running,
        "{}",
        instance.detail
    );
    let running = manager.running_paths().await;
    assert!(
        running.iter().any(|path| path
            .to_lowercase()
            .starts_with(&dir.to_string_lossy().to_lowercase())),
        "the running client comes from Nocta's folder: {running:?}"
    );
    // A version with a running client is protected from removal.
    config.client_keep_versions = 0;
    assert_eq!(client.cleanup(&config, &running), 0);

    tokio::time::sleep(Duration::from_secs(5)).await;
    // Roblox writes the flags it applied into its client log; find this run's log.
    let logs = std::path::PathBuf::from(std::env::var("LOCALAPPDATA").unwrap()).join("Roblox/logs");
    let newest = std::fs::read_dir(&logs)
        .unwrap()
        .flatten()
        .filter(|entry| entry.file_name().to_string_lossy().contains("Player"))
        .max_by_key(|entry| entry.metadata().and_then(|meta| meta.modified()).ok())
        .map(|entry| entry.path());
    // Roblox applied the flags from Nocta's copy and rendered with the chosen API.
    let log =
        String::from_utf8_lossy(&std::fs::read(newest.expect("a Roblox player log")).unwrap())
            .into_owned();
    assert!(
        log.contains("\"DFIntDebugFRMQualityLevelOverride\": \"1\""),
        "low graphics flags were not loaded"
    );
    assert!(
        log.contains("Vulkan Device:"),
        "the client did not render with Vulkan"
    );

    manager.close(&instance.id).await.unwrap();
    assert!(manager.running_paths().await.iter().all(|path| !path
        .to_lowercase()
        .starts_with(&dir.to_string_lossy().to_lowercase())));
}
