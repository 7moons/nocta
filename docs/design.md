# Design

Nocta is a tool for working through a lot of data, so the interface is dense, calm and fast to
scan.

## Visual direction

- A neutral, very dark default theme; panels differ by small steps in lightness.
- Thin, muted borders and separators instead of heavy shadows; small radii and compact spacing.
- Bright primary text; descriptions and metadata at lower contrast.
- Line icons, usually next to a label.
- Color signals state (healthy, captcha, invalid, destructive), never decoration.
- Settings rows put the title and description on the left and the control on the right.
- Tables align columns strictly, show status as small labels and keep row actions at the right.
- The layout is designed for 1280×720 and up. Below 800 px of height the activity panel starts
  folded to leave room for the accounts.

## Layout

A sidebar with the main views (Accounts, Overview, Settings); in the accounts views
it also holds the folders and Trash. The bottom activity panel has Logs, Tasks and Instances. Every
view has the same header: back button, icon, title, a short summary and its actions.

## Launching

One place decides where accounts launch. The button with the game's icon and name next to
**Launch selected** opens the launch panel:

1. **Game:** search Roblox by name, paste a link or a Place ID, or pick a saved or recent game.
2. **Server:** any server, a server chosen from the list (loaded right away, sortable, with the
   first selected account's private servers on top), or the server from a pasted link.

**Launch selected**, the launch button on a row and the account menu all launch straight into that
target. Without a game they open the panel and ask for one. Launch options (time between accounts,
delay, session check, multi-instance) are folded into the panel's footer.

## Themes

Every color is a theme token (`src/theme/tokens.ts`) exposed as a CSS variable such as `--surface`
or `--text-muted`. Stylesheets use only these variables; derived shades come from `color-mix`.

- Built-in themes are in `src/theme/presets.ts` and `src/theme/community.ts`; a new built-in theme
  is one more entry with every token.
- Custom themes and the active theme are stored locally (`nocta.theme.v1`) and are not part of
  backups. Themes can be exchanged as `nocta-theme` JSON; missing or invalid colors are filled in
  from the base theme of the same mode.
- The theme editor previews changes across the whole app; nothing is saved until **Save theme**,
  and **Cancel** restores the active theme.

## Language

The interface ships in English, Polish, German, Spanish, French, Portuguese (Brazil), Turkish and
Russian (Settings → General → Language) and starts in the system language when it is one of them
(a `pt-PT` system gets Brazilian Portuguese). Long words are cut with an ellipsis in table cells
instead of wrapping. Dates and numbers follow the chosen language. See
[Contributing](../CONTRIBUTING.md#translations) for adding text or a language.
