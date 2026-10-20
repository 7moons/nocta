//! Real share-link expansion with an account from a copy of a Nocta data folder.
//!
//! `NOCTA_SHARE_DATA=%APPDATA%\app.nocta.desktop NOCTA_SHARE_LINK=<share link> cargo test -p nocta-core --test share_link -- --ignored --nocapture`
//!
//! The database is copied first; the copy gets the remembered link, the original is untouched.
use nocta_core::{
    accounts::Accounts,
    roblox::web::{RobloxWeb, ShareLinkType},
    storage::Storage,
    vault::Vault,
};
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

#[tokio::test]
#[ignore]
async fn real_share_link_expands_with_any_account() {
    let source = std::path::PathBuf::from(std::env::var("NOCTA_SHARE_DATA").unwrap());
    let link = std::env::var("NOCTA_SHARE_LINK").unwrap();
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
    let web = RobloxWeb::new().unwrap();
    let url = web.validate_invite_url(&link).unwrap();
    let (kind, code) = ShareLinkType::from_url(&url).expect("a share link");
    let cancel = CancellationToken::new();
    let candidates: Vec<_> = accounts
        .list()
        .await
        .unwrap()
        .into_iter()
        .filter(|account| account.deleted_at.is_none() && account.has_cookie)
        .take(5)
        .collect();
    for account in candidates {
        let secrets = accounts.secrets(account.id).await.unwrap();
        match web
            .resolve_share_link(&secrets.cookie, &code, kind, &cancel)
            .await
        {
            Ok(Some(direct)) => {
                // Print only the shape: the link code is a secret.
                let shown = direct.split("privateServerLinkCode=").next().unwrap();
                println!("account #{} -> {shown}privateServerLinkCode=…", account.id);
                assert!(direct.starts_with("https://www.roblox.com/games/"));
                return;
            }
            Ok(None) => panic!("Roblox did not recognise the link"),
            Err(error) => println!("account #{}: {error}", account.id),
        }
    }
    panic!("no account could open the link");
}
