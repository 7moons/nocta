# Operations

## Data

By default Nocta keeps its data in the system app-data folder `app.nocta.desktop`
(`%APPDATA%\app.nocta.desktop` on Windows). With a `portable.flag` file next to the executable it
uses `data/` next to the program instead. Debug builds accept `NOCTA_DATA_DIR`; release builds
ignore it.

The encryption key is stored in Windows Credential Manager, tied to the database's UUID, so copying a data
folder to another computer does not move the key. To move accounts and settings, use **Settings →
Backup & import → Export** and **Restore** (passwords need at least 12 characters; restore needs an
empty workspace). A backup contains account secrets and settings, so treat the file and its
password as sensitive.

Settings → Backup & import also imports the accounts and folders of a database of the previous
Nocta app, chosen with **Choose file**. That file is only read.

## Accounts import format

One account per line: `username:password:cookie`. The password may be empty
(`username::cookie`); the cookie may contain colons. Duplicates are checked against all accounts,
including Trash. "Update accounts" replaces credentials of existing accounts matched by username.

## Browser runtime

Session checks and logins use a bundled Chromium in
`runtime/chrome`, never the user's own Chrome. Stage a fresh distribution with:

```bash
bun run runtime:stage-browser <distribution-directory> <four-part-version>
```

The script refuses to overwrite a non-empty `runtime/chrome`.

`bun run runtime:manifest` records the SHA-256 of every runtime file; `bun run runtime:verify`
checks them.

## Memory and drawing

On Windows the window is drawn by WebView2, which runs as several processes next to Nocta's own.
Nocta starts it lean: no GPU (the GPU process alone held about 90 MB), the GPU and network
services inside the browser process, one renderer and no background services. Measured with a
release build and 241 accounts: about 140 MB in 5 processes instead of about 280 MB in 7. Nocta's
own process uses about 8 MB. A hidden or minimized window asks WebView2 to keep as little memory
as it can.

The page itself loads English and the views of the accounts list up front; other languages and
views load when first opened. Snapshot updates leave out the accounts and logs when they did not
change, so the window does not rebuild them several times a second while tasks run.

GPU acceleration can be turned on in Settings → Appearance (Nocta restarts; a
`gpu-acceleration` file in the data folder remembers it). With it on, a window that stays blank
for 25 seconds after a start makes Nocta restart without it. `nocta --safe-display` starts once
without the GPU. Arguments in `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS` set by the user replace
Nocta's own. Development builds only watch for a blank window when `NOCTA_UI_TIMEOUT_SECS` is set.

## Releases

```bash
bun run release
```

Verifies the runtime manifest, builds the frontend and the Tauri app, and writes the installer,
the portable package and `SHA256SUMS` to `release/<version-platform-arch-timestamp>/`.

It builds an NSIS per-user installer and a portable ZIP. `--debug` builds test packages. Builds are
unsigned until a code-signing certificate is configured.

## Lifecycle and updates

The window has a custom title bar. Closing quits the app:
running work is cancelled and drained, managed browsers close, games keep running. With **Keep
running in the tray** (Settings → General → System) the close button hides the window instead, so
schedules, anti-AFK and auto-rejoin keep working; quit from the tray menu. **Start with Windows**
registers Nocta to start at sign-in (with `--autostart`), optionally straight into the tray.

## Command line

```
nocta --preset "Morning farm"
nocta --launch 1818 --accounts alice,Main,3
nocta --launch "https://www.roblox.com/share?code=…&type=Server" --accounts alice
```

`--preset` runs a launch preset by name or ID. `--launch` takes a Place ID or any Roblox link and
the accounts by username, alias or Nocta ID. When Nocta is already open the running window gets
the command; results appear in the activity log.

If a managed browser cannot be cleaned up, the account is blocked until a retry (every 5 seconds)
removes its process and profile. On startup Nocta removes leftovers of its own UUID
folders only.

Updating is manual: quit Nocta, keep the data folder and replace the program and runtime. The
database schema updates through SQLx migrations; applied migrations are never edited.
