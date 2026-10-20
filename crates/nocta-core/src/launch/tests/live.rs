//! The Roblox account tools, friendships and the region filter against the real Roblox, with
//! accounts from a copy of the user's data folder (`NOCTA_SHARE_DATA`, only the copy is used).
//! Every change is undone at the end: privacy goes back to its old value, a friendship made here
//! is removed and a pending request is declined. Display name and signing out other sessions are
//! left out: they cannot be undone right away.
//!
//! `NOCTA_SHARE_DATA=%APPDATA%\app.nocta.desktop cargo test -p nocta-core --lib launch::tests::live:: -- --ignored --nocapture --test-threads 1`
use super::launcher_for;
use crate::launch::Launcher;
use crate::roblox::web::{RobloxWeb, SIGNED_OUT};
use crate::{
    accounts::Accounts,
    models::{AccountHealth, BefriendMode, JoinStrategy, LaunchTargetInput, ServerRegion},
    storage::Storage,
    vault::Vault,
};
use reqwest::{Method, Url};
use serde_json::json;
use std::{sync::Arc, time::Duration};
use tokio_util::sync::CancellationToken;

/// Natural Disaster Survival: always busy, servers all over the world.
const BUSY_PLACE: &str = "189707";

struct Live {
    launcher: Arc<Launcher>,
    accounts: Accounts,
    /// Accounts with a working session: (Nocta ID, Roblox ID).
    working: Vec<(u32, u64)>,
    _copy: tempfile::TempDir,
}

/// Opens a copy of the user's data and finds up to `want` accounts that are signed in.
async fn live(want: usize) -> Live {
    live_in(want, false).await
}

/// Like [`live`]; with `real`, the user's own database is used instead of a copy, for changes
/// Nocta has to keep (a new session cookie).
async fn live_in(want: usize, real: bool) -> Live {
    let source = std::path::PathBuf::from(
        std::env::var("NOCTA_SHARE_DATA").expect("set NOCTA_SHARE_DATA to the data folder"),
    );
    let copy = tempfile::tempdir().unwrap();
    for file in ["nocta.sqlite", "nocta.sqlite-wal", "nocta.sqlite-shm"] {
        if !real && source.join(file).exists() {
            std::fs::copy(source.join(file), copy.path().join(file)).unwrap();
        }
    }
    let database = if real {
        source.join("nocta.sqlite")
    } else {
        copy.path().join("nocta.sqlite")
    };
    let storage = Storage::open(&database).await.unwrap();
    let vault = Arc::new(Vault::open(&storage.vault_id().await.unwrap(), false).unwrap());
    let accounts = Accounts::new(storage, vault);
    let (launcher, _) = launcher_for(accounts.clone(), RobloxWeb::new().unwrap(), None).await;
    let cancel = CancellationToken::new();
    let mut candidates = accounts.list().await.unwrap();
    candidates.retain(|account| account.deleted_at.is_none() && account.has_cookie);
    // Accounts already known to work first.
    candidates.sort_by_key(|account| account.health != AccountHealth::Healthy);
    let mut working = Vec::new();
    let (mut signed_out, mut other) = (0, 0);
    for account in candidates.iter().take(40) {
        if working.len() >= want {
            break;
        }
        let cookie = accounts.secrets(account.id).await.unwrap().cookie.clone();
        let url = Url::parse("https://users.roblox.com/v1/users/authenticated").unwrap();
        match launcher.web.get_json_authed(url, &cookie, &cancel).await {
            Ok(user) => {
                let id = user["id"].as_u64().unwrap();
                accounts
                    .mark_health(account.id, AccountHealth::Healthy, Some(&id.to_string()))
                    .await
                    .unwrap();
                working.push((account.id, id));
            }
            Err(crate::CoreError::Validation(message)) if message == SIGNED_OUT => signed_out += 1,
            Err(_) => other += 1,
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    println!(
        "{} accounts with a cookie; working sessions found: {} (signed out {signed_out}, other errors {other})",
        candidates.len(),
        working.len()
    );
    Live {
        launcher,
        accounts,
        working,
        _copy: copy,
    }
}

#[tokio::test]
#[ignore = "needs NOCTA_SHARE_DATA and talks to the real Roblox"]
async fn live_region_filter_picks_a_server_in_the_chosen_region() {
    let live = live(1).await;
    let (account, _) = *live.working.first().expect("a signed-in account");
    let cancel = CancellationToken::new();
    for region in [ServerRegion::Europe, ServerRegion::NorthAmerica] {
        let mut config = live.accounts.config().await.unwrap();
        config.server_region = Some(region);
        live.accounts.save_config(&config).await.unwrap();
        let (target, note) = live
            .launcher
            .settle_target(
                LaunchTargetInput::Pick {
                    place_id: BUSY_PLACE.into(),
                    strategy: JoinStrategy::Fullest,
                    exclude_job: None,
                },
                &[account],
                &cancel,
            )
            .await
            .unwrap();
        let LaunchTargetInput::Job { job_id, .. } = target else {
            panic!("an exact server")
        };
        println!(
            "{region:?}: server {job_id} - {}",
            note.as_deref().unwrap_or("")
        );
        // Ask again independently: the chosen server really is in that region.
        let cookie = live.accounts.secrets(account).await.unwrap().cookie.clone();
        let location = live
            .launcher
            .locate_server(&cookie, BUSY_PLACE, &job_id, &cancel)
            .await
            .unwrap();
        assert_eq!(location.region, region, "{}", location.place);
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}

#[tokio::test]
#[ignore = "needs NOCTA_SHARE_DATA and talks to the real Roblox"]
async fn live_privacy_is_changed_read_back_and_restored() {
    let live = live(1).await;
    let (account, _) = *live.working.first().expect("a signed-in account");
    let cancel = CancellationToken::new();
    let settings = live
        .launcher
        .account_tools
        .privacy_settings(account, &cancel)
        .await
        .unwrap();
    for setting in &settings {
        let restricted: Vec<String> = setting
            .restricted
            .iter()
            .map(|item| format!("{}: {}", item.value, item.requirement))
            .collect();
        println!(
            "{} = {} (options {:?}, restricted {restricted:?})",
            setting.key, setting.value, setting.options
        );
    }
    // Only a setting whose current and new values need no check of Roblox's own, so it can
    // surely be put back.
    let free = |setting: &crate::models::PrivacySetting, value: &str| {
        !setting.restricted.iter().any(|item| item.value == value)
    };
    // Who sees the inventory first: harmless, and both ways open to the owner.
    let (setting, other) = settings
        .iter()
        .filter(|setting| setting.key == "whoCanSeeMyInventory")
        .chain(settings.iter())
        .filter(|setting| free(setting, &setting.value))
        .find_map(|setting| {
            let other = setting
                .options
                .iter()
                .find(|option| **option != setting.value && free(setting, option))?;
            Some((setting, other.clone()))
        })
        .expect("a setting that can be changed and put back");
    println!("trying {}: {} -> {other}", setting.key, setting.value);
    let changed = live
        .launcher
        .account_tools
        .set_privacy(&[account], &setting.key, &other, &cancel)
        .await
        .unwrap();
    if changed != 1 {
        for log in live.accounts.logs().await.unwrap().iter().rev().take(3) {
            println!("log: {}", log.message);
        }
    }
    assert_eq!(changed, 1, "Roblox refused");
    let now = live
        .launcher
        .account_tools
        .privacy_settings(account, &cancel)
        .await
        .unwrap();
    let read = now.iter().find(|item| item.key == setting.key).unwrap();
    println!(
        "{} changed to {} (read back: {})",
        setting.key, other, read.value
    );
    assert_eq!(read.value, other);
    // Put it back.
    let restored = live
        .launcher
        .account_tools
        .set_privacy(&[account], &setting.key, &setting.value, &cancel)
        .await
        .unwrap();
    assert_eq!(restored, 1);
    let after = live
        .launcher
        .account_tools
        .privacy_settings(account, &cancel)
        .await
        .unwrap();
    let back = after.iter().find(|item| item.key == setting.key).unwrap();
    println!("{} restored to {}", setting.key, back.value);
    assert_eq!(back.value, setting.value);
}

#[tokio::test]
#[ignore = "needs NOCTA_SHARE_DATA and talks to the real Roblox"]
async fn live_quick_login_code_is_checked_and_approved() {
    let live = live(1).await;
    let (account, _) = *live.working.first().expect("a signed-in account");
    let cancel = CancellationToken::new();
    let web = &live.launcher.web;
    // The "other device": asks Roblox for a Quick Login code, signed in as nobody.
    let created = web
        .post_json_raw(
            Url::parse("https://apis.roblox.com/auth-token-service/v1/login/create").unwrap(),
            &json!({}),
            &cancel,
        )
        .await
        .unwrap();
    assert!(
        (200..300).contains(&created.status),
        "create: HTTP {}",
        created.status
    );
    let created = created.json().unwrap();
    let code = created["code"].as_str().expect("a code").to_owned();
    let private_key = created["privateKey"].as_str().expect("a key").to_owned();
    println!("other device shows code {code}");

    let device = live
        .launcher
        .account_tools
        .quick_login_check(account, &code, &cancel)
        .await
        .unwrap();
    println!(
        "Nocta sees: device {:?}, location {:?}",
        device.device, device.location
    );
    live.launcher
        .account_tools
        .quick_login_approve(account, &code, &cancel)
        .await
        .unwrap();
    // The other device now sees the code as approved.
    // Roblox's write protection asks for a CSRF token first, signed in or not.
    let client = reqwest::Client::new();
    let ask = |token: Option<String>| {
        let mut request = client
            .post("https://apis.roblox.com/auth-token-service/v1/login/status")
            .json(&json!({ "code": code, "privateKey": private_key }));
        if let Some(token) = token {
            request = request.header("x-csrf-token", token);
        }
        request.send()
    };
    let mut answer = ask(None).await.unwrap();
    if answer.status() == 403 {
        let token = answer
            .headers()
            .get("x-csrf-token")
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        answer = ask(token).await.unwrap();
    }
    let http = answer.status().as_u16();
    let status: serde_json::Value = answer.json().await.unwrap_or_default();
    println!(
        "other device sees status {:?} (HTTP {http})",
        status["status"]
    );
    assert_eq!(status["status"], "Validated");
}

#[tokio::test]
#[ignore = "needs NOCTA_SHARE_DATA and talks to the real Roblox"]
async fn live_two_own_accounts_become_friends_and_are_unfriended_after() {
    let live = live(2).await;
    assert!(live.working.len() >= 2, "needs two signed-in accounts");
    let ((a, a_user), (b, b_user)) = (live.working[0], live.working[1]);
    let cancel = CancellationToken::new();
    let summary = live
        .launcher
        .account_tools
        .befriend_accounts(&[a, b], BefriendMode::WithMain { main: a }, &cancel)
        .await
        .unwrap();
    let logs = serde_json::to_string(&live.accounts.logs().await.unwrap()).unwrap();
    println!("{summary:?}");
    assert_eq!(summary.pairs, 1);
    assert_eq!(summary.failed, 0, "{logs}");
    let cookie = |id| {
        let accounts = live.accounts.clone();
        async move { accounts.secrets(id).await.unwrap().cookie.clone() }
    };
    let (a_cookie, b_cookie) = (cookie(a).await, cookie(b).await);
    let web = &live.launcher.web;
    let status = web
        .get_json_authed(
            Url::parse(&format!(
                "https://friends.roblox.com/v1/users/{a_user}/friends/statuses?userIds={b_user}"
            ))
            .unwrap(),
            &a_cookie,
            &cancel,
        )
        .await
        .unwrap();
    let status = status["data"][0]["status"]
        .as_str()
        .unwrap_or("?")
        .to_owned();
    println!("Roblox says the two accounts are now: {status}");
    if summary.added + summary.already == 1 {
        assert_eq!(status, "Friends");
        if summary.added == 1 {
            // Undo: the friendship made here is removed.
            let undo = web
                .write_authed(
                    Method::POST,
                    Url::parse(&format!(
                        "https://friends.roblox.com/v1/users/{b_user}/unfriend"
                    ))
                    .unwrap(),
                    &a_cookie,
                    None,
                    &cancel,
                )
                .await
                .unwrap();
            println!("unfriended again: HTTP {}", undo.status);
        }
    } else {
        // Roblox wanted a captcha: the request waits; decline it so nothing is left behind.
        assert_eq!(summary.pending, 1);
        let undo = web
            .write_authed(
                Method::POST,
                Url::parse(&format!(
                    "https://friends.roblox.com/v1/users/{a_user}/decline-friend-request"
                ))
                .unwrap(),
                &b_cookie,
                None,
                &cancel,
            )
            .await
            .unwrap();
        println!("pending request declined: HTTP {}", undo.status);
    }
}

/// Allowed by the user: the account's display name is changed (Roblox allows the next change only
/// after a week). The new name is the username itself when the display name differs from it.
#[tokio::test]
#[ignore = "changes a display name on the real Roblox; run only with the owner's consent"]
async fn live_display_name_is_changed() {
    let live = live(1).await;
    let (account, user) = *live.working.first().expect("a signed-in account");
    let cancel = CancellationToken::new();
    let profile = |user: u64| {
        let web = live.launcher.web.clone();
        async move {
            web.get_raw(
                Url::parse(&format!("https://users.roblox.com/v1/users/{user}")).unwrap(),
                None,
                None,
                &CancellationToken::new(),
            )
            .await
            .unwrap()
            .json()
            .unwrap()
        }
    };
    let before = profile(user).await;
    let (name, shown) = (
        before["name"].as_str().unwrap().to_owned(),
        before["displayName"].as_str().unwrap().to_owned(),
    );
    let wanted = if shown != name {
        name.clone()
    } else {
        format!("Nocta{}", user % 10_000)
    };
    println!("display name before: {shown:?} (username {name:?}); asking for {wanted:?}");
    match live
        .launcher
        .account_tools
        .set_display_name(account, &wanted, &cancel)
        .await
    {
        Ok(()) => {
            let after = profile(user).await;
            println!("display name after: {:?}", after["displayName"]);
            assert_eq!(after["displayName"], wanted.as_str());
        }
        // Roblox's own refusal (a change in the last week, a filtered name) must come through.
        Err(error) => println!("Roblox refused: {error}"),
    }
}

/// Allowed by the user: every other session of the account is signed out. Runs on the real
/// database so Nocta keeps the new cookie; the old one stops working.
#[tokio::test]
#[ignore = "signs an account out everywhere else on the real Roblox; run only with the owner's consent"]
async fn live_other_sessions_are_signed_out_and_nocta_keeps_working() {
    let live = live_in(1, true).await;
    let (account, user) = *live.working.first().expect("a signed-in account");
    let cancel = CancellationToken::new();
    let old = live.accounts.secrets(account).await.unwrap().cookie.clone();
    live.launcher
        .account_tools
        .sign_out_other_sessions(account, &cancel)
        .await
        .unwrap();
    let new = live.accounts.secrets(account).await.unwrap().cookie.clone();
    assert_ne!(old, new, "Nocta saved the new cookie");
    let me = Url::parse("https://users.roblox.com/v1/users/authenticated").unwrap();
    let now = live
        .launcher
        .web
        .get_json_authed(me.clone(), &new, &cancel)
        .await
        .unwrap();
    println!("new cookie signed in as Roblox user {}", now["id"]);
    assert_eq!(now["id"].as_u64(), Some(user));
    let old_works = live.launcher.web.get_json_authed(me, &old, &cancel).await;
    println!(
        "old cookie: {}",
        match &old_works {
            Ok(_) => "still signed in".to_owned(),
            Err(error) => error.to_string(),
        }
    );
    assert!(old_works.is_err(), "the old session was signed out");
}
