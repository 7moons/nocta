//! Account changes on Roblox and friendships among the user's own accounts, against a fixture
//! server that records every request.
use super::fixture;
use crate::roblox::web::fixture::{response, serve};
use crate::{
    accounts::Accounts,
    models::{BefriendMode, BefriendSummary},
    CoreError,
};
use tokio_util::sync::CancellationToken;

fn json(body: &str) -> String {
    response("200 OK", &[("Content-Type", "application/json")], body)
}

fn csrf() -> String {
    response("403 Forbidden", &[("x-csrf-token", "synthetic-csrf")], "")
}

async fn with_ids(accounts: &Accounts, names: &[&str]) {
    accounts
        .replace_cookie(1, "cookie-alice", "156")
        .await
        .unwrap();
    for (index, name) in names.iter().enumerate() {
        let id = index as u32 + 2;
        accounts
            .import(format!("{name}:pw:cookie-{name}"), None, false)
            .await
            .unwrap();
        accounts
            .replace_cookie(
                id,
                &format!("cookie-{}", name.to_lowercase()),
                &(156 + id - 1).to_string(),
            )
            .await
            .unwrap();
    }
}

fn line(request: &str) -> &str {
    request.lines().next().unwrap_or_default()
}

#[tokio::test]
async fn signing_out_other_sessions_keeps_nocta_signed_in_with_the_new_cookie() {
    let (base, server) = serve(vec![
        csrf(),
        response(
            "200 OK",
            &[(
                "Set-Cookie",
                ".ROBLOSECURITY=_|WARNING:-rotated-cookie; domain=.roblox.com; path=/; secure; HttpOnly",
            )],
            "{}",
        ),
        csrf(),
        response("200 OK", &[], "{}"),
    ])
    .await;
    let (launcher, _, accounts) = fixture(&base).await;
    with_ids(&accounts, &[]).await;
    let cancel = CancellationToken::new();
    launcher
        .account_tools
        .sign_out_other_sessions(1, &cancel)
        .await
        .unwrap();
    assert_eq!(
        accounts.secrets(1).await.unwrap().cookie,
        "_|WARNING:-rotated-cookie"
    );
    // Without a new cookie Nocta says so instead of losing the session silently.
    let error = launcher
        .account_tools
        .sign_out_other_sessions(1, &cancel)
        .await
        .unwrap_err();
    assert!(error.to_string().contains("sent no new cookie"), "{error}");
    let requests = server.await.unwrap();
    assert_eq!(
        line(&requests[0]),
        "POST /v1/logoutfromallsessionsandreauthenticate HTTP/1.1"
    );
    assert!(requests[0].contains("cookie-alice"));
    assert!(requests[1].contains("x-csrf-token: synthetic-csrf"));
    assert!(
        requests[2].contains("rotated-cookie"),
        "the new cookie is used next time"
    );
}

#[tokio::test]
async fn privacy_is_read_with_its_options_and_set_on_many_accounts() {
    let settings = || {
        json(
            r#"{"whoCanJoinMeInExperiences":{"currentValue":"Friends","options":[{"option":{"optionValue":"AllUsers"}},{"option":{"optionValue":"Friends"}},{"option":{"optionValue":"NoOne"}}]},"whoCanChatWithMeInExperiences":{"currentValue":"NoOne","options":[{"option":{"optionValue":"AllUsers"},"requirement":"FacialAgeEstimation"},{"option":{"optionValue":"NoOne"}}]}}"#,
        )
    };
    let (base, server) = serve(vec![
        settings(),
        // Alice: options checked, then written.
        settings(),
        csrf(),
        json("{}"),
        // Bob: options checked, Roblox refuses with a reason.
        settings(),
        csrf(),
        response(
            "400 Bad Request",
            &[("Content-Type", "application/json")],
            r#"{"errors":[{"code":1,"message":"This setting is locked by parental controls."}]}"#,
        ),
        // An option behind Roblox's age check is refused before anything is written.
        settings(),
    ])
    .await;
    let (launcher, _, accounts) = fixture(&base).await;
    with_ids(&accounts, &["Bob"]).await;
    let cancel = CancellationToken::new();
    let read = launcher
        .account_tools
        .privacy_settings(1, &cancel)
        .await
        .unwrap();
    assert_eq!(read[1].key, "whoCanJoinMeInExperiences");
    assert_eq!(read[1].options, ["AllUsers", "Friends", "NoOne"]);
    assert_eq!(read[0].restricted[0].requirement, "FacialAgeEstimation");
    let changed = launcher
        .account_tools
        .set_privacy(&[1, 2], "whoCanJoinMeInExperiences", "AllUsers", &cancel)
        .await
        .unwrap();
    assert_eq!(changed, 1);
    let locked = launcher
        .account_tools
        .set_privacy(&[1], "whoCanChatWithMeInExperiences", "AllUsers", &cancel)
        .await
        .unwrap();
    assert_eq!(locked, 0);
    let logs = serde_json::to_string(&accounts.logs().await.unwrap()).unwrap();
    assert!(logs.contains("whoCanJoinMeInExperiences set to AllUsers"));
    assert!(
        logs.contains("locked by parental controls"),
        "Roblox's reason is kept"
    );
    assert!(logs.contains("only allows AllUsers after its own check (FacialAgeEstimation)"));
    assert!(launcher
        .account_tools
        .set_privacy(&[1], "allowVoice", "On", &cancel)
        .await
        .is_err());
    let requests = server.await.unwrap();
    assert_eq!(
        requests.len(),
        8,
        "nothing is written for the locked option"
    );
    assert_eq!(
        line(&requests[0]),
        "GET /user-settings-api/v1/user-settings/settings-and-options HTTP/1.1"
    );
    assert_eq!(
        line(&requests[3]),
        "POST /user-settings-api/v1/user-settings HTTP/1.1"
    );
    assert!(requests[3].ends_with(r#"{"whoCanJoinMeInExperiences":"AllUsers"}"#));
    assert!(
        requests[3].contains("RBXEventTrackerV2=CreateDate="),
        "writes carry a browser tracker, like a browser"
    );
    assert!(
        requests[6].contains("cookie-bob"),
        "each account uses its own session"
    );
}

#[tokio::test]
async fn display_name_and_quick_login_use_the_accounts_session() {
    let (base, server) = serve(vec![
        // Display name.
        csrf(),
        json("{}"),
        // Roblox wants the owner to confirm.
        response(
            "403 Forbidden",
            &[("rblx-challenge-id", "abc"), ("rblx-challenge-type", "twostepverification")],
            r#"{"errors":[{"code":0,"message":"Challenge is required"}]}"#,
        ),
        // Quick Login: look the code up, then approve it.
        csrf(),
        json(r#"{"code":"AB3D9Z","status":"UserLinked","deviceInfo":"Chrome on Windows","location":"Warsaw, Poland"}"#),
        csrf(),
        json("{}"),
    ])
    .await;
    let (launcher, _, accounts) = fixture(&base).await;
    with_ids(&accounts, &[]).await;
    let cancel = CancellationToken::new();
    launcher
        .account_tools
        .set_display_name(1, "Night Farmer", &cancel)
        .await
        .unwrap();
    let challenged = launcher
        .account_tools
        .set_display_name(1, "Day Farmer", &cancel)
        .await
        .unwrap_err();
    assert!(
        challenged.to_string().contains("confirm it's you"),
        "{challenged}"
    );
    assert!(matches!(
        launcher
            .account_tools
            .set_display_name(1, "x", &cancel)
            .await,
        Err(CoreError::Validation(_))
    ));
    let device = launcher
        .account_tools
        .quick_login_check(1, "ab3-d9z", &cancel)
        .await
        .unwrap();
    assert_eq!(device.device, "Chrome on Windows");
    assert_eq!(device.location, "Warsaw, Poland");
    launcher
        .account_tools
        .quick_login_approve(1, "AB3D9Z", &cancel)
        .await
        .unwrap();
    let requests = server.await.unwrap();
    let lines: Vec<&str> = requests.iter().map(|request| line(request)).collect();
    assert_eq!(
        lines,
        [
            "PATCH /v1/users/156/display-names HTTP/1.1",
            "PATCH /v1/users/156/display-names HTTP/1.1",
            "PATCH /v1/users/156/display-names HTTP/1.1",
            "POST /auth-token-service/v1/login/enterCode HTTP/1.1",
            "POST /auth-token-service/v1/login/enterCode HTTP/1.1",
            "POST /auth-token-service/v1/login/validateCode HTTP/1.1",
            "POST /auth-token-service/v1/login/validateCode HTTP/1.1",
        ]
    );
    assert!(requests[1].ends_with(r#"{"newDisplayName":"Night Farmer"}"#));
    assert!(requests[4].ends_with(r#"{"code":"AB3D9Z"}"#));
    assert!(requests
        .iter()
        .all(|request| request.contains("cookie-alice")));
}

#[tokio::test]
async fn own_accounts_become_friends_skipping_existing_friends_and_captchas() {
    let (base, server) = serve(vec![
        // Alice and Bob: not friends; Alice asks, Bob accepts.
        json(r#"{"data":[{"id":157,"status":"NotFriends"}]}"#),
        csrf(),
        json(r#"{"success":true,"isCaptchaRequired":false}"#),
        csrf(),
        json("{}"),
        // Alice and Cara: friends already.
        json(r#"{"data":[{"id":158,"status":"Friends"}]}"#),
        // Bob and Cara: Roblox wants a captcha for the request.
        json(r#"{"data":[{"id":158,"status":"NotFriends"}]}"#),
        csrf(),
        json(r#"{"success":false,"isCaptchaRequired":true}"#),
    ])
    .await;
    let (launcher, _, accounts) = fixture(&base).await;
    with_ids(&accounts, &["Bob", "Cara"]).await;
    let cancel = CancellationToken::new();
    let summary = launcher
        .account_tools
        .befriend_accounts(&[1, 2, 3], BefriendMode::Everyone, &cancel)
        .await
        .unwrap();
    assert_eq!(
        summary,
        BefriendSummary {
            pairs: 3,
            added: 1,
            already: 1,
            pending: 1,
            failed: 0,
        }
    );
    let requests = server.await.unwrap();
    assert_eq!(
        line(&requests[0]),
        "GET /v1/users/156/friends/statuses?userIds=157 HTTP/1.1"
    );
    assert_eq!(
        line(&requests[2]),
        "POST /v1/users/157/request-friendship HTTP/1.1"
    );
    assert!(requests[2].contains("cookie-alice"));
    assert_eq!(
        line(&requests[4]),
        "POST /v1/users/156/accept-friend-request HTTP/1.1"
    );
    assert!(
        requests[4].contains("cookie-bob"),
        "Bob accepts with his own session"
    );
    assert!(requests[6].contains("cookie-bob"));

    // With a main account only its pairs are made, and too many pairs are refused up front.
    let ids: Vec<u32> = (1..=3).collect();
    let error = launcher
        .account_tools
        .befriend_accounts(&ids[..1], BefriendMode::WithMain { main: 1 }, &cancel)
        .await
        .unwrap_err();
    assert!(
        error.to_string().contains("at least two accounts"),
        "{error}"
    );
}

#[tokio::test]
async fn too_many_friendships_are_refused_before_any_request() {
    let (base, server) = serve(Vec::new()).await;
    let (launcher, _, accounts) = fixture(&base).await;
    let names: Vec<String> = (0..25).map(|index| format!("Alt{index}")).collect();
    let names: Vec<&str> = names.iter().map(String::as_str).collect();
    with_ids(&accounts, &names).await;
    let ids: Vec<u32> = (1..=26).collect();
    let error = launcher
        .account_tools
        .befriend_accounts(&ids, BefriendMode::Everyone, &CancellationToken::new())
        .await
        .unwrap_err();
    assert!(error.to_string().contains("325 friendships"), "{error}");
    assert!(server.await.unwrap().is_empty());
}

#[tokio::test]
async fn a_region_keeps_the_strategys_order_and_skips_servers_elsewhere() {
    use crate::models::{JoinStrategy, LaunchTargetInput, ServerRegion};
    let join = |address: &str| {
        json(&format!(
            r#"{{"jobId":"x","status":2,"joinScript":{{"MachineAddress":"10.1.2.3","UdmuxEndpoints":[{{"Address":"{address}","Port":50000}}]}}}}"#
        ))
    };
    let (base, server) = serve(vec![
        json(
            r#"{"data":[{"id":"busy-us","playing":30,"maxPlayers":40},{"id":"mid-eu","playing":20,"maxPlayers":40},{"id":"quiet-eu","playing":5,"maxPlayers":40}],"nextPageCursor":null}"#,
        ),
        // The fullest server runs in Los Angeles; Roblox queues the first ask.
        csrf(),
        json(r#"{"jobId":"x","status":22,"joinScript":null,"queuePosition":1}"#),
        csrf(),
        join("198.51.100.7"),
        json(r#"{"success":true,"continent_code":"NA","country_code":"US","city":"Los Angeles"}"#),
        // …the next one in Frankfurt.
        csrf(),
        join("198.51.100.8"),
        json(r#"{"success":true,"continent_code":"EU","country_code":"DE","city":"Frankfurt"}"#),
    ])
    .await;
    let (launcher, _, accounts) = fixture(&base).await;
    with_ids(&accounts, &[]).await;
    let mut config = accounts.config().await.unwrap();
    config.server_region = Some(ServerRegion::Europe);
    accounts.save_config(&config).await.unwrap();
    let (target, note) = launcher
        .settle_target(
            LaunchTargetInput::Pick {
                place_id: "77".into(),
                strategy: JoinStrategy::Fullest,
                exclude_job: None,
            },
            &[1],
            &CancellationToken::new(),
        )
        .await
        .unwrap();
    assert!(
        matches!(target, LaunchTargetInput::Job { ref job_id, .. } if job_id == "mid-eu"),
        "{target:?}"
    );
    assert_eq!(
        note.as_deref(),
        Some("Fullest server: 20 of 40 players. In Frankfurt, DE.")
    );
    let requests = server.await.unwrap();
    assert_eq!(line(&requests[2]), "POST /v1/join-game-instance HTTP/1.1");
    assert!(requests[2].contains(r#""gameId":"busy-us""#));
    assert!(requests[2].contains(r#""placeId":77"#));
    assert!(requests[2].contains("cookie-alice"));
    assert!(
        requests[2]
            .to_lowercase()
            .contains("user-agent: roblox/wininet"),
        "the join service only answers Roblox's client"
    );
    // The queued attempt is asked again, as the same attempt.
    let attempt = |request: &str| {
        request
            .split(r#""gameJoinAttemptId":""#)
            .nth(1)
            .map(|rest| rest[..36].to_owned())
    };
    assert_eq!(attempt(&requests[2]), attempt(&requests[4]));
    // Only the Roblox server's address goes to the location service, with no cookie.
    assert_eq!(
        line(&requests[5]),
        "GET /198.51.100.7?fields=success,continent_code,country_code,city HTTP/1.1"
    );
    assert!(!requests[5].to_lowercase().contains("cookie"));
}

#[tokio::test]
async fn a_region_with_no_matching_server_says_how_many_were_checked() {
    use crate::models::{JoinStrategy, LaunchTargetInput, ServerRegion};
    let (base, _server) = serve(vec![
        json(r#"{"data":[{"id":"only-us","playing":3,"maxPlayers":10}],"nextPageCursor":null}"#),
        csrf(),
        json(r#"{"jobId":"x","status":2,"joinScript":{"UdmuxEndpoints":[{"Address":"198.51.100.9"}]}}"#),
        json(r#"{"success":true,"continent_code":"NA","country_code":"US","city":"Secaucus"}"#),
    ])
    .await;
    let (launcher, _, accounts) = fixture(&base).await;
    with_ids(&accounts, &[]).await;
    let mut config = accounts.config().await.unwrap();
    config.server_region = Some(ServerRegion::Asia);
    accounts.save_config(&config).await.unwrap();
    let error = launcher
        .settle_target(
            LaunchTargetInput::Pick {
                place_id: "77".into(),
                strategy: JoinStrategy::Hop,
                exclude_job: None,
            },
            &[1],
            &CancellationToken::new(),
        )
        .await
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("None of the 1 servers checked is in Asia"),
        "{error}"
    );
}

#[tokio::test]
async fn a_friendship_roblox_checks_waits_and_a_second_run_finishes_it() {
    let (base, server) = serve(vec![
        // First run: Roblox asks for a captcha on the request.
        json(r#"{"data":[{"id":157,"status":"NotFriends"}]}"#),
        csrf(),
        response(
            "403 Forbidden",
            &[
                ("rblx-challenge-id", "abc"),
                ("rblx-challenge-type", "captcha"),
            ],
            r#"{"errors":[{"code":0,"message":"Challenge is required to authorize the request"}]}"#,
        ),
        // Second run: the request went out after all; Bob only accepts.
        json(r#"{"data":[{"id":157,"status":"RequestSent"}]}"#),
        csrf(),
        json("{}"),
    ])
    .await;
    let (launcher, _, accounts) = fixture(&base).await;
    with_ids(&accounts, &["Bob"]).await;
    let cancel = CancellationToken::new();
    let first = launcher
        .account_tools
        .befriend_accounts(&[1, 2], BefriendMode::Everyone, &cancel)
        .await
        .unwrap();
    assert_eq!((first.pending, first.failed), (1, 0), "{first:?}");
    let logs = serde_json::to_string(&accounts.logs().await.unwrap()).unwrap();
    assert!(
        logs.contains("Roblox wants a captcha for this right now"),
        "{logs}"
    );
    let second = launcher
        .account_tools
        .befriend_accounts(&[1, 2], BefriendMode::Everyone, &cancel)
        .await
        .unwrap();
    assert_eq!(second.added, 1, "{second:?}");
    let requests = server.await.unwrap();
    assert_eq!(
        line(&requests[5]),
        "POST /v1/users/156/accept-friend-request HTTP/1.1"
    );
    assert!(requests[5].contains("cookie-bob"));
}
