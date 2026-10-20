use super::*;

#[tokio::test]
async fn folder_names_are_global_and_moves_preserve_group_order() {
    let accounts = setup().await;
    for (name, group) in [
        ("Alpha", Some("Work")),
        ("Beta", Some("Work")),
        ("Gamma", None),
    ] {
        accounts
            .save_folder(
                None,
                FolderInput {
                    name: name.into(),
                    color: "#AABBCC".into(),
                    group: group.map(str::to_owned),
                },
            )
            .await
            .unwrap();
    }
    assert!(accounts
        .save_folder(
            None,
            FolderInput {
                name: " alpha ".into(),
                color: "#abcdef".into(),
                group: Some("Other".into())
            }
        )
        .await
        .is_err());
    assert_eq!(accounts.folders().await.unwrap()[0].color, "#aabbcc");
    accounts
        .move_folder(2, Some(" Work ".into()), Some(1))
        .await
        .unwrap();
    let ordered = accounts.folders().await.unwrap();
    assert_eq!(
        ordered
            .iter()
            .filter(|f| f.group.as_deref() == Some("Work"))
            .map(|f| f.id)
            .collect::<Vec<_>>(),
        vec![2, 1]
    );
    accounts
        .move_folder(3, Some("Work".into()), Some(1))
        .await
        .unwrap();
    let ordered = accounts.folders().await.unwrap();
    assert_eq!(
        ordered
            .iter()
            .filter(|f| f.group.as_deref() == Some("Work"))
            .map(|f| f.id)
            .collect::<Vec<_>>(),
        vec![2, 3, 1]
    );
    accounts.move_folder(2, None, None).await.unwrap();
    assert_eq!(
        accounts
            .folders()
            .await
            .unwrap()
            .iter()
            .find(|f| f.id == 2)
            .unwrap()
            .group,
        None
    );
    assert!(accounts
        .move_folder(1, Some("Other".into()), Some(3))
        .await
        .is_err());
    assert_eq!(
        accounts
            .folders()
            .await
            .unwrap()
            .iter()
            .find(|f| f.id == 1)
            .unwrap()
            .group
            .as_deref(),
        Some("Work")
    );
}
async fn setup() -> Accounts {
    Accounts::new(
        Storage::memory().await.unwrap(),
        Arc::new(Vault::test_key()),
    )
}

/// Imports a copy of a real old database into a throwaway store. The original is only read.
/// Run with: NOCTA_LEGACY_DB=path cargo test -p nocta-core real_legacy -- --ignored --nocapture
#[tokio::test]
#[ignore = "needs NOCTA_LEGACY_DB pointing to an old Nocta database"]
async fn real_legacy_database_imports_into_a_throwaway_store() {
    let source = std::path::PathBuf::from(std::env::var_os("NOCTA_LEGACY_DB").unwrap());
    let before = std::fs::read(&source).unwrap();
    let copy = tempfile::tempdir().unwrap();
    let path = copy.path().join("old.sqlite");
    std::fs::write(&path, &before).unwrap();

    let found = crate::legacy::inspect(&path, "test").await.unwrap();
    println!(
        "found {} accounts, {} in trash, {} folders",
        found.accounts, found.in_trash, found.folders
    );
    let accounts = setup().await;
    let started = std::time::Instant::now();
    let data = crate::legacy::read(&path).await.unwrap();
    let report = accounts.import_legacy(data, true, |_| {}).await.unwrap();
    println!(
        "added {} skipped {} in {:.1}s",
        report.added,
        report.skipped,
        started.elapsed().as_secs_f32()
    );
    let mut reasons = std::collections::BTreeMap::new();
    for issue in &report.issues {
        let reason = issue.reason.split(": ").nth(1).unwrap_or(&issue.reason);
        *reasons.entry(reason.to_owned()).or_insert(0) += 1;
    }
    println!("skip reasons: {reasons:?}");
    assert_eq!(
        report.added, found.accounts,
        "every importable account was added"
    );
    let imported = accounts.list().await.unwrap();
    assert_eq!(imported.len() as u32, report.added);
    assert_eq!(
        accounts.folders().await.unwrap().len() as u32,
        found.folders
    );
    // Every imported account decrypts to a non-empty cookie.
    for account in imported.iter().take(50) {
        assert!(!accounts
            .secrets(account.id)
            .await
            .unwrap()
            .cookie
            .is_empty());
    }
    assert_eq!(
        std::fs::read(&source).unwrap(),
        before,
        "the original is untouched"
    );
}

#[tokio::test]
async fn legacy_import_maps_folders_trash_and_skips_without_writing_the_source() {
    let accounts = setup().await;
    accounts
        .save_folder(
            None,
            FolderInput {
                name: "mains".into(),
                color: "#123456".into(),
                group: None,
            },
        )
        .await
        .unwrap();
    accounts
        .import("Beta::existing-cookie".into(), None, false)
        .await
        .unwrap();
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("old-nocta.sqlite");
    crate::legacy::tests::old_nocta(&path).await;
    let before = std::fs::read(&path).unwrap();

    let data = crate::legacy::read(&path).await.unwrap();
    let report = accounts.import_legacy(data, false, |_| {}).await.unwrap();
    assert_eq!(report.added, 1, "{:?}", report.issues);
    let reasons = report
        .issues
        .iter()
        .map(|issue| issue.reason.as_str())
        .collect::<Vec<_>>();
    assert!(reasons.contains(&"Beta: Already in Nocta (or in Trash)."));
    assert!(reasons.contains(&"NoCookie: No cookie saved."));
    assert!(reasons.contains(&"Gone: In the old Trash (not included)."));

    let folders = accounts.folders().await.unwrap();
    let mains = folders
        .iter()
        .find(|folder| folder.name == "mains")
        .unwrap();
    assert_eq!(folders.len(), 2, "an existing folder is reused by name");
    assert!(folders
        .iter()
        .any(|folder| folder.name == "Alts" && folder.color == "#22aa55"));
    let list = accounts.list().await.unwrap();
    let alpha = list
        .iter()
        .find(|account| account.username == "Alpha")
        .unwrap();
    assert_eq!(alpha.folder_id, Some(mains.id));
    let secrets = accounts.secrets(alpha.id).await.unwrap();
    assert_eq!(
        (secrets.password.as_str(), secrets.cookie.as_str()),
        ("pw1", "_|WARNING:a")
    );

    // A second run with Trash included adds only the deleted account, into Trash.
    let data = crate::legacy::read(&path).await.unwrap();
    let again = accounts.import_legacy(data, true, |_| {}).await.unwrap();
    assert_eq!(again.added, 1);
    let gone = accounts
        .list()
        .await
        .unwrap()
        .into_iter()
        .find(|account| account.username == "Gone")
        .unwrap();
    assert!(gone.deleted_at.is_some());
    assert_eq!(
        std::fs::read(&path).unwrap(),
        before,
        "the old database is never modified"
    );
}

#[tokio::test]
async fn folder_edits_keep_validity_cookie_edits_clear_it_and_restore_has_explicit_destinations() {
    let accounts = setup().await;
    accounts
        .import("Alice:pw:cookie".into(), None, false)
        .await
        .unwrap();
    accounts.mark_cookie(1, true, Some("123")).await.unwrap();
    accounts
        .mark_health(1, AccountHealth::Healthy, None)
        .await
        .unwrap();
    accounts
        .save_folder(
            None,
            FolderInput {
                name: "Target".into(),
                color: "#AaBbCc".into(),
                group: None,
            },
        )
        .await
        .unwrap();
    let folder = accounts.folders().await.unwrap().remove(0);
    assert_eq!(folder.color, "#aabbcc");
    accounts
        .update(
            1,
            AccountUpdate {
                username: "Alice".into(),
                folder_id: Some(folder.id),
                password: Some("new-password".into()),
                cookie: None,
            },
        )
        .await
        .unwrap();
    let metadata = accounts.get(1).await.unwrap();
    assert_eq!(metadata.cookie_valid, Some(true));
    assert_eq!(metadata.health, AccountHealth::Healthy);
    assert!(metadata.checked_at.is_some());
    accounts.action(&[1], "trash", None).await.unwrap();
    accounts
        .action(&[1], "restore_unfiled", None)
        .await
        .unwrap();
    assert_eq!(accounts.get(1).await.unwrap().folder_id, None);
    accounts.action(&[1], "trash", None).await.unwrap();
    accounts
        .action(&[1], "restore", Some(folder.id))
        .await
        .unwrap();
    assert_eq!(accounts.get(1).await.unwrap().folder_id, Some(folder.id));
    accounts
        .update(
            1,
            AccountUpdate {
                username: "Alice".into(),
                folder_id: Some(folder.id),
                password: None,
                cookie: Some("changed-cookie".into()),
            },
        )
        .await
        .unwrap();
    let metadata = accounts.get(1).await.unwrap();
    assert_eq!(metadata.cookie_valid, None);
    assert_eq!(metadata.user_id, None);
    assert_eq!(metadata.checked_at, None);
    assert_eq!(metadata.health, AccountHealth::Unchecked);
}

#[tokio::test]
async fn progress_reports_committed_counts_and_bulk_update_keeps_folder_order_and_clipboard_secrets(
) {
    let accounts = setup().await;
    let progress = std::sync::Mutex::new(Vec::new());
    let input = (0..100)
        .map(|i| format!("Account_{i}:password:cookie:with:colons"))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n\nmalformed\nAccount_0:duplicate:cookie";
    let report = accounts
        .import_with_progress(input, None, false, |event| {
            progress.lock().unwrap().push(event)
        })
        .await
        .unwrap();
    assert_eq!((report.added, report.skipped), (100, 2));
    {
        let events = progress.lock().unwrap();
        assert!(events.len() >= 5);
        assert!(events
            .windows(2)
            .all(|events| events[0].processed <= events[1].processed));
        assert!(!events[0].committed);
        let committed = events.last().unwrap();
        assert!(committed.committed);
        assert_eq!(
            (
                committed.processed,
                committed.total,
                committed.added,
                committed.failed
            ),
            (103, 103, 100, 2)
        );
    }
    let report = accounts
        .import(
            "ACCOUNT_0:changed:new-cookie\nMissing:p:c\naccount_0:x:y\nbad".into(),
            None,
            true,
        )
        .await
        .unwrap();
    assert_eq!((report.updated, report.skipped), (1, 3));
    assert_eq!(
        accounts.copy(&[2, 1], "username").await.unwrap(),
        "Account_1\nAccount_0"
    );
    assert_eq!(
        accounts.copy(&[1], "user:pass:cookie").await.unwrap(),
        "Account_0:changed:new-cookie"
    );
    let random = accounts.copy(&[1], "user:random:cookie").await.unwrap();
    assert!(random.starts_with("Account_0:") && random.ends_with(":new-cookie"));
    assert_eq!(
        accounts
            .copy(&[1, 2], "#{id} {user} [{folder}] {{{password}}}")
            .await
            .unwrap(),
        "#1 Account_0 [General space] {changed}\n#2 Account_1 [General space] {password}"
    );
    assert!(accounts.copy(&[1], "{username}:{secret}").await.is_err());
    assert_eq!(accounts.secrets(1).await.unwrap().password, "changed");
}

#[tokio::test]
async fn native_settings_rejection_rolls_back_backup_and_folder_deletion_updates_saved_scope() {
    let source = setup().await;
    source
        .save_folder(
            None,
            FolderInput {
                name: "Scope".into(),
                color: "#aabbcc".into(),
                group: None,
            },
        )
        .await
        .unwrap();
    source
        .import("Alice:p:c".into(), Some(1), false)
        .await
        .unwrap();
    let bytes = source
        .export_backup("test-backup-password".into())
        .await
        .unwrap();
    let target = setup().await;
    let result = target
        .restore_backup_with_settings(bytes, "test-backup-password".into(), |_| {
            Err(CoreError::Validation("Native helper rejected.".into()))
        })
        .await;
    assert!(result.is_err());
    assert!(target.list().await.unwrap().is_empty());
    assert!(target.folders().await.unwrap().is_empty());
    source.remove_folder(1).await.unwrap();
    assert_eq!(source.get(1).await.unwrap().folder_id, None);
}

#[tokio::test]
async fn import_preserves_colons_and_deduplicates_trash_without_leaking_secrets() {
    let accounts = setup().await;
    let report = accounts
        .import(
            "Alice:pw:cookie:with:colons\nalice:other:secret\nbad-line".into(),
            None,
            false,
        )
        .await
        .unwrap();
    assert_eq!(report.added, 1);
    assert_eq!(report.skipped, 2);
    let rows = accounts.list().await.unwrap();
    assert_eq!(
        accounts.secrets(rows[0].id).await.unwrap().cookie,
        "cookie:with:colons"
    );
    let snapshot = serde_json::to_string(&rows).unwrap();
    assert!(!snapshot.contains("cookie:with"));
    assert!(!snapshot.contains("\"password\":"));
    accounts.action(&[rows[0].id], "trash", None).await.unwrap();
    assert_eq!(
        accounts
            .import("ALICE:x:y".into(), None, false)
            .await
            .unwrap()
            .skipped,
        1
    );
    let sealed: Vec<u8> = sqlx::query_scalar("SELECT secrets FROM accounts")
        .fetch_one(&accounts.storage.pool)
        .await
        .unwrap();
    assert!(!sealed.windows(6).any(|v| v == b"cookie"));
}

#[tokio::test]
async fn folder_deletion_preserves_accounts_and_purge_only_deletes_trash() {
    let accounts = setup().await;
    accounts
        .save_folder(
            None,
            FolderInput {
                name: "Main".into(),
                color: "#a3a3a3".into(),
                group: None,
            },
        )
        .await
        .unwrap();
    let folder = accounts.folders().await.unwrap().remove(0);
    accounts
        .import("Alice:p:c".into(), Some(folder.id), false)
        .await
        .unwrap();
    let id = accounts.list().await.unwrap()[0].id;
    accounts.action(&[id], "purge", None).await.unwrap();
    assert_eq!(accounts.list().await.unwrap().len(), 1);
    accounts.remove_folder(folder.id).await.unwrap();
    assert_eq!(accounts.list().await.unwrap()[0].folder_id, None);
    accounts.action(&[id], "trash", None).await.unwrap();
    accounts.action(&[id], "restore", None).await.unwrap();
    assert_eq!(accounts.list().await.unwrap()[0].deleted_at, None);
}

#[tokio::test]
async fn backup_restores_into_new_vault_and_keeps_existing_workspace_untouched() {
    let source = setup().await;
    source
        .save_folder(
            None,
            FolderInput {
                name: "Main".into(),
                color: "#a3a3a3".into(),
                group: Some("Work".into()),
            },
        )
        .await
        .unwrap();
    let folder = source.folders().await.unwrap()[0].id;
    source
        .import(
            "Alice:synthetic-password:synthetic-cookie".into(),
            Some(folder),
            false,
        )
        .await
        .unwrap();
    source.mark_cookie(1, true, Some("123")).await.unwrap();
    sqlx::query("UPDATE accounts SET presence = ? WHERE id = 1")
        .bind(r#"{"userPresenceType":2,"lastLocation":"A game","placeId":123,"lastOnline":"2026-10-04T12:00:00Z"}"#)
        .execute(&source.storage.pool).await.unwrap();
    let tasks = crate::tasks::Tasks::default();
    let task = tasks.reserve(&[1], TaskKind::LaunchGame).unwrap()[0].0;
    tasks.set(task, TaskStatus::Failed, "No launch handler.");
    source.record_task(&tasks.get(task).unwrap()).await.unwrap();
    source.mark_started(1).await.unwrap();
    source.action(&[1], "trash", None).await.unwrap();
    let config = source
        .storage
        .patched_config(serde_json::json!({"checkerConcurrency": 8}))
        .await
        .unwrap();
    source.storage.save_config(&config).await.unwrap();
    let bytes = source
        .export_backup("test-backup-password".into())
        .await
        .unwrap();
    assert!(!bytes.windows(6).any(|v| v == b"cookie"));
    let target = Accounts::new(
        Storage::memory().await.unwrap(),
        Arc::new(Vault::test_key_with_byte(9)),
    );
    assert!(target
        .restore_backup(bytes.clone(), "incorrect-password".into())
        .await
        .is_err());
    assert!(target.list().await.unwrap().is_empty());
    assert_eq!(
        target
            .restore_backup(bytes.clone(), "test-backup-password".into())
            .await
            .unwrap(),
        1
    );
    assert_eq!(target.secrets(1).await.unwrap().cookie, "synthetic-cookie");
    assert!(target.list().await.unwrap()[0].deleted_at.is_some());
    assert_eq!(target.list().await.unwrap()[0].cookie_valid, Some(true));
    assert!(target.list().await.unwrap()[0].last_started_at.is_some());
    let restored = target.list().await.unwrap().remove(0);
    assert_eq!(restored.presence.unwrap().place_id.as_deref(), Some("123"));
    assert_eq!(restored.last_task.unwrap().detail, "No launch handler.");
    assert_eq!(
        target.folders().await.unwrap()[0].group.as_deref(),
        Some("Work")
    );
    assert_eq!(
        target.storage.config().await.unwrap().checker_concurrency,
        8
    );
    assert!(target
        .restore_backup(bytes, "test-backup-password".into())
        .await
        .is_err());
    assert_eq!(target.list().await.unwrap().len(), 1);
}

#[tokio::test]
async fn disk_restart_preserves_secrets_and_settings_patches_do_not_clobber() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("nocta.sqlite");
    let storage = Storage::open(&path).await.unwrap();
    let vault_id = storage.vault_id().await.unwrap();
    let accounts = Accounts::new(storage.clone(), Arc::new(Vault::test_key()));
    accounts
        .import("Alice:p:c".into(), None, false)
        .await
        .unwrap();
    let config = storage
        .patched_config(serde_json::json!({"checkerConcurrency": 8}))
        .await
        .unwrap();
    storage.save_config(&config).await.unwrap();
    let tasks = crate::tasks::Tasks::default();
    let id = tasks.reserve(&[1], TaskKind::CheckHealth).unwrap()[0].0;
    tasks.set(id, TaskStatus::Failed, "Ambiguous response.");
    accounts.record_task(&tasks.get(id).unwrap()).await.unwrap();
    let config = storage
        .patched_config(serde_json::json!({"checkerDelaySecs": 3}))
        .await
        .unwrap();
    storage.save_config(&config).await.unwrap();
    assert!(storage
        .patched_config(serde_json::json!({"unknownField": true}))
        .await
        .is_err());
    assert!(storage
        .patched_config(serde_json::json!({"checkerConcurrency": 0}))
        .await
        .is_err());
    storage.close().await;
    let reopened = Storage::open(&path).await.unwrap();
    assert_eq!(reopened.vault_id().await.unwrap(), vault_id);
    assert!(reopened.has_secrets().await.unwrap());
    assert_eq!(reopened.config().await.unwrap().checker_concurrency, 8);
    assert_eq!(reopened.config().await.unwrap().checker_delay_secs, 3);
    let last = Accounts::new(reopened.clone(), Arc::new(Vault::test_key()))
        .get(1)
        .await
        .unwrap()
        .last_task
        .unwrap();
    assert_eq!(last.status, TaskStatus::Failed);
    assert_eq!(last.detail, "Ambiguous response.");
    assert_eq!(
        Accounts::new(reopened.clone(), Arc::new(Vault::test_key()))
            .secrets(1)
            .await
            .unwrap()
            .cookie,
        "c"
    );
    reopened.close().await;
}

#[tokio::test]
async fn details_and_authenticator_keys_are_saved_and_survive_a_backup() {
    let accounts = setup().await;
    accounts
        .import("Alpha:pw:_|WARNING:cookie-a".into(), None, false)
        .await
        .unwrap();
    accounts
        .update_details(
            1,
            AccountDetails {
                alias: " Main farm ".into(),
                note: "Uses the blue base".into(),
                color: Some("#ff8800".into()),
                pinned: true,
                is_main: false,
                fields: Vec::new(),
                afk_point: None,
            },
        )
        .await
        .unwrap();
    accounts
        .set_totp(1, Some("gezd gnbv gy3t qojq gezd gnbv gy3t qojq".into()))
        .await
        .unwrap();
    let account = accounts.get(1).await.unwrap();
    assert_eq!(account.alias, "Main farm");
    assert_eq!(account.color.as_deref(), Some("#ff8800"));
    assert!(account.pinned && account.has_totp);
    let code = accounts.totp_code(1).await.unwrap();
    assert_eq!(code.code.len(), 6);
    assert!((1..=30).contains(&code.remaining_secs));
    // The key is a secret: it never appears in the account list.
    let listed = serde_json::to_string(&accounts.list().await.unwrap()).unwrap();
    assert!(!listed.to_lowercase().contains("gezd"));
    // Other secrets are untouched by saving the key.
    assert_eq!(
        accounts.secrets(1).await.unwrap().cookie,
        "_|WARNING:cookie-a"
    );

    assert!(accounts
        .set_totp(1, Some("not a key".into()))
        .await
        .is_err());
    assert!(accounts
        .update_details(
            1,
            AccountDetails {
                alias: String::new(),
                note: String::new(),
                color: Some("orange".into()),
                pinned: false,
                is_main: false,
                fields: Vec::new(),
                afk_point: None,
            },
        )
        .await
        .is_err());

    // A backup carries the details and the key to another store.
    let bytes = accounts
        .export_backup("test-backup-password".into())
        .await
        .unwrap();
    let restored = setup().await;
    restored
        .restore_backup(bytes, "test-backup-password".into())
        .await
        .unwrap();
    let copy = restored.get(1).await.unwrap();
    assert_eq!(
        (copy.alias.as_str(), copy.pinned, copy.has_totp),
        ("Main farm", true, true)
    );
    assert_eq!(
        restored.totp_code(1).await.unwrap().code,
        accounts.totp_code(1).await.unwrap().code
    );

    accounts.set_totp(1, None).await.unwrap();
    assert!(!accounts.get(1).await.unwrap().has_totp);
    assert!(accounts.totp_code(1).await.is_err());
}

#[tokio::test]
async fn exports_selected_accounts_as_csv_and_json() {
    let accounts = setup().await;
    accounts
        .import(
            "Alpha:p,w:_|WARNING:cookie-a\nBeta::_|WARNING:cookie-b".into(),
            None,
            false,
        )
        .await
        .unwrap();
    let csv = accounts.export(&[2, 1], ExportFormat::Csv).await.unwrap();
    let lines: Vec<&str> = csv.lines().collect();
    assert_eq!(
        lines[0],
        "username,password,cookie,userId,alias,note,folder"
    );
    assert!(
        lines[1].starts_with("Beta,,_|WARNING:cookie-b"),
        "selection order is kept"
    );
    assert!(lines[2].starts_with("Alpha,\"p,w\",_|WARNING:cookie-a"));
    let json = accounts.export(&[1], ExportFormat::Json).await.unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed[0]["password"], "p,w");
    assert_eq!(parsed[0]["cookie"], "_|WARNING:cookie-a");
    assert!(accounts.export(&[], ExportFormat::Csv).await.is_err());
    assert!(accounts.export(&[99], ExportFormat::Csv).await.is_err());
}

#[tokio::test]
async fn roblox_account_manager_files_import_with_aliases_notes_ids_and_groups() {
    let accounts = setup().await;
    let json = br#"[
        {"SecurityToken":"_|WARNING:-DO-NOT-SHARE-THIS.--|_synthetic-a","Username":"Ram_Alice","UserID":156,"Group":"Farm","Alias":"Main","Description":"daily rewards","Password":"pw"},
        {"SecurityToken":"","Username":"Ram_NoCookie","Group":"Default"}
    ]"#;
    let data = crate::ram_import::parse(json).unwrap();
    let report = accounts.import_legacy(data, false, |_| {}).await.unwrap();
    assert_eq!(report.added, 1, "{:?}", report.issues);
    assert_eq!(report.skipped, 1);
    let alice = accounts
        .list()
        .await
        .unwrap()
        .into_iter()
        .find(|account| account.username == "Ram_Alice")
        .unwrap();
    assert_eq!(alice.alias, "Main");
    assert_eq!(alice.note, "daily rewards");
    assert_eq!(alice.user_id.as_deref(), Some("156"));
    assert!(alice.has_password && alice.has_cookie);
    let farm = accounts.folders().await.unwrap();
    assert_eq!(farm.len(), 1);
    assert_eq!(alice.folder_id, Some(farm[0].id));
    assert_eq!(farm[0].name, "Farm");
}

#[tokio::test]
async fn own_order_fields_and_click_points_are_saved_checked_and_backed_up() {
    let accounts = setup().await;
    for name in ["Alpha", "Beta", "Gamma"] {
        accounts
            .import(format!("{name}:pw:_|WARNING:cookie-{name}"), None, false)
            .await
            .unwrap();
    }
    let details = |fields: Vec<(&str, &str)>, afk_point: Option<ClickPoint>| AccountDetails {
        alias: String::new(),
        note: String::new(),
        color: None,
        pinned: false,
        is_main: false,
        fields: fields
            .into_iter()
            .map(|(name, value)| AccountField {
                name: name.into(),
                value: value.into(),
            })
            .collect(),
        afk_point,
    };
    let point = ClickPoint {
        x_percent: 30,
        y_percent: 80,
    };
    accounts
        .update_details(
            2,
            details(
                vec![(" Server ", " EU "), ("", ""), ("Group", "Farm")],
                Some(point),
            ),
        )
        .await
        .unwrap();
    let beta = accounts.get(2).await.unwrap();
    assert_eq!(
        beta.fields,
        [
            AccountField {
                name: "Server".into(),
                value: "EU".into()
            },
            AccountField {
                name: "Group".into(),
                value: "Farm".into()
            }
        ],
        "trimmed, empty rows dropped"
    );
    assert_eq!(beta.afk_point, Some(point));
    for (fields, point) in [
        (vec![("", "value")], None),
        (vec![("Server", "EU"), ("server", "US")], None),
        (vec![("Line", "one\ntwo")], None),
        (
            vec![],
            Some(ClickPoint {
                x_percent: 101,
                y_percent: 0,
            }),
        ),
    ] {
        assert!(
            accounts
                .update_details(2, details(fields, point))
                .await
                .is_err(),
            "refused"
        );
    }
    let too_many: Vec<(String, String)> = (0..11).map(|i| (format!("F{i}"), "v".into())).collect();
    let too_many: Vec<(&str, &str)> = too_many
        .iter()
        .map(|(name, value)| (name.as_str(), value.as_str()))
        .collect();
    assert!(accounts
        .update_details(2, details(too_many, None))
        .await
        .is_err());

    // The user's own order: Gamma, Alpha; Beta has no position.
    accounts.reorder(&[3, 1]).await.unwrap();
    let order: Vec<(u32, Option<u32>)> = accounts
        .list()
        .await
        .unwrap()
        .into_iter()
        .map(|account| (account.id, account.sort_order))
        .collect();
    assert_eq!(order, [(1, Some(1)), (2, None), (3, Some(0))]);

    let bytes = accounts
        .export_backup("test-backup-password".into())
        .await
        .unwrap();
    let restored = setup().await;
    restored
        .restore_backup(bytes, "test-backup-password".into())
        .await
        .unwrap();
    let copy = restored.get(2).await.unwrap();
    assert_eq!(copy.fields.len(), 2);
    assert_eq!(copy.afk_point, Some(point));
    assert_eq!(restored.get(3).await.unwrap().sort_order, Some(0));
}

#[tokio::test]
async fn settings_from_an_older_version_with_removed_fields_still_load() {
    let accounts = setup().await;
    // A config with settings this version no longer has.
    let mut old = serde_json::to_value(AppConfig::default()).unwrap();
    let fields = old.as_object_mut().unwrap();
    for (key, value) in [
        ("removedNumber", serde_json::json!(4)),
        ("removedChoice", serde_json::json!("some")),
        ("removedList", serde_json::json!([1, 2])),
    ] {
        fields.insert(key.into(), value);
    }
    fields.insert("checkerConcurrency".into(), serde_json::json!(7));
    sqlx::query("INSERT INTO app_meta (key, value) VALUES ('config', ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value")
        .bind(old.to_string())
        .execute(&accounts.storage.pool)
        .await
        .unwrap();
    let config = accounts.config().await.unwrap();
    assert_eq!(
        config.checker_concurrency, 7,
        "the rest of the settings stay"
    );
    // Saving writes only the settings this version has.
    accounts.save_config(&config).await.unwrap();
    let saved: String = sqlx::query_scalar("SELECT value FROM app_meta WHERE key = 'config'")
        .fetch_one(&accounts.storage.pool)
        .await
        .unwrap();
    assert!(!saved.contains("removedNumber"));
}

#[tokio::test]
async fn groups_are_renamed_merged_and_dissolved_in_one_step() {
    let accounts = setup().await;
    for (name, group) in [
        ("Mains", Some("Farm")),
        ("Alts", Some("Farm")),
        ("Spare", Some("Other")),
        ("Loose", None),
    ] {
        accounts
            .save_folder(
                None,
                FolderInput {
                    name: name.into(),
                    color: "#aabbcc".into(),
                    group: group.map(str::to_owned),
                },
            )
            .await
            .unwrap();
    }
    let in_group = |folders: &[Folder], group: Option<&str>| {
        folders
            .iter()
            .filter(|folder| folder.group.as_deref() == group)
            .map(|folder| folder.name.clone())
            .collect::<Vec<_>>()
    };
    accounts
        .rename_folder_group("Farm", Some(" Farm EU ".into()))
        .await
        .unwrap();
    let folders = accounts.folders().await.unwrap();
    assert_eq!(in_group(&folders, Some("Farm EU")), ["Mains", "Alts"]);
    assert!(in_group(&folders, Some("Farm")).is_empty());
    // Into an existing group: its own folders stay first.
    accounts
        .rename_folder_group("Other", Some("Farm EU".into()))
        .await
        .unwrap();
    let folders = accounts.folders().await.unwrap();
    assert_eq!(
        in_group(&folders, Some("Farm EU")),
        ["Mains", "Alts", "Spare"]
    );
    // Dissolving puts the folders after the ungrouped ones.
    accounts.rename_folder_group("Farm EU", None).await.unwrap();
    let folders = accounts.folders().await.unwrap();
    assert_eq!(
        in_group(&folders, None),
        ["Loose", "Mains", "Alts", "Spare"]
    );
    assert!(matches!(
        accounts.rename_folder_group("Missing", None).await,
        Err(CoreError::NotFound)
    ));
    assert!(accounts
        .rename_folder_group("x", Some("a".repeat(81)))
        .await
        .is_err());
}
