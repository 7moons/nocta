# Feature parity with Roblox Account Manager

Nocta took over the useful features of [Vaelixx/Roblox-Account-Manager](https://github.com/Vaelixx/Roblox-Account-Manager)
(the rebuilt successor of ic3w0lf's manager) and of [MultiAlt](https://github.com/luanmacea/MultiAlt). This page lists each feature, where it lives in Nocta
and the test that proves it. "Real" tests talk to Roblox, Windows or real files and are run by hand
(see [Testing](testing.md)); the others run in CI.

## Done

| Feature | In Nocta | Proof |
| --- | --- | --- |
| Join modes: squad, lowest ping, server hop, follow a player | Launch panel → Server | `squad_and_follow_targets_settle_to_one_exact_server`, `join-modes.spec.ts`; real: `real_squad_lowest_ping_and_hop_pick_live_servers` |
| Frame-rate cap via Roblox's own setting, AFK profile | Settings → Game launch → Performance and AFK | `launch::fps` tests; real: `real_roblox_settings_get_the_cap_and_their_own_value_back`, `live_frame_rate_cap_reaches_nocta_clients_and_leaves_the_users_file_alone` |
| Auto-rejoin after a crash with back-off, Discord webhook | Launch options, Settings → Crashes and notifications | `a_crashed_client_rejoins_the_same_server_and_closing_it_does_not`, `webhook` tests |
| Presets and schedules (close after a set time) | Automation | `launch::automation` tests, `friends-automation.spec.ts` |
| Schedules close only the clients they started | Automation | `a_schedule_closes_only_the_clients_it_started` |
| Alias, note, color tag, pinned accounts | Edit account | `details_and_authenticator_keys_are_saved_and_survive_a_backup`, `account-features.spec.ts` |
| Avatars and online status | Accounts table | `presence_of_own_accounts_is_refreshed_with_a_working_session`, `a_banned_session_falls_back_to_anonymous_presence_and_explains_friends`; real: `real_presence_and_friends_of_saved_accounts` |
| Robux and playtime per account | Account actions | `robux_balances_are_checked_with_each_accounts_own_session`, upkeep test, `client-upkeep.spec.ts` |
| Old-session flag and one-click fix | Accounts banner | `client-upkeep.spec.ts`, `account-features.spec.ts` |
| Two-step verification codes | Edit account, account actions | RFC 6238 vectors in `totp` |
| Export to CSV or JSON | Selection bar → Export | `exports_selected_accounts_as_csv_and_json` |
| Add an account by signing in on Roblox's page | Add accounts | `a_signed_in_session_becomes_an_account_or_refreshes_the_existing_one` |
| Import from ic3w0lf's Roblox Account Manager (DPAPI, password, plain) | Settings → Backup & import | `ram_import` tests (DPAPI, libsodium-encrypted fixture), `roblox_account_manager_files_import_with_aliases_notes_ids_and_groups`, `legacy.spec.ts`; real: `real_ram_file` with a .NET-written file |
| Friends page with joining | Friends | `friends_list_their_games_and_avatars_with_players_in_game_first`, `friends-automation.spec.ts` |
| Anti-AFK with random timing | Settings → Game launch → Anti-AFK | upkeep test; real: `the_key_reaches_a_real_window_and_focus_goes_back` |
| Memory trimming and a force-close memory limit | Settings → Performance and AFK | upkeep test; real: `reads_and_trims_the_memory_of_a_real_process` |
| Command palette (Ctrl+K) | Everywhere | `friends-automation.spec.ts` |
| Clipboard cleared 30 seconds after copying secrets | Copy, 2FA codes | `account-features.spec.ts` |
| Global shortcuts (also per preset) | Settings → General → Shortcuts, presets | `hotkeys` tests, `client-upkeep.spec.ts` |
| `--launch` / `--preset` on the command line | [Operations](operations.md#command-line) | `cli` tests, `command_line_launches_accounts_by_name_or_alias_and_runs_presets_by_name`; checked against the running app |
| System tray and start with Windows | Settings → General → System | `client-upkeep.spec.ts` (settings); the tray and autostart use Tauri's plugins |
| FastFlags limited to Roblox's allowlist | Settings → Roblox client → FastFlags | `fast_flags` tests, `user_fast_flags_are_added_and_win_over_built_in_ones`, `client-upkeep.spec.ts` |
| Eight languages: English, Polish, German, Spanish, French, Portuguese (Brazil), Turkish, Russian | Settings → General | `language.spec.ts`, `languages-compact.spec.ts` (placeholders and plurals of every language, a screenshot per language); TypeScript checks all have the same keys |
| Compact account list | Settings → Appearance, Ctrl+K | `languages-compact.spec.ts` (13 instead of 9 rows on a 1280×720 window) |
| The next account starts once the previous client is in its game (from Roblox's log) | Launch options, Settings → Game launch → Running clients | `the_next_account_starts_once_the_previous_client_is_in_game`, `client_logs` tests, `client-care.spec.ts`; real: `real_client_logs` (this computer's Roblox logs) |
| Client stage and Roblox's disconnect reason | Instances | `client_logs` tests, `running_clients_are_named_profiled_arranged_and_restarted_when_lost_or_frozen`, `client-care.spec.ts` |
| Windows named after their account | Running clients | `running_clients_are_named_…`; real: `a_real_window_is_named_placed_and_seen_hanging` |
| Arrange windows in a grid across monitors | Instances, Ctrl+K, automatic | `window_grid` tests, `running_clients_are_named_…`, `client-care.spec.ts`; real: `a_real_window_is_named_placed_and_seen_hanging` |
| Frozen and disconnected clients are closed and rejoin | Running clients | `running_clients_are_named_…`; real: `a_real_window_is_named_placed_and_seen_hanging` (hang detection) |
| Other accounts restart every N minutes, main accounts keep running | Running clients, Edit account → Main account | `alts_restart_on_a_timer_but_main_accounts_keep_running`, `client-care.spec.ts` |
| Client profiles: priority, EcoQoS, CPU limit, window size for main and other accounts | Settings → Game launch → Client profiles | `priority_efficiency_mode_and_cpu_limit_apply_to_a_real_process`, `running_clients_are_named_…`, `client-care.spec.ts` |
| Keep the computer awake while clients are looked after | Running clients | `running_clients_are_named_…`; real: `a_real_window_is_named_placed_and_seen_hanging` (power state) |
| Server region filter (with every picking strategy) | Launch panel → Server → Region | `a_region_keeps_the_strategys_order_and_skips_servers_elsewhere`, `a_region_with_no_matching_server_says_how_many_were_checked`, `region` tests, `account-tools.spec.ts`; real: `real_region`, `live_region_filter_picks_a_server_in_the_chosen_region` (Europe → London, North America → Washington, each checked again) |
| Fullest and emptiest server | Launch panel → Server | `fullest_and_emptiest_pick_by_player_count_and_skip_full_servers`, `client-care.spec.ts`; real: `real_social` |
| Streamer mode: usernames, Roblox IDs, avatars and names in logs hidden; aliases stay | Settings → Appearance, Ctrl+K | `account-tools.spec.ts`, `streamer_mode_keeps_usernames_out_of_window_titles` |
| Sign out other sessions (keeping Nocta's new cookie), display name, Quick Login, privacy | Account actions → Roblox account | `launch::tests::account_web` (exact requests, CSRF round trip, rotated cookie, verification challenge), `account-tools.spec.ts`; live: `live_privacy_is_changed_read_back_and_restored`, `live_quick_login_code_is_checked_and_approved`, `live_display_name_is_changed`, `live_other_sessions_are_signed_out_and_nocta_keeps_working` |
| One privacy setting for many accounts | Selection → Set privacy… | `privacy_is_read_with_its_options_and_set_on_many_accounts`, `account-tools.spec.ts` |
| Own accounts become friends: everyone with everyone, or with one main account | Selection → Make them friends… | `own_accounts_become_friends_skipping_existing_friends_and_captchas`, `too_many_friendships_are_refused_before_any_request`, `account-tools.spec.ts`; live: `live_two_own_accounts_become_friends_and_are_unfriended_after` |
| Anti-AFK clicks a point of the window, per account or for all | Edit account, Settings → Anti-AFK | `anti_afk_clicks_an_accounts_own_point_before_the_general_action`; real: `the_key_reaches_a_real_window_and_focus_goes_back` (the click lands on the point) |
| Accounts dragged into an own order; own fields per account, found by search | Accounts table, Edit account | `own_order_fields_and_click_points_are_saved_checked_and_backed_up`, `account-tools.spec.ts` |
| Safe display mode: restart without GPU acceleration when the window stays blank | Automatic, Settings → Appearance | `account-tools.spec.ts`; checked on the running app (WebView2 started with `--disable-gpu` after the automatic restart) |

## Not taken over (on purpose, or later)

- **Changing the machine ID, MAC address or registry** (MultiAlt) is ban evasion and is left out,
  as is creating Roblox accounts.
- Privacy options that Roblox puts behind its own check (an age check, or "Inherited" from
  another setting) are locked in Nocta, and leaving one asks first: Nocta cannot switch back.
  Options marked `SelfUpdateSetting` are the owner's own choice and stay open.
- Checked on live Roblox with the user's accounts (`launch::tests::live`): the region filter (Europe and
  North America, each confirmed again), a privacy change read back and put back, a Quick Login
  code created, checked and approved (the other side sees `Validated`), and two own accounts made
  friends and unfriended again. With the owner's consent (they cannot be undone right away):
  a display name changed and confirmed on the public profile, and every other session signed out
  while Nocta kept working with the new cookie (the old one is refused).
- **Account PIN** (MultiAlt) is left out: Roblox answers `{"success":false}` to setting one
  today (checked live), so Nocta could only pretend.
- **Local HTTP API** and **plugins**: a local server or third-party code next to encrypted cookies
  needs its own security design; left out for now.
- **Master password and lock**, **per-account proxies** and **automatic updates** have open
  questions (vault migration, proxying the game client, release signing) and are planned
  separately.
