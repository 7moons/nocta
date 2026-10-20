//! Reads account databases of the earlier Nocta app, picked by the user, so their accounts and
//! folders can be imported. Files are opened read-only; nothing is ever written to them.
//!
//! The old apps kept usernames, passwords and cookies in plain SQLite columns. Schemas differ
//! between versions, so columns are detected instead of assumed; only `username` and `cookie`
//! are required.
use crate::{models::LegacySource, CoreError, CoreResult};
use sqlx::{
    sqlite::{SqliteConnectOptions, SqliteConnection},
    ConnectOptions, Row,
};
use std::path::Path;

#[derive(Debug, Clone)]
pub struct LegacyFolder {
    pub id: i64,
    pub name: String,
    pub color: Option<String>,
    pub group: Option<String>,
}

#[derive(Debug, Clone)]
pub struct LegacyAccount {
    pub username: String,
    pub password: String,
    pub cookie: String,
    pub folder_id: Option<i64>,
    pub deleted: bool,
    /// Details only some apps keep (ic3w0lf's manager); empty otherwise.
    pub alias: String,
    pub note: String,
    pub user_id: Option<String>,
}

#[derive(Debug, Default)]
pub struct LegacyData {
    pub folders: Vec<LegacyFolder>,
    pub accounts: Vec<LegacyAccount>,
}

/// Known database locations that exist, deduplicated (Windows paths are case-insensitive).
async fn open(path: &Path) -> CoreResult<SqliteConnection> {
    SqliteConnectOptions::new()
        .filename(path)
        .read_only(true)
        .connect()
        .await
        .map_err(|_| CoreError::Validation("This file is not a readable account database.".into()))
}

async fn columns(connection: &mut SqliteConnection, table: &str) -> CoreResult<Vec<String>> {
    // `table` only ever comes from the fixed names below, never from input.
    Ok(sqlx::query(&format!("PRAGMA table_info({table})"))
        .fetch_all(&mut *connection)
        .await?
        .iter()
        .filter_map(|row| row.try_get::<String, _>("name").ok())
        .map(|name| name.to_lowercase())
        .collect())
}

struct Layout {
    password: bool,
    folder: bool,
    is_deleted: bool,
    deleted_at: bool,
    folders: bool,
    folder_color: bool,
    folder_group: bool,
}

async fn layout(connection: &mut SqliteConnection) -> CoreResult<Layout> {
    let accounts = columns(connection, "accounts").await?;
    if !accounts.iter().any(|column| column == "username")
        || !accounts.iter().any(|column| column == "cookie")
    {
        return Err(CoreError::Validation(
            "This database has no accounts table with usernames and cookies.".into(),
        ));
    }
    let folders = columns(connection, "folders").await?;
    let has = |list: &[String], name: &str| list.iter().any(|column| column == name);
    Ok(Layout {
        password: has(&accounts, "password"),
        folder: has(&accounts, "folder_id") && has(&folders, "name"),
        is_deleted: has(&accounts, "is_deleted"),
        deleted_at: has(&accounts, "deleted_at"),
        folders: has(&folders, "id") && has(&folders, "name"),
        folder_color: has(&folders, "color"),
        folder_group: has(&folders, "group_name"),
    })
}

fn deleted_expression(layout: &Layout) -> &'static str {
    match (layout.is_deleted, layout.deleted_at) {
        (true, true) => "(COALESCE(is_deleted, 0) != 0 OR deleted_at IS NOT NULL)",
        (true, false) => "(COALESCE(is_deleted, 0) != 0)",
        (false, true) => "(deleted_at IS NOT NULL)",
        (false, false) => "0",
    }
}

/// Counts what a database holds without reading any secret. Only accounts with a username and a
/// cookie count, since those are the ones an import can bring over.
pub async fn inspect(path: &Path, app: &str) -> CoreResult<LegacySource> {
    let mut connection = open(path).await?;
    let layout = layout(&mut connection).await?;
    let accounts: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM accounts WHERE TRIM(COALESCE(username, '')) != '' AND TRIM(COALESCE(cookie, '')) != ''",
    )
    .fetch_one(&mut connection)
    .await?;
    let in_trash: i64 = sqlx::query_scalar(&format!(
        "SELECT COUNT(*) FROM accounts WHERE TRIM(COALESCE(username, '')) != '' AND TRIM(COALESCE(cookie, '')) != '' AND {}",
        deleted_expression(&layout)
    ))
    .fetch_one(&mut connection)
    .await?;
    let folders: i64 = if layout.folders {
        sqlx::query_scalar("SELECT COUNT(*) FROM folders")
            .fetch_one(&mut connection)
            .await?
    } else {
        0
    };
    let modified_at = std::fs::metadata(path)
        .and_then(|meta| meta.modified())
        .ok()
        .map(|time| chrono::DateTime::<chrono::Utc>::from(time).to_rfc3339());
    Ok(LegacySource {
        path: path.to_string_lossy().into_owned(),
        app: app.to_owned(),
        accounts: accounts.max(0) as u32,
        in_trash: in_trash.max(0) as u32,
        folders: folders.max(0) as u32,
        modified_at,
        needs_password: false,
    })
}

/// Reads folders and accounts, including passwords and cookies, for an import.
pub async fn read(path: &Path) -> CoreResult<LegacyData> {
    let mut connection = open(path).await?;
    let layout = layout(&mut connection).await?;
    let mut data = LegacyData::default();
    if layout.folders {
        let sql = format!(
            "SELECT id, name, {}, {} FROM folders ORDER BY {}id",
            if layout.folder_color { "color" } else { "NULL" },
            if layout.folder_group {
                "group_name"
            } else {
                "NULL"
            },
            if columns(&mut connection, "folders")
                .await?
                .iter()
                .any(|column| column == "sort_order")
            {
                "sort_order, "
            } else {
                ""
            }
        );
        for row in sqlx::query(&sql).fetch_all(&mut connection).await? {
            data.folders.push(LegacyFolder {
                id: row.try_get(0)?,
                name: row.try_get::<Option<String>, _>(1)?.unwrap_or_default(),
                color: row.try_get(2)?,
                group: row.try_get(3)?,
            });
        }
    }
    let sql = format!(
        "SELECT username, {}, cookie, {}, {} FROM accounts ORDER BY id",
        if layout.password { "password" } else { "NULL" },
        if layout.folder { "folder_id" } else { "NULL" },
        deleted_expression(&layout)
    );
    for row in sqlx::query(&sql).fetch_all(&mut connection).await? {
        // Columns are loosely typed in the old apps; read as text where possible.
        let text = |index: usize| -> String {
            row.try_get::<Option<String>, _>(index)
                .ok()
                .flatten()
                .unwrap_or_default()
        };
        data.accounts.push(LegacyAccount {
            username: text(0).trim().to_owned(),
            password: text(1),
            cookie: text(2).trim().to_owned(),
            folder_id: row.try_get::<Option<i64>, _>(3).ok().flatten(),
            deleted: row.try_get::<Option<i64>, _>(4).ok().flatten().unwrap_or(0) != 0,
            alias: String::new(),
            note: String::new(),
            user_id: None,
        });
    }
    Ok(data)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use sqlx::Connection;

    /// Builds a database shaped like the old Nocta's.
    pub(crate) async fn old_nocta(path: &Path) {
        let mut connection = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .connect()
            .await
            .unwrap();
        for statement in [
            "CREATE TABLE folders (id INTEGER PRIMARY KEY, name TEXT NOT NULL, color TEXT, group_name TEXT, sort_order INTEGER)",
            "CREATE TABLE accounts (id INTEGER PRIMARY KEY, username TEXT, password TEXT, cookie TEXT, status TEXT, is_deleted INTEGER DEFAULT 0, deleted_at TEXT, folder_id INTEGER, userId INTEGER)",
            "INSERT INTO folders VALUES (1, 'Mains', '#ff8800', 'Farm', 2), (2, 'Alts', '#22aa55', NULL, 1)",
            "INSERT INTO accounts (username, password, cookie, folder_id) VALUES ('Alpha', 'pw1', '_|WARNING:a', 1), ('Beta', NULL, '_|WARNING:b', 2), ('NoCookie', 'pw', '', NULL), ('', 'x', 'y', NULL)",
            "INSERT INTO accounts (username, password, cookie, is_deleted, deleted_at) VALUES ('Gone', 'pw', '_|WARNING:g', 1, '2026-01-01')",
        ] {
            sqlx::query(statement).execute(&mut connection).await.unwrap();
        }
        connection.close().await.unwrap();
    }

    #[tokio::test]
    async fn reads_old_databases_by_their_columns() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("old-nocta.sqlite");
        old_nocta(&path).await;
        // A file that is not a database is refused, not read.
        let other = root.path().join("other.sqlite");
        std::fs::write(&other, b"not sqlite").unwrap();
        assert!(inspect(&other, "Selected database").await.is_err());

        let found = inspect(&path, "Selected database").await.unwrap();
        // Alpha, Beta and Gone have cookies; NoCookie and the nameless row cannot be imported.
        assert_eq!((found.accounts, found.in_trash, found.folders), (3, 1, 2));

        let data = read(&path).await.unwrap();
        assert_eq!(
            data.folders
                .iter()
                .map(|f| f.name.as_str())
                .collect::<Vec<_>>(),
            ["Alts", "Mains"]
        );
        let alpha = &data.accounts[0];
        assert_eq!(
            (
                alpha.username.as_str(),
                alpha.password.as_str(),
                alpha.folder_id
            ),
            ("Alpha", "pw1", Some(1))
        );
        assert_eq!(
            data.accounts[1].password, "",
            "a NULL password becomes empty"
        );
        assert!(
            data.accounts
                .iter()
                .find(|a| a.username == "Gone")
                .unwrap()
                .deleted
        );
    }
}
