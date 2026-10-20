use super::instances::fake::{FakePlatform, Shared};
use super::*;
use crate::roblox::web::fixture::{response, serve};

mod account_web;
mod clients;
mod live;
mod live_fps;

pub(super) async fn fixture(base: &str) -> (Arc<Launcher>, Arc<FakePlatform>, Accounts) {
    let accounts = Accounts::new(
        crate::storage::Storage::memory().await.unwrap(),
        Arc::new(crate::vault::Vault::test_key()),
    );
    accounts
        .import("Alice:pw:synthetic-cookie".into(), None, false)
        .await
        .unwrap();
    let (launcher, platform) =
        launcher_for(accounts.clone(), RobloxWeb::fixture(base), Some(base)).await;
    (launcher, platform, accounts)
}

/// A launcher over `accounts` that talks through `web`; `sessions_base` points session checks at
/// a fixture server (None: the real Roblox).
pub(super) async fn launcher_for(
    accounts: Accounts,
    web: RobloxWeb,
    sessions_base: Option<&str>,
) -> (Arc<Launcher>, Arc<FakePlatform>) {
    let config = AppConfig::default();
    let temp = tempfile::tempdir().unwrap();
    let mut sessions_client = crate::sessions::http::RobloxClient::new().unwrap();
    if let Some(base) = sessions_base {
        sessions_client.use_fixture(base);
    }
    let sessions = Sessions::new(
        accounts.clone(),
        Arc::new(crate::browser::BrowserManager::new(
            None,
            temp.path().join("p"),
        )),
        &config,
    )
    .unwrap();
    let sessions = Arc::new(Sessions::with_client(&sessions, sessions_client));
    let platform = Arc::new(FakePlatform::default());
    let manager = InstanceManager::fast(Arc::new(Shared(platform.clone())));
    let launcher = Launcher::new(
        accounts.clone(),
        sessions,
        Arc::new(web),
        manager,
        Arc::new(
            client::ClientInstall::new(
                temp.path().join("roblox"),
                Arc::new(tokio::sync::Notify::new()),
            )
            .unwrap(),
        ),
    );
    (launcher, platform)
}

pub(super) async fn wait_terminal(launcher: &Launcher, task: u32) -> AccountTask {
    for _ in 0..1500 {
        if let Some(task) = launcher.sessions.tasks.get(task) {
            if task.status.terminal() {
                return task;
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    panic!("launch task did not finish");
}

pub(super) fn identity() -> String {
    response(
        "200 OK",
        &[("Content-Type", "application/json")],
        r#"{"id":1,"name":"Alice"}"#,
    )
}

#[tokio::test]
async fn share_links_expand_with_any_signed_in_account_once() {
    let json = |body: &str| response("200 OK", &[("Content-Type", "application/json")], body);
    let (base, server) = serve(vec![
        // Alice (healthy, tried first) is signed out; Bob opens the link.
        response("401 Unauthorized", &[], ""),
        json(r#"{"privateServerInviteData":{"status":"Valid","placeId":77,"linkCode":"code-1"}}"#),
    ])
    .await;
    let (launcher, _, accounts) = fixture(&base).await;
    accounts
        .import("Bob:pw:other-cookie".into(), None, false)
        .await
        .unwrap();
    accounts
        .mark_health(1, AccountHealth::Healthy, None)
        .await
        .unwrap();
    let cancel = CancellationToken::new();
    let link = reqwest::Url::parse("https://www.roblox.com/share?code=abc&type=Server").unwrap();
    let direct = launcher
        .expand_share_link(&link, None, &cancel)
        .await
        .unwrap();
    assert_eq!(
        direct.as_deref(),
        Some("https://www.roblox.com/games/77/?privateServerLinkCode=code-1")
    );
    let requests = server.await.unwrap();
    assert!(requests[0].contains("synthetic-cookie"));
    assert!(requests[1].contains("other-cookie"));
    // Remembered: the fixture server is gone, so a second lookup would fail.
    assert_eq!(
        launcher
            .expand_share_link(&link, None, &cancel)
            .await
            .unwrap(),
        direct
    );
    // Other links are left alone.
    let game = reqwest::Url::parse("https://www.roblox.com/games/5/x").unwrap();
    assert!(launcher
        .expand_share_link(&game, None, &cancel)
        .await
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn configured_server_selection_resolves_to_the_exact_job_without_http() {
    let (launcher, _, _) = fixture("http://127.0.0.1:9").await;
    let config = AppConfig {
        place_id: "123".into(),
        invite_url: "https://www.roblox.com/games/123/?gameInstanceId=job-fixture".into(),
        ..Default::default()
    };
    let target = launcher
        .resolve_summary(None, &LaunchTargetInput::Configured, &config)
        .await
        .unwrap();
    assert_eq!(target.mode, LaunchMode::Job);
    assert_eq!(target.place_id, "123");
    assert_eq!(target.job_id.as_deref(), Some("job-fixture"));
    assert!(!target.used_fallback);
}

#[tokio::test]
async fn public_launch_runs_precheck_ticket_and_dispatches_exact_uri() {
    let player = FakePlatform::default().process("RobloxPlayerBeta.exe", true, 1);
    let (base, server) = serve(vec![
        identity(),
        response("403 Forbidden", &[("x-csrf-token", "join-csrf")], ""),
        response("200 OK", &[], ""),
        response("403 Forbidden", &[("x-csrf-token", "ticket-csrf")], ""),
        response(
            "200 OK",
            &[("rbx-authentication-ticket", "synthetic-ticket")],
            "",
        ),
    ])
    .await;
    let (launcher, platform, accounts) = fixture(&base).await;
    platform
        .spawn_on_dispatch
        .lock()
        .unwrap()
        .push(player.clone());
    let config = AppConfig {
        place_id: "920587237".into(),
        multi_instance: false,
        pre_launch_delay_secs: 0,
        ..Default::default()
    };
    let ids = launcher
        .start(&[1], LaunchTargetInput::Configured, config)
        .await
        .unwrap();
    let task = wait_terminal(&launcher, ids[0]).await;
    assert_eq!(task.status, TaskStatus::Succeeded, "{}", task.detail);
    assert!(task.detail.contains("Roblox client is running"));
    let requests = server.await.unwrap();
    assert!(requests[2].starts_with("POST /v1/join-game"));
    assert!(requests[4].starts_with("POST /v1/authentication-ticket"));
    let dispatched = platform.dispatched.lock().unwrap().clone();
    assert_eq!(dispatched.len(), 1);
    let uri = &dispatched[0];
    assert!(
        uri.starts_with("roblox-player:1+launchmode:play+gameinfo:synthetic-ticket+launchtime:")
    );
    assert!(uri.contains("placelauncherurl:https%3A%2F%2Fassetgame.roblox.com%2Fgame%2FPlaceLauncher.ashx%3Frequest%3DRequestGame%26placeId%3D920587237"));
    // The precheck and PlaceLauncher share the same tracker ID.
    let tracker = uri.rsplit("+browsertrackerid:").next().unwrap();
    assert!(requests[2].contains(&format!("\"browserTrackerId\":{tracker}")));
    let instances = launcher.instances.snapshot();
    assert_eq!(instances[0].status, InstanceStatus::Running);
    let logs = serde_json::to_string(&accounts.logs().await.unwrap()).unwrap();
    for secret in [
        "synthetic-ticket",
        "synthetic-cookie",
        "ticket-csrf",
        "join-csrf",
    ] {
        assert!(!logs.contains(secret), "{secret} leaked into logs");
        assert!(!serde_json::to_string(&instances).unwrap().contains(secret));
        assert!(!task.detail.contains(secret));
    }
    assert_eq!(accounts.storage_metrics().await.launches_success_total, 1);
}

#[tokio::test]
async fn a_captcha_at_the_precheck_stops_the_launch_and_points_to_the_browser() {
    let (base, server) = serve(vec![
        identity(),
        response("403 Forbidden", &[("x-csrf-token", "join-csrf")], ""),
        response(
            "403 Forbidden",
            &[
                ("rblx-challenge-id", "id"),
                ("rblx-challenge-metadata", "meta"),
            ],
            "",
        ),
    ])
    .await;
    let (launcher, platform, accounts) = fixture(&base).await;
    let config = AppConfig {
        place_id: "1".into(),
        multi_instance: false,
        ..Default::default()
    };
    let ids = launcher
        .start(&[1], LaunchTargetInput::Configured, config)
        .await
        .unwrap();
    let task = wait_terminal(&launcher, ids[0]).await;
    assert_eq!(task.status, TaskStatus::Failed);
    // Captchas are not solved by Nocta any more: the account's browser is where they go.
    assert!(
        task.detail.contains("Open the account's browser"),
        "{}",
        task.detail
    );
    assert!(platform.dispatched.lock().unwrap().is_empty());
    assert_eq!(
        accounts.get(1).await.unwrap().health,
        AccountHealth::CaptchaRequired
    );
    server.await.unwrap();
    assert_eq!(accounts.storage_metrics().await.launches_total, 1);
}

#[tokio::test]
async fn invalid_explicit_targets_and_missing_tickets_fail_without_dispatch() {
    let (launcher, _, _) = fixture("http://127.0.0.1:9").await;
    assert!(launcher
        .start(
            &[1],
            LaunchTargetInput::Job {
                place_id: "1".into(),
                job_id: "".into()
            },
            AppConfig::default()
        )
        .await
        .is_err());
    assert!(launcher
        .start(
            &[1],
            LaunchTargetInput::Server {
                target_id: "target-unknown".into()
            },
            AppConfig::default()
        )
        .await
        .is_err());
    launcher.sessions.tasks.ensure_idle(&[1]).unwrap();

    let (base, server) = serve(vec![
        identity(),
        response("403 Forbidden", &[("x-csrf-token", "t")], ""),
        response("403 Forbidden", &[], ""),
        identity(),
        response("403 Forbidden", &[], ""),
        response("403 Forbidden", &[], ""),
    ])
    .await;
    let (launcher, platform, _) = fixture(&base).await;
    let config = AppConfig {
        place_id: "1".into(),
        launch_precheck: false,
        multi_instance: false,
        ..Default::default()
    };
    let ids = launcher
        .start(&[1], LaunchTargetInput::Configured, config)
        .await
        .unwrap();
    let task = wait_terminal(&launcher, ids[0]).await;
    assert_eq!(task.status, TaskStatus::Failed);
    assert!(task.detail.contains("authentication ticket"));
    assert!(platform.dispatched.lock().unwrap().is_empty());
    let requests = server.await.unwrap();
    // The retry reuses the token from the first attempt when Roblox returns no new one.
    assert!(requests[5].to_lowercase().contains("x-csrf-token: t"));
}

#[tokio::test]
async fn private_launch_from_opaque_target_uses_request_private_game() {
    let (base, server) = serve(vec![
        identity(),
        response("403 Forbidden", &[("x-csrf-token", "c")], ""),
        response("200 OK", &[("rbx-authentication-ticket", "tk")], ""),
    ])
    .await;
    let (launcher, platform, _) = fixture(&base).await;
    let target_id = launcher.servers.register(ResolvedTarget {
        place_id: "77".into(),
        kind: ResolvedKind::Private {
            access_code: SecretString::new("synthetic-access"),
            link_code: None,
        },
        used_fallback: false,
    });
    let summary = launcher
        .resolve_summary(
            None,
            &LaunchTargetInput::Server {
                target_id: target_id.clone(),
            },
            &AppConfig::default(),
        )
        .await
        .unwrap();
    assert_eq!(summary.mode, LaunchMode::Private);
    assert!(!serde_json::to_string(&summary)
        .unwrap()
        .contains("synthetic-access"));
    let config = AppConfig {
        launch_precheck: false,
        multi_instance: false,
        ..Default::default()
    };
    let ids = launcher
        .start(&[1], LaunchTargetInput::Server { target_id }, config)
        .await
        .unwrap();
    let task = wait_terminal(&launcher, ids[0]).await;
    assert!(!task.detail.contains("synthetic-access"));
    let uri = platform.dispatched.lock().unwrap()[0].clone();
    assert!(uri.contains("RequestPrivateGame"));
    assert!(uri.contains("accessCode%3Dsynthetic-access"));
    assert!(!uri.contains("isPlayTogetherGame"));
    server.await.unwrap();
}

#[tokio::test]
async fn bulk_launch_waits_between_accounts_and_stop_cancels_the_wait() {
    let (launcher, platform, accounts) = fixture("http://127.0.0.1:9").await;
    accounts
        .import("Bob:pw:c".into(), None, false)
        .await
        .unwrap();
    let config = AppConfig {
        place_id: "1".into(),
        bulk_action_delay_secs: 30,
        multi_instance: false,
        ..Default::default()
    };
    // Account 1 fails fast on the unreachable fixture; account 2 waits for the bulk interval.
    let ids = launcher
        .start(&[1, 2], LaunchTargetInput::Configured, config)
        .await
        .unwrap();
    wait_terminal(&launcher, ids[0]).await;
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    let waiting = launcher.sessions.tasks.get(ids[1]).unwrap();
    assert!(
        waiting.detail.contains("bulk interval"),
        "{}",
        waiting.detail
    );
    launcher.sessions.tasks.stop(false);
    let cancelled = wait_terminal(&launcher, ids[1]).await;
    assert_eq!(cancelled.status, TaskStatus::Cancelled);
    assert!(platform.dispatched.lock().unwrap().is_empty());
    launcher.shutdown().await;
}

#[tokio::test]
async fn squad_and_follow_targets_settle_to_one_exact_server() {
    let json = |body: &str| response("200 OK", &[("Content-Type", "application/json")], body);
    let (base, server) = serve(vec![
        // Squad of two: the first page has only one free slot, the second fits both.
        json(r#"{"data":[{"id":"tight","playing":9,"maxPlayers":10}],"nextPageCursor":"page-2"}"#),
        json(r#"{"data":[{"id":"roomy","playing":4,"maxPlayers":10}],"nextPageCursor":null}"#),
        // Follow: username lookup, presence after the CSRF challenge.
        json(r#"{"data":[{"id":156,"name":"builderman","displayName":"Builder"}]}"#),
        response("403 Forbidden", &[("x-csrf-token", "csrf")], ""),
        json(r#"{"userPresences":[{"userPresenceType":2,"userId":156,"placeId":1818,"gameId":"job-42"}]}"#),
        // Follow a player who is offline.
        json(r#"{"data":[{"id":157,"name":"nobody","displayName":"Nobody"}]}"#),
        response("403 Forbidden", &[("x-csrf-token", "csrf")], ""),
        json(r#"{"userPresences":[{"userPresenceType":0,"userId":157}]}"#),
    ])
    .await;
    let (launcher, _, accounts) = fixture(&base).await;
    accounts
        .import("Bob:pw:other-cookie".into(), None, false)
        .await
        .unwrap();
    let cancel = CancellationToken::new();
    let (target, note) = launcher
        .settle_target(
            LaunchTargetInput::Pick {
                place_id: "77".into(),
                strategy: JoinStrategy::Squad,
                exclude_job: None,
            },
            &[1, 2],
            &cancel,
        )
        .await
        .unwrap();
    assert!(matches!(target, LaunchTargetInput::Job { ref job_id, .. } if job_id == "roomy"));
    assert!(note.unwrap().contains("room for 2"));

    let (target, _) = launcher
        .settle_target(
            LaunchTargetInput::Follow {
                username: "builderman".into(),
            },
            &[1],
            &cancel,
        )
        .await
        .unwrap();
    assert!(matches!(
        target,
        LaunchTargetInput::Job { ref place_id, ref job_id } if place_id == "1818" && job_id == "job-42"
    ));
    let offline = launcher
        .settle_target(
            LaunchTargetInput::Follow {
                username: "nobody".into(),
            },
            &[1],
            &cancel,
        )
        .await
        .unwrap_err();
    assert!(offline.to_string().contains("not in a game"));
    let requests = server.await.unwrap();
    assert!(
        requests[0].contains("sortOrder=Desc"),
        "squads look at full servers first"
    );
    assert!(requests[1].contains("cursor=page-2"));
    // Presence is asked as the first selected account.
    assert!(requests[4].contains("synthetic-cookie"));
}

#[tokio::test]
async fn a_crashed_client_rejoins_the_same_server_and_closing_it_does_not() {
    let launch_responses = || {
        vec![
            identity(),
            response("403 Forbidden", &[("x-csrf-token", "join-csrf")], ""),
            response("200 OK", &[], ""),
            response("403 Forbidden", &[("x-csrf-token", "ticket-csrf")], ""),
            response(
                "200 OK",
                &[("rbx-authentication-ticket", "synthetic-ticket")],
                "",
            ),
        ]
    };
    let mut responses = launch_responses();
    responses.extend(launch_responses());
    let (base, server) = serve(responses).await;
    let (launcher, platform, accounts) = fixture(&base).await;
    let config = AppConfig {
        place_id: "920587237".into(),
        multi_instance: false,
        pre_launch_delay_secs: 0,
        auto_rejoin: true,
        ..Default::default()
    };
    accounts.save_config(&config).await.unwrap();
    super::rejoin::Rejoin::new(launcher.clone()).spawn();
    let first = platform.process("RobloxPlayerBeta.exe", true, 1);
    let second = platform.process("RobloxPlayerBeta.exe", true, 1);
    platform
        .spawn_on_dispatch
        .lock()
        .unwrap()
        .push(first.clone());
    let target = LaunchTargetInput::Job {
        place_id: "920587237".into(),
        job_id: "11111111-2222-3333-4444-555555555555".into(),
    };
    let ids = launcher.start(&[1], target, config).await.unwrap();
    wait_terminal(&launcher, ids[0]).await;

    // The client crashes; the monitor notices and the account goes back to the same server.
    platform
        .spawn_on_dispatch
        .lock()
        .unwrap()
        .push(second.clone());
    platform
        .running
        .lock()
        .unwrap()
        .retain(|process| process.pid != first.pid);
    launcher.instances.poll().await;
    for _ in 0..500 {
        if platform.dispatched.lock().unwrap().len() == 2 {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    let dispatched = platform.dispatched.lock().unwrap().clone();
    assert_eq!(dispatched.len(), 2, "the account was relaunched");
    assert!(dispatched[1].contains("11111111-2222-3333-4444-555555555555"));
    for _ in 0..500 {
        if launcher
            .instances
            .snapshot()
            .iter()
            .any(|instance| instance.pid == Some(second.pid))
        {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    let logs = serde_json::to_string(&accounts.logs().await.unwrap()).unwrap();
    assert!(logs.contains("Rejoining in 5 s"));

    // Closing it through Nocta does not bring it back.
    let running = launcher
        .instances
        .snapshot()
        .into_iter()
        .find(|instance| instance.pid == Some(second.pid))
        .unwrap();
    launcher.instances.close(&running.id).await.unwrap();
    launcher.instances.poll().await;
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    assert_eq!(platform.dispatched.lock().unwrap().len(), 2);
    server.await.unwrap();
}

#[tokio::test]
async fn a_signed_in_session_becomes_an_account_or_refreshes_the_existing_one() {
    let (launcher, _, accounts) = fixture("http://127.0.0.1:9").await;
    let identity = crate::sessions::http::Identity {
        id: 4242,
        name: "NewAlt".into(),
    };
    let id = launcher
        .sessions
        .save_signed_in(&identity, "_|WARNING:fresh-cookie", None)
        .await
        .unwrap();
    let account = accounts.get(id).await.unwrap();
    assert_eq!(account.username, "NewAlt");
    assert_eq!(account.user_id.as_deref(), Some("4242"));
    assert_eq!(
        accounts.secrets(id).await.unwrap().cookie,
        "_|WARNING:fresh-cookie"
    );
    // Signing in as an account Nocta already has replaces its cookie instead of a duplicate.
    let again = launcher
        .sessions
        .save_signed_in(
            &crate::sessions::http::Identity {
                id: 1,
                name: "alice".into(),
            },
            "_|WARNING:newer",
            None,
        )
        .await
        .unwrap();
    assert_eq!(again, 1);
    assert_eq!(accounts.secrets(1).await.unwrap().cookie, "_|WARNING:newer");
    assert_eq!(accounts.list().await.unwrap().len(), 2);
}

#[tokio::test]
async fn presence_of_own_accounts_is_refreshed_with_a_working_session() {
    let json = |body: &str| response("200 OK", &[("Content-Type", "application/json")], body);
    let (base, server) = serve(vec![
        response("403 Forbidden", &[("x-csrf-token", "csrf")], ""),
        json(r#"{"userPresences":[{"userPresenceType":2,"userId":156,"placeId":1818,"lastLocation":"Crossroads"}]}"#),
    ])
    .await;
    let (launcher, _, accounts) = fixture(&base).await;
    accounts
        .replace_cookie(1, "_|WARNING:synthetic-cookie", "156")
        .await
        .unwrap();
    let updated = launcher
        .presence
        .refresh_presence(&CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(updated, 1);
    let presence = accounts.get(1).await.unwrap().presence.unwrap();
    assert_eq!(presence.user_presence_type, 2);
    assert_eq!(presence.place_id.as_deref(), Some("1818"));
    assert_eq!(presence.last_location.as_deref(), Some("Crossroads"));
    let requests = server.await.unwrap();
    assert!(requests[1].contains(r#""userIds":[156]"#));
}

#[tokio::test]
async fn a_banned_session_falls_back_to_anonymous_presence_and_explains_friends() {
    let json = |body: &str| response("200 OK", &[("Content-Type", "application/json")], body);
    let moderated = || {
        response(
            "403 Forbidden",
            &[("Content-Type", "application/json")],
            r#"{"errors":[{"code":0,"message":"User is moderated"}]}"#,
        )
    };
    let (base, server) = serve(vec![
        // The only session is banned: Roblox refuses it even with a CSRF token...
        response("403 Forbidden", &[("x-csrf-token", "csrf")], ""),
        moderated(),
        // ...so Nocta asks without a session and still learns who is online.
        json(r#"{"userPresences":[{"userPresenceType":1,"userId":156,"lastLocation":"Website"}]}"#),
        // Friends need a session, so the ban is named instead of a vague failure.
        moderated(),
    ])
    .await;
    let (launcher, _, accounts) = fixture(&base).await;
    accounts
        .replace_cookie(1, "_|WARNING:synthetic-cookie", "156")
        .await
        .unwrap();
    let cancel = CancellationToken::new();
    assert_eq!(
        launcher.presence.refresh_presence(&cancel).await.unwrap(),
        1
    );
    let presence = accounts.get(1).await.unwrap().presence.unwrap();
    assert_eq!(presence.user_presence_type, 1);
    let error = launcher.presence.friends_of(1, &cancel).await.unwrap_err();
    assert_eq!(error.to_string(), crate::roblox::web::MODERATED);
    let requests = server.await.unwrap();
    assert!(
        !requests[2].to_ascii_lowercase().contains("cookie:"),
        "the anonymous request carries no session"
    );
}

#[tokio::test]
async fn friends_list_their_games_and_avatars_with_players_in_game_first() {
    let json = |body: &str| response("200 OK", &[("Content-Type", "application/json")], body);
    let (base, server) = serve(vec![
        json(r#"{"data":[{"id":2,"name":"amy","displayName":"Amy"},{"id":3,"name":"zed","displayName":"Zed"}]}"#),
        response("403 Forbidden", &[("x-csrf-token", "csrf")], ""),
        json(r#"{"userPresences":[{"userPresenceType":0,"userId":2},{"userPresenceType":2,"userId":3,"placeId":1818,"gameId":"job-9","lastLocation":"Crossroads"}]}"#),
        json(r#"{"data":[{"targetId":3,"imageUrl":"https://tr.rbxcdn.com/zed.png"}]}"#),
    ])
    .await;
    let (launcher, _, accounts) = fixture(&base).await;
    accounts
        .replace_cookie(1, "_|WARNING:synthetic-cookie", "156")
        .await
        .unwrap();
    let friends = launcher
        .presence
        .friends_of(1, &CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(
        friends[0].user.name, "zed",
        "the friend in a game comes first"
    );
    assert_eq!(
        friends[0].presence.as_ref().unwrap().job_id.as_deref(),
        Some("job-9")
    );
    assert_eq!(
        friends[0].avatar_url.as_deref(),
        Some("https://tr.rbxcdn.com/zed.png")
    );
    assert_eq!(friends[1].presence.as_ref().unwrap().kind, 0);
    let requests = server.await.unwrap();
    assert!(requests[0].starts_with("GET /v1/users/156/friends"));
}

#[tokio::test]
async fn a_schedule_closes_only_the_clients_it_started() {
    let launch_responses = || {
        vec![
            identity(),
            response("403 Forbidden", &[("x-csrf-token", "join-csrf")], ""),
            response("200 OK", &[], ""),
            response("403 Forbidden", &[("x-csrf-token", "ticket-csrf")], ""),
            response(
                "200 OK",
                &[("rbx-authentication-ticket", "synthetic-ticket")],
                "",
            ),
        ]
    };
    let mut responses = launch_responses();
    responses.extend(launch_responses());
    let (base, server) = serve(responses).await;
    let (launcher, platform, accounts) = fixture(&base).await;
    let config = AppConfig {
        place_id: "920587237".into(),
        multi_instance: false,
        pre_launch_delay_secs: 0,
        ..Default::default()
    };
    accounts.save_config(&config).await.unwrap();
    let scheduled = platform.process("RobloxPlayerBeta.exe", true, 1);
    let manual = platform.process("RobloxPlayerBeta.exe", true, 1);
    platform
        .spawn_on_dispatch
        .lock()
        .unwrap()
        .push(scheduled.clone());
    let preset = crate::models::LaunchPreset {
        id: "p".into(),
        name: "Morning".into(),
        account_ids: vec![1],
        target: crate::models::PresetTarget::Place {
            place_id: "920587237".into(),
        },
        afk: false,
        hotkey: String::new(),
    };
    let (run, tasks) = super::automation::Automation::new(launcher.clone())
        .start_scheduled(&preset, config.clone())
        .await
        .unwrap();
    wait_terminal(&launcher, tasks[0]).await;

    // The user launches the same account again by hand.
    platform
        .spawn_on_dispatch
        .lock()
        .unwrap()
        .push(manual.clone());
    let manual_tasks = launcher
        .start(&[1], LaunchTargetInput::Configured, config)
        .await
        .unwrap();
    wait_terminal(&launcher, manual_tasks[0]).await;

    assert_eq!(
        super::automation::Automation::new(launcher.clone())
            .close_run(&run)
            .await,
        1
    );
    let running: Vec<_> = launcher
        .instances
        .snapshot()
        .into_iter()
        .filter(|instance| instance.status != crate::models::InstanceStatus::Exited)
        .filter_map(|instance| instance.pid)
        .collect();
    assert_eq!(running, vec![manual.pid], "the manual client keeps running");
    assert_eq!(
        super::automation::Automation::new(launcher.clone())
            .close_run(&run)
            .await,
        0,
        "a run closes only once"
    );
    server.await.unwrap();
}

#[tokio::test]
async fn upkeep_counts_playtime_waits_for_the_user_before_anti_afk_and_enforces_memory() {
    let (base, server) = serve(vec![
        identity(),
        response("403 Forbidden", &[("x-csrf-token", "join-csrf")], ""),
        response("200 OK", &[], ""),
        response("403 Forbidden", &[("x-csrf-token", "ticket-csrf")], ""),
        response(
            "200 OK",
            &[("rbx-authentication-ticket", "synthetic-ticket")],
            "",
        ),
    ])
    .await;
    let (launcher, platform, accounts) = fixture(&base).await;
    let mut config = AppConfig {
        place_id: "920587237".into(),
        multi_instance: false,
        pre_launch_delay_secs: 0,
        anti_afk: true,
        anti_afk_minutes: 1,
        anti_afk_random: false,
        trim_memory: true,
        ..Default::default()
    };
    accounts.save_config(&config).await.unwrap();
    let client = platform.process("RobloxPlayerBeta.exe", true, 1);
    platform
        .spawn_on_dispatch
        .lock()
        .unwrap()
        .push(client.clone());
    let tasks = launcher
        .start(&[1], LaunchTargetInput::Configured, config.clone())
        .await
        .unwrap();
    wait_terminal(&launcher, tasks[0]).await;

    let mut state = crate::launch::client_care::Upkeep::default();
    let t0 = std::time::Instant::now();
    let at = |secs: u64| t0 + std::time::Duration::from_secs(secs);
    crate::launch::client_care::ClientCare::new(launcher.clone())
        .upkeep(&mut state, at(0))
        .await;
    // The first pass only starts the clock: a client still loading keeps its memory.
    assert!(platform.trimmed.lock().unwrap().is_empty());
    crate::launch::client_care::ClientCare::new(launcher.clone())
        .upkeep(&mut state, at(30))
        .await;
    assert!(platform.nudged.lock().unwrap().is_empty(), "not due yet");

    // Due after a minute, but the user is typing: anti-AFK waits instead of taking focus.
    *platform.user_idle.lock().unwrap() = Some(1);
    crate::launch::client_care::ClientCare::new(launcher.clone())
        .upkeep(&mut state, at(61))
        .await;
    crate::launch::client_care::ClientCare::new(launcher.clone())
        .upkeep(&mut state, at(90))
        .await;
    assert!(platform.nudged.lock().unwrap().is_empty());
    *platform.user_idle.lock().unwrap() = Some(100);
    crate::launch::client_care::ClientCare::new(launcher.clone())
        .upkeep(&mut state, at(110))
        .await;
    assert_eq!(*platform.nudged.lock().unwrap(), vec![client.pid]);
    assert_eq!(accounts.get(1).await.unwrap().playtime_secs, 110);
    // Trimming runs every five minutes, not every pass.
    assert!(platform.trimmed.lock().unwrap().is_empty());
    crate::launch::client_care::ClientCare::new(launcher.clone())
        .upkeep(&mut state, at(301))
        .await;
    assert_eq!(*platform.trimmed.lock().unwrap(), vec![client.pid]);

    // A client over the memory limit is closed.
    config.memory_limit_mb = 1024;
    accounts.save_config(&config).await.unwrap();
    platform.memory.lock().unwrap().insert(client.pid, 5000);
    crate::launch::client_care::ClientCare::new(launcher.clone())
        .upkeep(&mut state, at(120))
        .await;
    assert_eq!(platform.killed.lock().unwrap().len(), 1);
    let logs = serde_json::to_string(&accounts.logs().await.unwrap()).unwrap();
    assert!(logs.contains("used 5000 MB (limit 1024 MB); it was closed"));
    assert!(logs.contains("Anti-AFK kept 1 Roblox clients active"));
    server.await.unwrap();
}

#[tokio::test]
async fn robux_balances_are_checked_with_each_accounts_own_session() {
    let json = |body: &str| response("200 OK", &[("Content-Type", "application/json")], body);
    let (base, server) = serve(vec![json(r#"{"robux":1250}"#)]).await;
    let (launcher, _, accounts) = fixture(&base).await;
    accounts
        .replace_cookie(1, "_|WARNING:synthetic-cookie", "156")
        .await
        .unwrap();
    accounts
        .import("NoId:pw:synthetic-cookie-2".into(), None, false)
        .await
        .unwrap();
    let updated = launcher
        .presence
        .refresh_robux(&[1, 2], &CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(updated, 1, "the account without a Roblox ID is skipped");
    let alice = accounts.get(1).await.unwrap();
    assert_eq!(alice.robux, Some(1250));
    assert!(alice.robux_checked_at.is_some());
    let requests = server.await.unwrap();
    assert!(requests[0].starts_with("GET /v1/users/156/currency"));
    assert!(requests[0].contains("synthetic-cookie"));
    let logs = serde_json::to_string(&accounts.logs().await.unwrap()).unwrap();
    assert!(logs.contains("Robux not checked"));
}

#[tokio::test]
async fn command_line_launches_accounts_by_name_or_alias_and_runs_presets_by_name() {
    let launch_responses = || {
        vec![
            identity(),
            response("403 Forbidden", &[("x-csrf-token", "join-csrf")], ""),
            response("200 OK", &[], ""),
            response("403 Forbidden", &[("x-csrf-token", "ticket-csrf")], ""),
            response(
                "200 OK",
                &[("rbx-authentication-ticket", "synthetic-ticket")],
                "",
            ),
        ]
    };
    let mut responses = launch_responses();
    responses.extend(launch_responses());
    let (base, server) = serve(responses).await;
    let (launcher, platform, accounts) = fixture(&base).await;
    accounts
        .update_details(
            1,
            crate::models::AccountDetails {
                alias: "Main".into(),
                note: String::new(),
                color: None,
                pinned: false,
                is_main: false,
                fields: Vec::new(),
                afk_point: None,
            },
        )
        .await
        .unwrap();
    let config = AppConfig {
        multi_instance: false,
        pre_launch_delay_secs: 0,
        launch_presets: vec![crate::models::LaunchPreset {
            id: "p1".into(),
            name: "Morning farm".into(),
            account_ids: vec![1],
            target: crate::models::PresetTarget::Place {
                place_id: "920587237".into(),
            },
            afk: false,
            hotkey: String::new(),
        }],
        ..Default::default()
    };
    accounts.save_config(&config).await.unwrap();
    for _ in 0..2 {
        let process = platform.process("RobloxPlayerBeta.exe", true, 1);
        platform.spawn_on_dispatch.lock().unwrap().push(process);
    }

    let command = crate::cli::parse(&["nocta", "--launch", "920587237", "--accounts", "main"])
        .unwrap()
        .unwrap();
    let message = super::automation::Automation::new(launcher.clone())
        .run_cli(command, config.clone())
        .await
        .unwrap();
    assert_eq!(message, "Command line: launching 1 accounts.");
    for _ in 0..500 {
        if platform.dispatched.lock().unwrap().len() == 1 {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    assert!(platform.dispatched.lock().unwrap()[0].contains("920587237"));

    let unknown = crate::cli::parse(&["nocta", "--launch", "1", "--accounts", "nobody"])
        .unwrap()
        .unwrap();
    let error = super::automation::Automation::new(launcher.clone())
        .run_cli(unknown, config.clone())
        .await
        .unwrap_err();
    assert_eq!(error.to_string(), "There is no account \"nobody\".");

    // Wait for the first launch to finish before the preset uses the same account.
    for _ in 0..500 {
        if launcher.sessions.tasks.ensure_idle(&[1]).is_ok() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    let preset = crate::cli::parse(&["nocta", "--preset", "morning FARM"])
        .unwrap()
        .unwrap();
    let message = super::automation::Automation::new(launcher.clone())
        .run_cli(preset, config)
        .await
        .unwrap();
    assert_eq!(
        message,
        "Command line: started preset \"Morning farm\" (1 accounts)."
    );
    for _ in 0..500 {
        if platform.dispatched.lock().unwrap().len() == 2 {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    assert_eq!(platform.dispatched.lock().unwrap().len(), 2);
    server.await.unwrap();
}
