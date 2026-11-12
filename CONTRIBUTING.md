# Contributing to Nocta

Thanks for helping. This guide covers setting up, how the code is organized, the checks a change
has to pass, and how to add translations.

## Setting up

You need [Bun](https://bun.sh) 1.3, [Rust](https://rustup.rs) 1.93 and the
[Tauri prerequisites](https://tauri.app/start/prerequisites/).

```bash
bun install --frozen-lockfile
bun run desktop        # the app, rebuilding on changes
bun run dev            # only the interface, as a read-only browser preview
```

## Layout

| Path | What it holds |
| --- | --- |
| `crates/nocta-core` | All domain logic in Rust, independent of Tauri: storage, accounts, sessions, launcher. |
| `src-tauri` | The desktop shell: window, app state and the IPC commands in `src/commands/`. |
| `src` | The React interface. `app/` is the shell, `features/` holds one folder per area, `i18n/` the translations, `theme/` the theme system, `styles/` the CSS. |
| `src/generated/contracts.ts` | TypeScript types generated from the Rust models. Do not edit by hand. |
| `tests/ui` | Playwright tests of the interface with a mocked backend. |
| `scripts` | Release, runtime staging and native smoke tests. |

[Architecture](docs/architecture.md) explains how the pieces talk to each other.

## Sign-off (DCO)

Every commit needs a sign-off line. It certifies the [Developer Certificate of Origin](https://developercertificate.org): you wrote the change, or you have the right to submit it under the project's license (GPL-3.0). There is no paperwork and no agreement to sign; the rights stay with you.

```bash
git commit -s -m "your message"     # appends: Signed-off-by: Your Name <you@example.com>
git commit --amend -s               # add it to the last commit
git rebase --signoff main           # add it to every commit of a branch
```

The name and email come from your git config. Pull requests with unsigned commits are asked to add the sign-off before they are merged. Bots such as Dependabot are exempt.

## Checks

Every pull request runs these on Windows (`.github/workflows/verify.yml`); run them
before pushing:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p nocta-core
bun run contracts        # then: git diff --exit-code -- src/generated/contracts.ts
bun run lint
bun run format:check
bun run build
bun run test             # Playwright
```

Tests that talk to real services or start real programs are marked `#[ignore]`; [Testing](docs/testing.md)
lists them and when to run them.

## Conventions

- **Small, focused files.** A module or component does one thing. When a file grows several
  responsibilities, split it (see `features/accounts/launch/` or `crates/nocta-core/src/accounts/`).
- **Rust:** `rustfmt` and Clippy with warnings as errors. Errors shown to users are `CoreError`
  values with a sentence a user can act on. Never log or return secrets; types holding them redact
  `Debug`.
- **TypeScript:** strict mode, ESLint (`bun run lint`) and Prettier (`bun run format`). Components
  get their text from `useT()`; no user-facing string is written inline.
- **Contracts:** change a model in `crates/nocta-core/src/models/`, then run `bun run contracts`
  and commit the regenerated `src/generated/contracts.ts`.
- **Styles:** colors only through the theme variables from `src/theme/tokens.ts`; stylesheets live
  next to their feature under `src/styles/features/`.
- **Comments** explain why, not what.

## Translations

The interface text lives in `src/i18n/locales/`. `en.ts` is the source; every other locale must
have exactly the same keys, which TypeScript checks.

- **Adding text:** add the key to `en.ts` and to every other locale, then use it with
  `t("section.key")`. Parameters are written `{name}`; counts use plural objects such as
  `{ one: "{count} account", other: "{count} accounts" }` (any `Intl.PluralRules` category works).
- **Adding a language:** copy `en.ts` to `<code>.ts`, translate it with the plural categories of
  that language (`new Intl.PluralRules("<code>").resolvedOptions().pluralCategories` lists them),
  and add it to `LANGUAGES` in `src/i18n/index.ts`.
- **Checking:** `languages-compact.spec.ts` fails when a translation drops or misspells a
  `{placeholder}` or loses its plural forms. Translations other than English and Polish were
  written without native-speaker review; corrections from native speakers are very welcome.

Messages that come from Roblox or the backend are shown as they arrive.

## Pull requests

Add what changes for the user to `CHANGELOG.md` under the next version; the app shows that file
under its version number.

Describe what changes for the user and how you checked it. Include a screenshot for visible
changes, and keep unrelated refactors in separate pull requests.
