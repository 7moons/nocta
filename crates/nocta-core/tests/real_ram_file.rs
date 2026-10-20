//! Imports an `AccountData.json` written the way ic3w0lf's Roblox Account Manager writes it.
//!
//! `NOCTA_RAM_FILE=<AccountData.json> [NOCTA_RAM_PASSWORD=<password>] cargo test -p nocta-core --test real_ram_file -- --ignored --nocapture`
use nocta_core::ram_import;

#[tokio::test]
#[ignore = "needs NOCTA_RAM_FILE"]
async fn real_account_manager_file_is_read() {
    let path = std::path::PathBuf::from(std::env::var("NOCTA_RAM_FILE").unwrap());
    let password = std::env::var("NOCTA_RAM_PASSWORD").ok();
    let source = ram_import::inspect(&path).await.unwrap();
    println!(
        "{}: {} accounts, {} groups, password: {}",
        source.app, source.accounts, source.folders, source.needs_password
    );
    let data = ram_import::read(&path, password.as_deref()).await.unwrap();
    for account in data.accounts.iter().take(5) {
        // Never print secrets: only whether they are there.
        println!(
            "  {} alias={:?} cookie={} password={}",
            account.username,
            account.alias,
            !account.cookie.is_empty(),
            !account.password.is_empty()
        );
    }
    assert!(!data.accounts.is_empty());
}
