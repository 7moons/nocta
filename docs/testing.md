# Testing

## Everyday checks

These run in CI on Windows for every push and pull request:

| Command | What it covers |
| --- | --- |
| `cargo fmt --all -- --check` | Rust formatting. |
| `cargo clippy --workspace --all-targets -- -D warnings` | Rust lints, warnings as errors. |
| `cargo test -p nocta-core` | Core unit and integration tests against local HTTP fixtures: storage, backups, import, tasks, sessions, launch targets and invites, server lists, process tracking, the Roblox client installer, game search and share links. |
| `bun run contracts` + `git diff --exit-code -- src/generated/contracts.ts` | The TypeScript contracts match the Rust models. |
| `bun run lint`, `bun run format:check` | ESLint and Prettier. |
| `bun run build` | TypeScript type check (including translation keys) and the production bundle. |
| `bun run test` | Playwright tests of the interface with a mocked backend: import, the 10,000-row table, launch panel and server browser, Roblox client settings, running instances, the old-database import and language switching. |

Playwright uses its own Chromium (`bunx playwright install chromium`); set `NOCTA_TEST_BROWSER` to
a Chromium executable to use another one, for example the bundled runtime:

```bash
NOCTA_TEST_BROWSER="$PWD/runtime/chrome/chrome.exe" bun run test
```

Screenshots from the UI tests go to `test-results/screenshots/`.

## Tests against real services and programs

Marked `#[ignore]` because they need the network, a real browser runtime, Roblox, or your own data.
None of them change your Nocta data.

| Command | What it does |
| --- | --- |
| `NOCTA_TEST_BROWSER=runtime/chrome/chrome.exe cargo test -p nocta-core --test browser_lifecycle -- --include-ignored` | Starts the managed Chromium: separate profiles and cookies, cancellation over CDP, cleanup. |
| `cargo test -p nocta-core --test managed_client -- --ignored` | Installs Roblox from the real CDN into a temporary folder, starts it, claims the process and checks that the flags were applied. |
| `cargo test -p nocta-core real_game_search -- --ignored --nocapture` | Searches Roblox for a game by name. |
| `NOCTA_SHARE_DATA=<data folder> NOCTA_SHARE_LINK=<share link> cargo test -p nocta-core --test share_link -- --ignored --nocapture` | Expands a real share link with an account from a copy of a Nocta data folder. |
| `NOCTA_LEGACY_DB=<old database> cargo test -p nocta-core real_legacy -- --ignored --nocapture` | Imports an old Nocta database (a copy) into a throwaway store. |
| `NOCTA_SHARE_DATA=<data folder> cargo test -p nocta-core --test real_social -- --ignored --nocapture` | Picks live servers by squad, lowest ping and hop; looks up users and avatars; reads presence (with and without a session) and friends with accounts from a copy of a Nocta data folder, skipping banned ones. |
| `NOCTA_RAM_FILE=<AccountData.json> [NOCTA_RAM_PASSWORD=<password>] cargo test -p nocta-core --test real_ram_file -- --ignored --nocapture` | Reads a Roblox Account Manager file (DPAPI, password or plain) without printing secrets. |
| `cargo test -p nocta-core --lib the_key_reaches -- --ignored --nocapture` | Opens a real window, lets anti-AFK press its key and click a point in it, and checks both arrived where they should and focus went back (takes focus and the mouse for a moment). |
| `cargo test -p nocta-core --lib a_real_window_is_named -- --ignored --nocapture` | Opens a real window: names it, places and resizes it, sees it hang, and checks the computer is kept awake. |
| `NOCTA_SHARE_DATA=<data folder> cargo test -p nocta-core --lib launch::tests::live:: -- --ignored --nocapture --test-threads 1` | Region filter, privacy (changed and put back), Quick Login and friendships between two own accounts (undone after) on live Roblox, with a copy of a Nocta data folder. Privacy only touches settings both of whose values need no Roblox check. |
| `cargo test -p nocta-core --test real_region -- --ignored --nocapture` | Locates real Roblox datacenter addresses with the lookup the region filter uses. |
| `cargo test -p nocta-core --release --test real_client_logs -- --ignored --nocapture` | Reads this computer's Roblox logs and counts joins, in-game lines and disconnect reasons. |
| `cargo test -p nocta-core --test real_fps_cap -- --ignored --nocapture` | Sets and removes the frame-rate cap on a copy of this computer's Roblox settings and checks the file is otherwise unchanged. |
| `NOCTA_SHARE_DATA=%APPDATA%\app.nocta.desktop cargo test -p nocta-core --lib launch::tests::live_fps -- --ignored --nocapture --test-threads 1` | Joins a game twice with one real account: the capped client keeps its cap (CPU against an uncapped control), Roblox's settings file holds the user's value once it is in game and again after the user closes it. |

## Native smoke tests

`scripts/native-smoke.mjs` drives the built desktop app over the WebView2 debugging port
(Windows): CRUD and restart (`--verify-restart`), sessions (`--sessions`), launch and import
features (`--features`) and the packaged build (`--packaged`). It needs its own data folder named
`native-smoke-data`: start a debug build with `NOCTA_DATA_DIR` pointing to it, or put a release
build with `portable.flag` inside it.
`scripts/native-scale-smoke.mjs` runs import, session checks and Stop on 10,000 synthetic accounts and
writes `test-results/native-scale-result.json`: snapshot times and frame gaps in the window while
checks run, and how long Stop takes. Both use synthetic accounts only and refuse other databases.
Their cookies are empty or malformed, so Nocta rejects them before any request, and the one launch
is stopped during its pre-launch wait. Each mode ends by closing Nocta, so start it again before the
next one.

`scripts/native-folders-smoke.mjs` checks the folder panel in the real window with real Windows mouse input
(`scripts/native-mouse.ps1`): hovering a color, dragging a folder into a group and within it, and
dragging an account onto a folder. Playwright's own drag events skip the window's native drag
handling, so the interface tests alone cannot show that dragging works in the app; the script's
header says how to start Nocta for it.
