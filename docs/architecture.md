# Architecture

Nocta has three layers:

- **`crates/nocta-core`**: all domain logic in Rust: storage, accounts, sessions, the launcher
  and Nocta's own Roblox install. It does not depend on Tauri, so it is tested
  on its own.
- **`src-tauri`**: the desktop shell. It opens the window, chooses the data folder, holds the app
  state and exposes an explicit list of IPC commands (`src/commands/`, registered in `lib.rs`).
- **`src`**: the React interface. It renders a snapshot of the backend state, keeps only interface
  state of its own (selection, open panels, theme, language) and changes data only through IPC.
  Opened in a normal browser it runs as a read-only preview.

## Glossary

- **Account**: a Roblox identity managed in Nocta. It is not a running game.
- **Folder**: a named group of accounts; folders can be grouped further for display.
- **Session state** (health): the result of the last check of an account's access.
- **Task**: one operation on one account (check health, refresh cookie, launch…).
  An account runs at most one task at a time.
- **Game instance**: a running Roblox client. Nocta tracks the ones it started ("owned") and lists
  the others ("external").

Session state, the running task and running instances are separate states and never overwrite
each other.

## Core modules

| Module | Responsibility |
| --- | --- |
| `storage`, `vault`, `backup` | SQLite (WAL, foreign keys, migrations), the encryption key in Windows Credential Manager, password-protected backups. |
| `accounts/` | Accounts and folders, bulk and legacy import, clipboard templates, logs; one file per concern. |
| `tasks` | Reserving an account for one operation, task history, cancellation. |
| `sessions/` | Authenticated identity checks, join prechecks, cookie refresh and login, manual browsers. |
| `browser/` | Managed Chromium with fresh per-task profiles, cleaned up (or quarantined) after use. |
| `launch/` | `Launcher`: launch targets and invite resolution (`targets`), the launch pipeline (`pipeline`), servers, game info and search. Next to it, each with its own type: `Automation` (`automation/`: presets, schedules, command line), `ClientCare` (`client_care/`: the upkeep loop, playtime, anti-AFK, memory, window naming and arranging, restarts), `Rejoin` (auto-rejoin) and `FrameRate` (the frame-rate cap). Also process tracking (`instances/`), platform adapters (`native/`), Nocta's own Roblox install (`client/`) and the module's tests (`tests/`). |
| `roblox/` | Clients of Roblox's web APIs: the HTTP client and the hosts it may send cookies to (`web`), Roblox link parsing (`invite`), changes on an account (`AccountTools`), status, Robux and friends (`Presence`) and user lookups (`Social`). |
| `metrics`, `legacy` | Lifetime metrics, reading databases of the old Nocta app. |
| `models/` | Types shared with the shell and, through ts-rs, the frontend. |

## Data and secrets

SQLite is accessed with SQLx: WAL mode, foreign keys and versioned migrations. One connection
writes; the snapshot sent to the window is read through separate read-only connections, which in WAL
mode see the last committed state without waiting for writes during bulk work. Passwords, cookies and two-step verification keys are sealed with AES-256-GCM using a fresh nonce
and the owning record as associated data. The installation key is random, stored in Windows
Credential Manager and identified by the database's UUID, so copying the data
folder to another computer does not carry the key; backups do.

A backup contains accounts (including Trash), folders and settings, encrypted with a
key derived from the backup password (Argon2id). Restoring requires an empty workspace and is
written in one transaction.

## Tasks and processes

`tasks` reserves an account for one operation; colliding operations on the same account are
refused. Stop cancels queued and running work and closes task browsers, but leaves games and
browsers that wait for the user. Tasks stopped while still queued leave no log line, so stopping a
check of thousands of accounts does not push out the log. Quitting drains the launcher and
sessions, closes managed browsers and the database, and leaves games running.

Managed browsers get UUID profiles in Nocta's data folder. A Job Object owns the process
tree. If cleanup fails the account is quarantined until a retry succeeds.

## Launching

1. **Target.** The configured place, a server (`gameInstanceId`), a private server or an invite
   link. Share links are expanded once through Roblox's share-links API with any signed-in account
   and the result is remembered. Private server access codes never leave the backend; the interface
   refers to them by opaque IDs that expire.
2. **Precheck.** Optionally checks that the account can join. When Roblox wants a captcha first,
   the launch stops and points to the account's browser, where the user clears it.
3. **Ticket and URI.** An authentication ticket is requested and a `roblox-player:` URI is built.
4. **Start.** The URI goes to the system handler, or to `RobloxPlayerBeta.exe` of Nocta's own
   install. The new process is claimed by PID and creation time, so a reused PID is never mistaken
   for it.

Game search uses Roblox's public omni-search without a cookie, with icons from the thumbnails API
and a five-minute cache per query. Server lists are fetched sorted by Roblox across the whole game.

### Nocta's own Roblox install (Windows)

Versions live in `{data}/roblox/versions/{guid}`, independent of the system `roblox-player`
handler, which is never changed. The current version comes from
`clientsettingscdn.roblox.com/v2/client-version/WindowsPlayer`; the package manifest and packages
come from `setup.rbxcdn.com`, each package is checked against its MD5 and extracted with path
checks (Roblox's leading `\` in entry names is relative). A version is assembled under `.staging/`
and moved into place only when complete. Before each launch `ClientSettings/ClientAppSettings.json`
is written with flags from Roblox's allowlist only (in effect since 29 September 2025). Update
modes are automatic, ask and manual; a version can be pinned, and versions in use are never
removed.

## IPC and synchronization

Rust models generate the TypeScript contracts with ts-rs (`bun run contracts`). The frontend calls
only the registered commands. A snapshot holds account metadata, folders, settings without keys,
the last 500 log entries, tasks, metrics, the Roblox client's state and running instances. Accounts
and logs are left out of a snapshot event when they have not changed since the last one.

The frontend subscribes to `nocta:snapshot` before loading, ignores older revisions and batches
updates (200 ms). Imports report progress over a Tauri channel. Settings and data changes are
serialized; frequent task updates are published separately.

HTTP adapters limit time and body size, sanitize errors, check every redirect and attach cookies
only to allowed Roblox hosts. The frontend has no shell or filesystem access.

## Interface

- `src/app`: the shell (title bar, sidebar, activity panel, notifications, shortcuts).
- `src/features/<area>`: one folder per view; larger parts get their own folder, for example
  `features/accounts/launch/` for the launch panel.
- `src/i18n`: translations and locale-aware formatting. Components get text from `useT()`.
- `src/theme`: theme tokens, built-in themes and the theme store.
- `src/styles`: CSS, organized like the features; colors only through theme variables.
