use super::fake::*;
use super::*;
use crate::secret::SecretString;

fn record() -> LaunchRecord<'static> {
    LaunchRecord {
        account_id: 1,
        username: "Alice",
        place_id: "1",
        mode: LaunchMode::Public,
    }
}

#[tokio::test]
async fn claims_only_new_processes_and_closes_only_owned_ones() {
    let platform = Arc::new(FakePlatform::default());
    let foreign = platform.process("RobloxPlayerBeta.exe", true, 1);
    platform.running.lock().unwrap().push(foreign.clone());
    let player = platform.process("RobloxPlayerBeta.exe", true, 1);
    platform
        .spawn_on_dispatch
        .lock()
        .unwrap()
        .push(player.clone());
    let manager = InstanceManager::fast(Arc::new(Shared(platform.clone())));
    let instance = manager
        .launch(
            record(),
            SecretString::new("roblox-player:1+x"),
            None,
            &CancellationToken::new(),
        )
        .await
        .unwrap();
    assert_eq!(instance.status, InstanceStatus::Running);
    assert_eq!(instance.pid, Some(player.pid));
    assert_eq!(platform.dispatched.lock().unwrap().len(), 1);
    manager.close_all().await.unwrap();
    assert_eq!(*platform.killed.lock().unwrap(), vec![player.key()]);
    assert!(platform.running.lock().unwrap().contains(&foreign));
    assert_eq!(manager.snapshot()[0].status, InstanceStatus::Exited);
    let serialized = serde_json::to_string(&manager.snapshot()).unwrap();
    assert!(!serialized.contains("roblox-player:"));
}

#[tokio::test]
async fn lists_and_closes_external_clients_only_on_request() {
    let platform = Arc::new(FakePlatform::default());
    let foreign = platform.process("RobloxPlayerBeta.exe", true, 1);
    let second = platform.process("RobloxPlayerBeta.exe", true, 1);
    platform
        .running
        .lock()
        .unwrap()
        .extend([foreign.clone(), second.clone()]);
    let owned = platform.process("RobloxPlayerBeta.exe", true, 1);
    platform
        .spawn_on_dispatch
        .lock()
        .unwrap()
        .push(owned.clone());
    let manager = InstanceManager::fast(Arc::new(Shared(platform.clone())));
    manager
        .launch(
            record(),
            SecretString::new("roblox-player:1+x"),
            None,
            &CancellationToken::new(),
        )
        .await
        .unwrap();
    manager.poll().await;
    // Pre-existing clients are listed as external and never claimed for the account.
    let external = manager.external_snapshot();
    assert_eq!(
        external.iter().map(|item| item.pid).collect::<Vec<_>>(),
        vec![foreign.pid, second.pid]
    );
    assert!(platform.killed.lock().unwrap().is_empty());
    manager.close_external(&external[0].id).await.unwrap();
    assert_eq!(*platform.killed.lock().unwrap(), vec![foreign.key()]);
    assert!(manager.close_external(&external[0].id).await.is_err());
    // "Close all Roblox" takes Nocta's own instance and the remaining external one.
    assert_eq!(manager.close_all_roblox().await.unwrap(), 2);
    assert!(platform.running.lock().unwrap().is_empty());
    assert!(manager.external_snapshot().is_empty());
}

#[tokio::test]
async fn launcher_handoff_and_pid_reuse_protection() {
    let platform = Arc::new(FakePlatform::default());
    let launcher = platform.process("RobloxPlayerLauncher.exe", false, 1);
    platform
        .spawn_on_dispatch
        .lock()
        .unwrap()
        .push(launcher.clone());
    let manager = InstanceManager::fast(Arc::new(Shared(platform.clone())));
    let instance = manager
        .launch(
            record(),
            SecretString::new("uri"),
            None,
            &CancellationToken::new(),
        )
        .await
        .unwrap();
    assert_eq!(instance.status, InstanceStatus::Starting);
    assert_eq!(instance.pid, Some(launcher.pid));
    // The launcher starts a player and exits.
    let player = platform.process("RobloxPlayerBeta.exe", true, launcher.pid);
    {
        let mut running = platform.running.lock().unwrap();
        running.retain(|process| process.pid != launcher.pid);
        running.push(player.clone());
    }
    manager.poll().await;
    let snapshot = manager.snapshot();
    assert_eq!(snapshot[0].status, InstanceStatus::Running);
    assert_eq!(snapshot[0].pid, Some(player.pid));
    // PID reuse: the same PID with a different creation time is a different process.
    {
        let mut running = platform.running.lock().unwrap();
        running.clear();
        let mut reused = player.clone();
        reused.created_ms += 10_000;
        running.push(reused);
    }
    manager.poll().await;
    assert_eq!(manager.snapshot()[0].status, InstanceStatus::Exited);
    manager.close_all().await.unwrap();
    assert!(platform.killed.lock().unwrap().is_empty());
    assert_eq!(platform.running.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn missing_handler_unknown_dispatch_and_enumeration_errors() {
    let platform = Arc::new(FakePlatform::default());
    platform
        .no_handler
        .store(true, std::sync::atomic::Ordering::SeqCst);
    let manager = InstanceManager::fast(Arc::new(Shared(platform.clone())));
    assert!(manager
        .launch(
            record(),
            SecretString::new("uri"),
            None,
            &CancellationToken::new()
        )
        .await
        .is_err());
    assert!(platform.dispatched.lock().unwrap().is_empty());
    platform
        .no_handler
        .store(false, std::sync::atomic::Ordering::SeqCst);
    let instance = manager
        .launch(
            record(),
            SecretString::new("uri"),
            None,
            &CancellationToken::new(),
        )
        .await
        .unwrap();
    assert_eq!(instance.status, InstanceStatus::Unknown);
    // A player that appears later is still claimed by the monitor shortly after dispatch.
    let player = platform.process("RobloxPlayerBeta.exe", true, 1);
    platform.running.lock().unwrap().push(player.clone());
    platform
        .fail_enumeration
        .store(true, std::sync::atomic::Ordering::SeqCst);
    manager.poll().await;
    assert_eq!(manager.snapshot()[0].status, InstanceStatus::Unknown);
    platform
        .fail_enumeration
        .store(false, std::sync::atomic::Ordering::SeqCst);
    manager.poll().await;
    assert_eq!(manager.snapshot()[0].status, InstanceStatus::Running);
    assert!(manager.set_multi_instance(true).is_err());
    assert_eq!(
        manager.state().multi_instance,
        MultiInstanceState::Unsupported
    );
}

#[tokio::test]
async fn late_monitor_does_not_claim_baseline_or_assign_a_second_dispatch_to_the_first() {
    let platform = Arc::new(FakePlatform::default());
    let existing = platform.process("RobloxPlayerBeta.exe", true, 1);
    platform.running.lock().unwrap().push(existing.clone());
    let manager = InstanceManager::fast(Arc::new(Shared(platform.clone())));
    let first = manager
        .launch(
            record(),
            SecretString::new("a"),
            None,
            &CancellationToken::new(),
        )
        .await
        .unwrap();
    assert_eq!(first.status, InstanceStatus::Unknown);
    manager.poll().await;
    assert_eq!(manager.snapshot()[0].status, InstanceStatus::Unknown);
    let second = manager
        .launch(
            record(),
            SecretString::new("b"),
            None,
            &CancellationToken::new(),
        )
        .await
        .unwrap();
    let later = platform.process("RobloxPlayerBeta.exe", true, 1);
    platform.running.lock().unwrap().push(later.clone());
    manager.poll().await;
    let snapshot = manager.snapshot();
    assert_eq!(
        snapshot
            .iter()
            .find(|entry| entry.id == first.id)
            .unwrap()
            .pid,
        None
    );
    assert_eq!(
        snapshot
            .iter()
            .find(|entry| entry.id == second.id)
            .unwrap()
            .pid,
        Some(later.pid)
    );
    manager.close_all().await.unwrap();
    assert!(platform.running.lock().unwrap().contains(&existing));
}

#[tokio::test]
async fn concurrent_launches_claim_distinct_processes() {
    let platform = Arc::new(FakePlatform::default());
    let manager = InstanceManager::fast(Arc::new(Shared(platform.clone())));
    let first = platform.process("RobloxPlayerBeta.exe", true, 1);
    platform
        .spawn_on_dispatch
        .lock()
        .unwrap()
        .push(first.clone());
    let cancel = CancellationToken::new();
    let launch_a = manager.launch(record(), SecretString::new("a"), None, &cancel);
    let manager_b = manager.clone();
    let platform_b = platform.clone();
    let launch_b = async move {
        tokio::time::sleep(Duration::from_millis(5)).await;
        let second = platform_b.process("RobloxPlayerBeta.exe", true, 1);
        // Queue the second client for the second dispatch only.
        let _ = manager_b.dispatch.lock().await;
        platform_b
            .spawn_on_dispatch
            .lock()
            .unwrap()
            .push(second.clone());
        drop(second);
        manager_b
            .launch(
                record(),
                SecretString::new("b"),
                None,
                &CancellationToken::new(),
            )
            .await
    };
    let (a, b) = tokio::join!(launch_a, launch_b);
    let (a, b) = (a.unwrap(), b.unwrap());
    assert_ne!(a.pid, b.pid);
    assert!(a.pid.is_some() && b.pid.is_some());
}

#[tokio::test]
async fn reports_clients_that_close_by_themselves_but_not_ones_nocta_closes() {
    let platform = Arc::new(FakePlatform::default());
    let manager = InstanceManager::fast(Arc::new(Shared(platform.clone())));
    let mut exits = manager.subscribe_exits();
    let cancel = CancellationToken::new();
    let launch = |player: ProcessInfo| {
        platform.spawn_on_dispatch.lock().unwrap().push(player);
        manager.launch(
            record(),
            SecretString::new("roblox-player:1+x"),
            None,
            &cancel,
        )
    };
    let crashing = platform.process("RobloxPlayerBeta.exe", true, 1);
    let exits_id = launch(crashing.clone()).await.unwrap().id;
    // The client disappears on its own (a crash or a kick).
    platform
        .running
        .lock()
        .unwrap()
        .retain(|process| process.pid != crashing.pid);
    manager.poll().await;
    assert_eq!(
        exits.try_recv().unwrap(),
        ExitEvent {
            instance_id: exits_id,
            account_id: 1,
            username: "Alice".into(),
            place_id: "1".into(),
        }
    );

    // Closing through Nocta is not a crash.
    let closed = platform.process("RobloxPlayerBeta.exe", true, 1);
    let instance = launch(closed).await.unwrap();
    manager.close(&instance.id).await.unwrap();
    manager.poll().await;
    assert!(exits.try_recv().is_err());
}

#[test]
fn anti_afk_clicks_an_accounts_own_point_before_the_general_action() {
    use crate::models::{AntiAfkAction, AppConfig, ClickPoint};
    let own = ClickPoint {
        x_percent: 10,
        y_percent: 90,
    };
    let keys = AppConfig {
        anti_afk_action: AntiAfkAction::Jump,
        ..Default::default()
    };
    assert_eq!(
        AfkInput::choose(&keys, None),
        AfkInput::Key(AntiAfkAction::Jump)
    );
    assert_eq!(AfkInput::choose(&keys, Some(own)), AfkInput::Click(own));
    let clicks = AppConfig {
        anti_afk_action: AntiAfkAction::Click,
        anti_afk_click: ClickPoint {
            x_percent: 50,
            y_percent: 20,
        },
        ..Default::default()
    };
    assert_eq!(
        AfkInput::choose(&clicks, None),
        AfkInput::Click(clicks.anti_afk_click)
    );
}
