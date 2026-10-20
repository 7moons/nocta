//! Picking a server for the selected accounts (squad, lowest ping, server hop) and following a
//! player into their server. Both turn into an exact `Job` target before any account launches, so
//! every account of one launch goes to the same place.
use super::{servers::ListRequest, Launcher};
use crate::{
    models::{JoinStrategy, LaunchTargetInput, ServerKind, ServerListOptions, ServerRow},
    CoreError, CoreResult,
};
use rand::seq::SliceRandom;
use tokio_util::sync::CancellationToken;

/// Pages of 100 public servers looked at before giving up on a squad fit (or a region).
const SQUAD_PAGES: usize = 3;
/// Most servers whose region is looked up for one launch; each asks Roblox once.
const REGION_CHECKS: usize = 12;

/// A chosen server and a sentence saying why, for the task log.
#[derive(Debug, PartialEq, Eq)]
pub struct Pick {
    pub job_id: String,
    pub note: String,
}

fn free(row: &ServerRow) -> u32 {
    row.max_players.saturating_sub(row.playing)
}

fn joinable(row: &ServerRow) -> bool {
    row.kind == ServerKind::Public && row.joinable && !row.id.is_empty() && free(row) > 0
}

/// Servers from `rows` in the order `strategy` prefers them, each with a sentence saying why.
/// `accounts` is how many accounts join at once; `exclude` holds servers to avoid (the current
/// one for a hop).
pub fn rank_servers(
    rows: &[ServerRow],
    strategy: JoinStrategy,
    accounts: u32,
    exclude: &[String],
) -> Vec<Pick> {
    let mut candidates: Vec<&ServerRow> = rows
        .iter()
        .filter(|row| joinable(row) && !exclude.contains(&row.id))
        .collect();
    let pick = |row: &&ServerRow, note: String| Pick {
        job_id: row.id.clone(),
        note,
    };
    match strategy {
        // The fullest server that still has room for everyone keeps the group together in a
        // lively server.
        JoinStrategy::Squad => {
            candidates.retain(|row| free(row) >= accounts);
            candidates.sort_by(|a, b| b.playing.cmp(&a.playing).then(a.id.cmp(&b.id)));
            candidates
                .iter()
                .map(|row| {
                    pick(
                        row,
                        format!(
                            "Squad join: a server with {} of {} players has room for {accounts}.",
                            row.playing, row.max_players
                        ),
                    )
                })
                .collect()
        }
        // Roblox often leaves ping out; then the emptiest servers are the safest choice.
        JoinStrategy::LowestPing => {
            let (mut pinged, mut rest): (Vec<&ServerRow>, Vec<&ServerRow>) =
                candidates.into_iter().partition(|row| row.ping.is_some());
            pinged.sort_by_key(|row| row.ping);
            rest.sort_by_key(|row| row.playing);
            pinged
                .iter()
                .map(|row| pick(row, format!("Lowest ping: {} ms.", row.ping.unwrap_or(0))))
                .chain(rest.iter().map(|row| {
                    pick(
                        row,
                        "Lowest ping: Roblox reported no ping, so the emptiest server was used."
                            .into(),
                    )
                }))
                .collect()
        }
        JoinStrategy::Fullest => {
            candidates.sort_by(|a, b| b.playing.cmp(&a.playing).then(a.id.cmp(&b.id)));
            candidates
                .iter()
                .map(|row| {
                    pick(
                        row,
                        format!(
                            "Fullest server: {} of {} players.",
                            row.playing, row.max_players
                        ),
                    )
                })
                .collect()
        }
        JoinStrategy::Emptiest => {
            candidates.sort_by(|a, b| a.playing.cmp(&b.playing).then(a.id.cmp(&b.id)));
            candidates
                .iter()
                .map(|row| {
                    pick(
                        row,
                        format!(
                            "Emptiest server: {} of {} players.",
                            row.playing, row.max_players
                        ),
                    )
                })
                .collect()
        }
        JoinStrategy::Hop => {
            candidates.shuffle(&mut rand::thread_rng());
            candidates
                .iter()
                .map(|row| {
                    pick(
                        row,
                        format!(
                            "Server hop: a random server with {}/{} players.",
                            row.playing, row.max_players
                        ),
                    )
                })
                .collect()
        }
    }
}

/// The server `strategy` likes best, if any can be joined.
pub fn pick_server(
    rows: &[ServerRow],
    strategy: JoinStrategy,
    accounts: u32,
    exclude: &[String],
) -> Option<Pick> {
    rank_servers(rows, strategy, accounts, exclude)
        .into_iter()
        .next()
}

impl Launcher {
    /// Turns `Pick` and `Follow` targets into an exact server for all `accounts`, with a note for
    /// the log. Other targets are returned unchanged.
    pub(super) async fn settle_target(
        &self,
        input: LaunchTargetInput,
        accounts: &[u32],
        cancel: &CancellationToken,
    ) -> CoreResult<(LaunchTargetInput, Option<String>)> {
        match input {
            LaunchTargetInput::Pick {
                place_id,
                strategy,
                exclude_job,
            } => {
                let place_id = super::target::validate_place_id(&place_id)?;
                let exclude: Vec<String> = exclude_job.into_iter().collect();
                let needed = accounts.len() as u32;
                let region = self.accounts.config().await?.server_region;
                // Finding a server's region asks Roblox with the first account's session.
                let asker = match (region, accounts.first()) {
                    (Some(_), Some(id)) => Some(self.accounts.secrets(*id).await?.cookie.clone()),
                    (Some(_), None) => {
                        return Err(CoreError::Validation("Select at least one account.".into()))
                    }
                    (None, _) => None,
                };
                let mut checked = 0;
                let mut cursor = None;
                for _ in 0..SQUAD_PAGES {
                    let options = ServerListOptions {
                        // Squads look among the fullest servers first; the others among all.
                        most_players_first: matches!(
                            strategy,
                            JoinStrategy::Squad | JoinStrategy::Fullest
                        ),
                        exclude_full: true,
                        cursor: cursor.clone(),
                    };
                    let key = format!("pick-{}", uuid::Uuid::new_v4());
                    let page = self
                        .servers
                        .list(ListRequest {
                            request_key: &key,
                            place_id: &place_id,
                            vip_cookie: None,
                            include_vip: false,
                            previous: None,
                            options: &options,
                        })
                        .await?;
                    if cancel.is_cancelled() {
                        return Err(CoreError::Cancelled);
                    }
                    let ranked = rank_servers(&page.items, strategy, needed, &exclude);
                    if let (Some(region), Some(cookie)) = (region, asker.as_deref()) {
                        for pick in ranked {
                            if checked >= REGION_CHECKS {
                                break;
                            }
                            checked += 1;
                            match self
                                .locate_server(cookie, &place_id, &pick.job_id, cancel)
                                .await
                            {
                                Ok(location) if location.region == region => {
                                    return Ok((
                                        LaunchTargetInput::Job {
                                            place_id,
                                            job_id: pick.job_id,
                                        },
                                        Some(format!("{} In {}.", pick.note, location.place)),
                                    ));
                                }
                                Ok(_) => {}
                                Err(CoreError::Cancelled) => return Err(CoreError::Cancelled),
                                // A signed-out or banned session cannot ask about any server.
                                Err(CoreError::Validation(message))
                                    if message == crate::roblox::web::SIGNED_OUT
                                        || message == crate::roblox::web::MODERATED
                                        || message == crate::roblox::web::RATE_LIMITED =>
                                {
                                    return Err(CoreError::Validation(message))
                                }
                                Err(error) => {
                                    tracing::debug!(%error, "Server region not found");
                                }
                            }
                        }
                        cursor = page.next_cursor;
                        if checked >= REGION_CHECKS || cursor.is_none() {
                            return Err(CoreError::Validation(format!(
                                "None of the {checked} servers checked is in {}. Try again later or choose another region.",
                                region.name()
                            )));
                        }
                        continue;
                    }
                    if let Some(pick) = ranked.into_iter().next() {
                        return Ok((
                            LaunchTargetInput::Job {
                                place_id,
                                job_id: pick.job_id,
                            },
                            Some(pick.note),
                        ));
                    }
                    // Only a squad that did not fit is worth another page.
                    cursor = page.next_cursor;
                    if strategy != JoinStrategy::Squad || cursor.is_none() {
                        break;
                    }
                }
                Err(CoreError::Validation(match strategy {
                    JoinStrategy::Squad => format!(
                        "No public server of this game has room for {needed} accounts right now."
                    ),
                    _ => "No public server of this game can be joined right now.".into(),
                }))
            }
            LaunchTargetInput::Follow { username } => {
                let user = self.social.user_by_name(&username, cancel).await?;
                let account = *accounts
                    .first()
                    .ok_or_else(|| CoreError::Validation("Select at least one account.".into()))?;
                let cookie = self.accounts.secrets(account).await?;
                let user_id: u64 = user.id.parse().map_err(|_| CoreError::Network)?;
                let presence = self
                    .social
                    .presence(Some(&cookie.cookie), &[user_id], cancel)
                    .await?
                    .into_iter()
                    .find(|presence| presence.user_id == user.id);
                match presence {
                    Some(presence) if presence.kind == 2 => match (presence.place_id, presence.job_id) {
                        (Some(place_id), Some(job_id)) => Ok((
                            LaunchTargetInput::Job { place_id, job_id },
                            Some(format!("Following {} into their server.", user.name)),
                        )),
                        _ => Err(CoreError::Validation(format!(
                            "{} is in a game, but Roblox does not share their server with this account. They may have joins turned off, or the account is not their friend.",
                            user.name
                        ))),
                    },
                    _ => Err(CoreError::Validation(format!(
                        "{} is not in a game right now.",
                        user.name
                    ))),
                }
            }
            other => Ok((other, None)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(id: &str, playing: u32, max: u32, ping: Option<u32>) -> ServerRow {
        ServerRow {
            id: id.into(),
            kind: ServerKind::Public,
            playing,
            max_players: max,
            player_tokens: vec![],
            name: None,
            target_id: None,
            joinable: true,
            ping,
            fps: None,
        }
    }

    #[test]
    fn fullest_and_emptiest_pick_by_player_count_and_skip_full_servers() {
        let rows = [
            row("full", 10, 10, None),
            row("tight", 9, 10, None),
            row("quiet", 1, 10, None),
            row("busy", 6, 10, None),
        ];
        let fullest = pick_server(&rows, JoinStrategy::Fullest, 3, &[]).unwrap();
        assert_eq!(fullest.job_id, "tight", "a full server cannot be joined");
        assert!(fullest.note.contains("9 of 10"));
        let emptiest = pick_server(&rows, JoinStrategy::Emptiest, 3, &[]).unwrap();
        assert_eq!(emptiest.job_id, "quiet");
        assert_eq!(
            pick_server(&rows, JoinStrategy::Emptiest, 1, &["quiet".into()])
                .unwrap()
                .job_id,
            "busy"
        );
    }

    #[test]
    fn squad_takes_the_fullest_server_with_room_for_everyone() {
        let rows = [
            row("full", 10, 10, None),
            row("tight", 8, 10, None),
            row("roomy", 3, 10, None),
            row("busy", 6, 10, None),
        ];
        let pick = pick_server(&rows, JoinStrategy::Squad, 3, &[]).unwrap();
        assert_eq!(pick.job_id, "busy", "8/10 has only 2 free slots");
        assert!(pick.note.contains("room for 3"));
        assert!(pick_server(&rows, JoinStrategy::Squad, 8, &[]).is_none());
    }

    #[test]
    fn lowest_ping_falls_back_to_the_emptiest_server() {
        let rows = [
            row("a", 5, 10, Some(90)),
            row("b", 9, 10, Some(40)),
            row("c", 1, 10, None),
        ];
        assert_eq!(
            pick_server(&rows, JoinStrategy::LowestPing, 1, &[])
                .unwrap()
                .job_id,
            "b"
        );
        let no_ping = [row("a", 5, 10, None), row("c", 1, 10, None)];
        assert_eq!(
            pick_server(&no_ping, JoinStrategy::LowestPing, 1, &[])
                .unwrap()
                .job_id,
            "c"
        );
    }

    #[test]
    fn hop_never_returns_the_excluded_or_unjoinable_servers() {
        let mut closed = row("closed", 1, 10, None);
        closed.joinable = false;
        let rows = [
            row("current", 2, 10, None),
            row("other", 3, 10, None),
            closed,
            row("full", 10, 10, None),
        ];
        for _ in 0..20 {
            let pick = pick_server(&rows, JoinStrategy::Hop, 1, &["current".into()]).unwrap();
            assert_eq!(pick.job_id, "other");
        }
        assert!(pick_server(&rows[..1], JoinStrategy::Hop, 1, &["current".into()]).is_none());
    }
}
