//! Join strategies, user lookup, avatars, presence and friends against the real Roblox API.
//!
//! `cargo test -p nocta-core --test real_social -- --ignored --nocapture`
//!
//! The account tests also need `NOCTA_SHARE_DATA=%APPDATA%\app.nocta.desktop`: the database is
//! copied first and only read; cookies are never printed.
use nocta_core::{
    accounts::Accounts,
    launch::{
        join::pick_server,
        servers::{ListRequest, ServerLists},
    },
    models::{JoinStrategy, ServerListOptions},
    roblox::{social::Social, web::RobloxWeb},
    storage::Storage,
    vault::Vault,
};
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

/// Natural Disaster Survival: always busy, many public servers.
const BUSY_PLACE: &str = "189707";

#[tokio::test]
#[ignore = "talks to the real Roblox API"]
async fn real_squad_lowest_ping_and_hop_pick_live_servers() {
    let web = Arc::new(RobloxWeb::new().unwrap());
    let servers = ServerLists::new(web);
    for (strategy, most_players_first) in [
        (JoinStrategy::Squad, true),
        (JoinStrategy::LowestPing, false),
        (JoinStrategy::Hop, false),
        (JoinStrategy::Fullest, true),
        (JoinStrategy::Emptiest, false),
    ] {
        // Roblox throttles quick server-list calls; space them out.
        tokio::time::sleep(std::time::Duration::from_secs(12)).await;
        let page = servers
            .list(ListRequest {
                request_key: &format!("real-{strategy:?}"),
                place_id: BUSY_PLACE,
                vip_cookie: None,
                include_vip: false,
                previous: None,
                options: &ServerListOptions {
                    most_players_first,
                    exclude_full: true,
                    cursor: None,
                },
            })
            .await
            .unwrap();
        assert!(!page.items.is_empty(), "no public servers listed");
        let needed = if matches!(strategy, JoinStrategy::Fullest | JoinStrategy::Emptiest) {
            1
        } else {
            3
        };
        let pick = pick_server(&page.items, strategy, needed, &[]).expect("a server");
        let row = page.items.iter().find(|row| row.id == pick.job_id).unwrap();
        println!(
            "{strategy:?}: {} ({}/{} players, ping {:?}) - {}",
            pick.job_id, row.playing, row.max_players, row.ping, pick.note
        );
        assert!(row.max_players - row.playing >= needed);
        if strategy == JoinStrategy::Squad {
            let fullest_with_room = page
                .items
                .iter()
                .filter(|row| row.joinable && row.max_players - row.playing >= 3)
                .map(|row| row.playing)
                .max()
                .unwrap();
            assert_eq!(row.playing, fullest_with_room);
        }
        let joinable = || {
            page.items
                .iter()
                .filter(|row| row.joinable && row.playing < row.max_players)
        };
        if strategy == JoinStrategy::Fullest {
            assert_eq!(
                row.playing,
                joinable().map(|row| row.playing).max().unwrap()
            );
        }
        if strategy == JoinStrategy::Emptiest {
            assert_eq!(
                row.playing,
                joinable().map(|row| row.playing).min().unwrap()
            );
        }
        if strategy == JoinStrategy::Hop {
            let again =
                pick_server(&page.items, strategy, 1, std::slice::from_ref(&pick.job_id)).unwrap();
            assert_ne!(
                again.job_id, pick.job_id,
                "a hop never returns the same server"
            );
        }
    }
}

#[tokio::test]
#[ignore = "talks to the real Roblox API"]
async fn real_user_lookup_and_avatars() {
    let social = Social::new(Arc::new(RobloxWeb::new().unwrap()));
    let cancel = CancellationToken::new();
    let user = social.user_by_name("Roblox", &cancel).await.unwrap();
    println!("{user:?}");
    assert_eq!(user.id, "1");
    assert!(social
        .user_by_name("this_user_cannot_exist_9f3k2", &cancel)
        .await
        .is_err());
    let avatars = social.avatars(&[1, 156], &cancel).await.unwrap();
    println!("{avatars:?}");
    assert!(avatars["1"].starts_with("https://") && avatars["1"].contains(".rbxcdn.com/"));
}

#[tokio::test]
#[ignore = "needs NOCTA_SHARE_DATA and talks to the real Roblox API"]
async fn real_presence_and_friends_of_saved_accounts() {
    let source = std::path::PathBuf::from(std::env::var("NOCTA_SHARE_DATA").unwrap());
    let copy = tempfile::tempdir().unwrap();
    for file in ["nocta.sqlite", "nocta.sqlite-wal", "nocta.sqlite-shm"] {
        if source.join(file).exists() {
            std::fs::copy(source.join(file), copy.path().join(file)).unwrap();
        }
    }
    let storage = Storage::open(&copy.path().join("nocta.sqlite"))
        .await
        .unwrap();
    let vault = Arc::new(Vault::open(&storage.vault_id().await.unwrap(), false).unwrap());
    let accounts = Accounts::new(storage, vault);
    let social = Social::new(Arc::new(RobloxWeb::new().unwrap()));
    let cancel = CancellationToken::new();
    let mut candidates: Vec<_> = accounts
        .list()
        .await
        .unwrap()
        .into_iter()
        .filter(|account| {
            account.deleted_at.is_none() && account.has_cookie && account.user_id.is_some()
        })
        .collect();
    // Healthy sessions first; banned ones are skipped as they answer.
    candidates.sort_by_key(|account| account.health != nocta_core::models::AccountHealth::Healthy);
    candidates.truncate(25);
    let ids: Vec<u64> = candidates
        .iter()
        .filter_map(|account| account.user_id.as_deref()?.parse().ok())
        .collect();
    // Without a session Roblox still says who is online.
    let anonymous = social.presence(None, &ids, &cancel).await.unwrap();
    println!("anonymous presence for {} accounts", anonymous.len());
    assert_eq!(anonymous.len(), ids.len());
    for account in &candidates {
        let secrets = accounts.secrets(account.id).await.unwrap();
        let cookie = &secrets.cookie;
        let presence = match social.presence(Some(cookie), &ids, &cancel).await {
            Ok(presence) => presence,
            Err(error) => {
                println!("account #{}: {error}", account.id);
                continue;
            }
        };
        println!(
            "presence of {} accounts via #{}:",
            presence.len(),
            account.id
        );
        for item in &presence {
            println!(
                "  user {} kind {} place {:?}",
                item.user_id, item.kind, item.place_id
            );
        }
        assert_eq!(presence.len(), ids.len());
        let user_id: u64 = account.user_id.as_deref().unwrap().parse().unwrap();
        let friends = match social.friends(cookie, user_id, &cancel).await {
            Ok(friends) => friends,
            Err(error) => {
                println!("account #{}: {error}", account.id);
                continue;
            }
        };
        println!("account #{} has {} friends", account.id, friends.len());
        for friend in friends.iter().take(5) {
            println!(
                "  {} (@{}) id {}",
                friend.display_name, friend.name, friend.id
            );
            assert!(!friend.name.is_empty(), "every friend has a name");
        }
        return;
    }
    println!("no saved account has a working, unbanned session");
}
