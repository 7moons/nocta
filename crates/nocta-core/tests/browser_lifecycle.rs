use nocta_core::{
    browser::{BrowserManager, BrowserPurpose},
    CoreError,
};
use serde_json::json;
use std::{path::PathBuf, time::Duration};
use tokio_util::sync::CancellationToken;

#[tokio::test]
#[ignore = "requires NOCTA_TEST_BROWSER pointing to the managed Chromium executable"]
async fn owned_trees_profiles_and_cancelled_cdp_are_cleaned_up() {
    let executable =
        PathBuf::from(std::env::var_os("NOCTA_TEST_BROWSER").expect("Set NOCTA_TEST_BROWSER"));
    let directory = tempfile::tempdir().unwrap();
    let manager = BrowserManager::new(Some(executable), directory.path().join("profiles"));
    manager.sweep().await.unwrap();
    let cancel = CancellationToken::new();
    let first = manager
        .start(1, BrowserPurpose::Task, false, &cancel)
        .await
        .unwrap();
    first
        .client
        .call(
            "Network.setCookie",
            json!({"name": "synthetic", "value": "one", "url": "http://127.0.0.1/"}),
            Some(&first.page),
            &cancel,
        )
        .await
        .unwrap();
    let second = manager
        .start(2, BrowserPurpose::Manual, false, &cancel)
        .await
        .unwrap();
    let cookies = second
        .client
        .call(
            "Network.getCookies",
            json!({"urls": ["http://127.0.0.1/"]}),
            Some(&second.page),
            &cancel,
        )
        .await
        .unwrap();
    assert!(cookies["cookies"].as_array().unwrap().is_empty());
    assert!(matches!(
        manager.start(1, BrowserPurpose::Task, false, &cancel).await,
        Err(CoreError::Busy)
    ));
    let pending_cancel = CancellationToken::new();
    let operation = first.client.call(
        "Runtime.evaluate",
        json!({"expression": "new Promise(() => {})", "awaitPromise": true}),
        Some(&first.page),
        &pending_cancel,
    );
    tokio::pin!(operation);
    assert!(
        tokio::time::timeout(Duration::from_millis(50), &mut operation)
            .await
            .is_err()
    );
    pending_cancel.cancel();
    assert!(matches!(operation.await, Err(CoreError::Cancelled)));
    manager.close(1).await.unwrap();
    assert!(!first.running().await.unwrap());
    assert!(second.running().await.unwrap());
    manager.close_all().await.unwrap();
    assert!(!second.running().await.unwrap());
    assert_eq!(
        std::fs::read_dir(directory.path().join("profiles"))
            .unwrap()
            .count(),
        0
    );
    let already_cancelled = CancellationToken::new();
    already_cancelled.cancel();
    assert!(matches!(
        manager
            .start(3, BrowserPurpose::Task, false, &already_cancelled)
            .await,
        Err(CoreError::Cancelled)
    ));
}

/// An account's browser window keeps the account's name while its pages change their titles.
#[tokio::test]
#[ignore = "requires NOCTA_TEST_BROWSER pointing to the managed Chromium executable; opens a window"]
async fn the_account_browser_window_keeps_the_account_name() {
    use nocta_core::browser::BrowserOptions;
    use windows_sys::Win32::UI::WindowsAndMessaging::GetWindowTextW;
    let executable =
        PathBuf::from(std::env::var_os("NOCTA_TEST_BROWSER").expect("Set NOCTA_TEST_BROWSER"));
    let directory = tempfile::tempdir().unwrap();
    let manager = BrowserManager::new(Some(executable), directory.path().join("profiles"));
    let cancel = CancellationToken::new();
    for app_mode in [true, false] {
        let browser = manager
            .start_with_options(
                7,
                BrowserPurpose::Manual,
                BrowserOptions {
                    visible: true,
                    app_mode,
                    offline: true,
                    ..Default::default()
                },
                &cancel,
            )
            .await
            .unwrap();
        let pid = browser.process_id().await;
        let kept = nocta_core::launch::native::windows_title::keep(pid, "Farm (Alice_1)");
        let titles = || {
            nocta_core::launch::native::windows_window::process_windows(pid)
                .into_iter()
                .map(|window| {
                    let mut buffer = [0u16; 256];
                    let length = unsafe { GetWindowTextW(window, buffer.as_mut_ptr(), 256) };
                    String::from_utf16_lossy(&buffer[..length as usize])
                })
                .collect::<Vec<_>>()
        };
        for page in ["Roblox Home", "Charts", "Avatar Editor"] {
            browser
                .navigate(
                    &format!("data:text/html,<title>{page}</title><p>{page}"),
                    &cancel,
                )
                .await
                .unwrap();
            tokio::time::sleep(Duration::from_millis(700)).await;
            let now = titles();
            println!("app mode {app_mode}, page {page}: {now:?}");
            assert!(!now.is_empty(), "the browser has a window");
            assert!(
                now.iter().all(|title| title == "Farm (Alice_1)"),
                "every window shows the account"
            );
        }
        drop(kept);
        manager.close(7).await.unwrap();
    }
}
