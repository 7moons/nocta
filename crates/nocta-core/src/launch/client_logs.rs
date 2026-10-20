//! Roblox's own log files: when a client has joined its game, and why it was disconnected.
//!
//! Every Roblox player writes `%LOCALAPPDATA%\Roblox\logs\<version>_<time>_Player_<id>_last.log`.
//! A client's log is the first new player log that appears after Nocta starts it. Logs grow to
//! gigabytes over long sessions, so only the bytes added since the last read are scanned.
use std::{
    collections::HashSet,
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    time::SystemTime,
};

/// At most this much new log is read per poll; the rest waits for the next one.
const READ_LIMIT: u64 = 4 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LogEvent {
    /// `! Joining game '<job>' place <place> at <ip>`
    Joining { job_id: String, place_id: String },
    /// `Connection accepted from …` or `Replicator created for player …`
    InGame,
    /// `Disconnected from server for reason: <side>: <code> (<Name>)`
    Disconnected { code: u32, name: String },
}

/// Where Roblox writes its logs on this system.
pub fn logs_dir() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA")
        .map(|base| PathBuf::from(base).join("Roblox").join("logs"))
        .filter(|dir| dir.is_dir())
}

pub fn parse_line(line: &str) -> Option<LogEvent> {
    if let Some(rest) = line.split("! Joining game '").nth(1) {
        let (job_id, rest) = rest.split_once('\'')?;
        let place_id = rest
            .split_whitespace()
            .skip_while(|word| *word != "place")
            .nth(1)?
            .to_owned();
        return Some(LogEvent::Joining {
            job_id: job_id.to_owned(),
            place_id,
        });
    }
    if line.contains("[DFLog::NetworkClient] Connection accepted from")
        || line.contains("Replicator created for player")
    {
        return Some(LogEvent::InGame);
    }
    if let Some(reason) = line.split("Disconnected from server for reason: ").nth(1) {
        // "Player: 285 (DisconnectClientInitiated)"
        let after_side = reason.split_once(": ").map_or(reason, |(_, rest)| rest);
        let (code, name) = after_side.split_once(' ').unwrap_or((after_side, ""));
        let code: u32 = code.trim().parse().ok()?;
        let name = name
            .trim()
            .trim_start_matches('(')
            .trim_end_matches(')')
            .to_owned();
        return Some(LogEvent::Disconnected { code, name });
    }
    None
}

/// What a disconnect code means, from Roblox's `ConnectionError` enum.
pub fn describe_disconnect(code: u32) -> &'static str {
    match code {
        264 => "the same account joined from somewhere else",
        265 => "the join ticket was used twice",
        266 => "the connection timed out",
        267 => "the game kicked the player",
        268 => "Roblox flagged unexpected client behavior",
        271 => "the server had no players left",
        273 => "the account was signed in somewhere else",
        274 => "the game's developers shut the server down",
        275 => "Roblox is under maintenance",
        276 => "the server asked the player to rejoin",
        277 => "the connection was lost",
        278 => "the player was idle for 20 minutes",
        279 => "the client could not connect",
        280 => "the client is out of date",
        281 => "a security policy blocked the player",
        282 => "the IP address is blocked",
        284 => "the client failed",
        285 => "the client left (teleport or closing)",
        286 => "the private server's owner removed the player",
        287 => "the game is moderated",
        292 => "the client ran out of memory",
        _ => "an unknown reason",
    }
}

/// Code 285 is the client leaving on purpose (teleports, closing); everything else is a real
/// disconnect worth reporting.
pub fn is_unexpected(code: u32) -> bool {
    code != 285
}

/// The newest player log created at or after `since` that no other client has claimed.
pub fn find_new_log(dir: &Path, since: SystemTime, claimed: &HashSet<PathBuf>) -> Option<PathBuf> {
    let mut found: Vec<(SystemTime, PathBuf)> = std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            let name = path.file_name()?.to_string_lossy().to_string();
            if !name.contains("_Player_")
                || name.contains("CrashHandler")
                || !name.ends_with(".log")
            {
                return None;
            }
            let meta = entry.metadata().ok()?;
            let created = meta.created().or_else(|_| meta.modified()).ok()?;
            (created >= since && !claimed.contains(&path)).then_some((created, path))
        })
        .collect();
    // The oldest unclaimed one belongs to the earliest launch still waiting.
    found.sort();
    found.into_iter().next().map(|(_, path)| path)
}

/// Reads a growing log from where the last read stopped.
#[derive(Debug)]
pub struct LogWatch {
    pub path: PathBuf,
    offset: u64,
    partial: String,
}

impl LogWatch {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            offset: 0,
            partial: String::new(),
        }
    }

    /// Whether everything written so far has been read.
    pub fn caught_up(&self) -> bool {
        std::fs::metadata(&self.path).is_ok_and(|meta| meta.len() <= self.offset)
    }

    /// Events in the lines written since the last poll.
    pub fn poll(&mut self) -> std::io::Result<Vec<LogEvent>> {
        let mut file = File::open(&self.path)?;
        let length = file.metadata()?.len();
        if length < self.offset {
            // Rotated or truncated: start over.
            self.offset = 0;
            self.partial.clear();
        }
        if length == self.offset {
            return Ok(Vec::new());
        }
        file.seek(SeekFrom::Start(self.offset))?;
        let mut bytes = Vec::new();
        file.take(READ_LIMIT).read_to_end(&mut bytes)?;
        self.offset += bytes.len() as u64;
        let text = String::from_utf8_lossy(&bytes);
        let mut events = Vec::new();
        let combined = std::mem::take(&mut self.partial) + &text;
        let mut lines: Vec<&str> = combined.split('\n').collect();
        // The last piece may be a line still being written.
        let last = lines.pop().unwrap_or_default();
        self.partial = last.to_owned();
        for line in lines {
            if let Some(event) = parse_line(line) {
                events.push(event);
            }
        }
        Ok(events)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    // Lines as Roblox 0.741 writes them (copied from a real log, IDs shortened).
    const JOIN: &str = "2026-10-16T15:03:03.639Z,126.639687,4f64,6 [FLog::Output] ! Joining game '02bf9043-b6e2-4e75-960d-cde512773ccb' place 74629631798007 at 10.17.8.212";
    const ACCEPTED: &str = "2026-10-16T15:03:03.732Z,126.732956,4f64,6,Info [DFLog::NetworkClient] Connection accepted from 128.116.21.33|59133";
    const LEFT: &str = "2026-10-16T17:52:48.872Z,10311.872070,3874,6,Info [DFLog::RbxTransportDummyClient] Disconnected from server for reason: Player: 285 (DisconnectClientInitiated)";

    #[test]
    fn reads_joins_and_disconnects_from_real_log_lines() {
        assert_eq!(
            parse_line(JOIN),
            Some(LogEvent::Joining {
                job_id: "02bf9043-b6e2-4e75-960d-cde512773ccb".into(),
                place_id: "74629631798007".into(),
            })
        );
        assert_eq!(parse_line(ACCEPTED), Some(LogEvent::InGame));
        assert_eq!(
            parse_line(LEFT),
            Some(LogEvent::Disconnected {
                code: 285,
                name: "DisconnectClientInitiated".into()
            })
        );
        assert_eq!(
            parse_line("x [DFLog::RbxTransportDummyClient] Disconnected from server for reason: Server: 267 (DisconnectLuaKick)"),
            Some(LogEvent::Disconnected { code: 267, name: "DisconnectLuaKick".into() })
        );
        assert_eq!(parse_line("[FLog::Output] nothing to see"), None);
        assert!(!is_unexpected(285));
        assert!(is_unexpected(277));
        assert_eq!(
            describe_disconnect(278),
            "the player was idle for 20 minutes"
        );
    }

    #[test]
    fn reads_only_new_lines_and_waits_for_half_written_ones() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir
            .path()
            .join("0.741_20261016T150057Z_Player_41A67_last.log");
        let mut file = std::fs::File::create(&path).unwrap();
        writeln!(file, "{JOIN}").unwrap();
        write!(file, "{}", &ACCEPTED[..40]).unwrap();
        file.flush().unwrap();
        let mut watch = LogWatch::new(path.clone());
        assert_eq!(watch.poll().unwrap().len(), 1, "only the finished line");
        writeln!(file, "{}", &ACCEPTED[40..]).unwrap();
        file.flush().unwrap();
        assert_eq!(watch.poll().unwrap(), vec![LogEvent::InGame]);
        assert!(watch.poll().unwrap().is_empty(), "nothing new");
    }

    #[test]
    fn a_client_claims_the_first_new_player_log() {
        let dir = tempfile::tempdir().unwrap();
        let old = dir.path().join("a_Player_OLD_last.log");
        std::fs::write(&old, "").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(50));
        let since = SystemTime::now();
        std::thread::sleep(std::time::Duration::from_millis(50));
        std::fs::write(dir.path().join("b_Player_NEW_CrashHandler_last.log"), "").unwrap();
        let first = dir.path().join("c_Player_ONE_last.log");
        std::fs::write(&first, "").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(50));
        let second = dir.path().join("d_Player_TWO_last.log");
        std::fs::write(&second, "").unwrap();
        let mut claimed = HashSet::new();
        assert_eq!(
            find_new_log(dir.path(), since, &claimed),
            Some(first.clone())
        );
        claimed.insert(first);
        assert_eq!(find_new_log(dir.path(), since, &claimed), Some(second));
    }
}
