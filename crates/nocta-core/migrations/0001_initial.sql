CREATE TABLE app_meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);

CREATE TABLE folders (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    normalized_name TEXT NOT NULL UNIQUE,
    color TEXT NOT NULL,
    group_name TEXT,
    sort_order INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE accounts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    username TEXT NOT NULL,
    normalized_username TEXT NOT NULL UNIQUE,
    folder_id INTEGER REFERENCES folders(id) ON DELETE SET NULL,
    health TEXT NOT NULL DEFAULT 'unchecked',
    checked_at TEXT,
    created_at TEXT NOT NULL,
    deleted_at TEXT,
    has_password INTEGER NOT NULL DEFAULT 0,
    has_cookie INTEGER NOT NULL DEFAULT 0,
    user_id TEXT,
    secrets BLOB NOT NULL
);
CREATE INDEX accounts_folder ON accounts(folder_id, deleted_at, id);
CREATE INDEX accounts_health ON accounts(health, deleted_at, id);

CREATE TABLE provider_secrets (provider TEXT PRIMARY KEY, secrets BLOB NOT NULL);

CREATE TABLE logs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp TEXT NOT NULL,
    level TEXT NOT NULL,
    module TEXT NOT NULL,
    message TEXT NOT NULL,
    account_id INTEGER
);
