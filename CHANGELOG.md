# Changelog

Every version of Nocta and what changed in it. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.0.0] - 2026-10-18

The first public release.

### Accounts

- Import accounts as `username:password:cookie` lines with a preview, or add one by signing in on Roblox's own page.
- Folders and folder groups in the sidebar: create and rename in place, set groups from the menu (right click or ⋯), reorder folders and move accounts onto them by dragging.
- Any folder color: presets, a color picker with a hex field, and your own saved palette.
- Your own account order by drag and drop, aliases, notes, color tags, pins and named fields.
- Avatars, online status, Robux, playtime and two-step verification codes for each account.
- Copy accounts in your own formats, export them to CSV or JSON, and restore them from Trash.
- Import from the previous Nocta app and from Roblox Account Manager (`AccountData.json`).
- Password-protected backups.

### Sessions

- Check which sessions still work, and validate or refresh cookies.
- Open an account in its own signed-in browser, titled with the account's name.
- Accounts that need a captcha are marked, and their browser is where it is cleared.

### Launching

- Find a game by name or paste a Roblox link, including share links and private servers.
- Pick a server from the server browser, or a strategy: room for every account, lowest ping, server hop, fullest, emptiest, one region, or the server a player is in.
- Accounts start one after another, each once the previous one is in its game.
- Nocta's own Roblox install with version pinning, update control, a low-graphics mode and a FastFlag editor limited to Roblox's allowlist.
- Multi-instance, so several Roblox clients run side by side.

### Client upkeep

- Anti-AFK before Roblox's 20-minute kick, waiting while you type.
- Auto-rejoin after crashes and disconnects, with optional Discord webhook notices.
- A RAM cap per client, memory trimming, efficiency mode and a CPU limit, set separately for main accounts and the others.
- A frame-rate cap that reaches only the clients Nocta starts.
- Client windows named after their account, arranged in a grid, and resizable down to 1×1 pixel, also by dragging the border.
- Frozen and disconnected clients are restarted.

### Automation

- Launch presets with optional system-wide shortcuts.
- Schedules that run presets on chosen days and close what they started later.
- Command line: `nocta --preset "<name>"` and `nocta --launch <place or link> --accounts a,b`.

### Roblox account tools

- Sign an account out of every other device, change its display name and privacy, and approve Quick Login codes.
- Make your own accounts friends: everyone with everyone, or with one main account.

### Interface

- Overview of accounts in a game, memory, the next schedule and accounts that need attention.
- Command palette (Ctrl+K), global shortcuts, a tray icon and starting with Windows.
- Themes with a live theme editor, streamer mode and a compact account list.
- English, Polish, German, Spanish, French, Portuguese (Brazil), Turkish and Russian.
- Times in 12- or 24-hour format, or the way the chosen language writes them.

[1.0.0]: https://github.com/7moons/nocta/releases/tag/v1.0.0
