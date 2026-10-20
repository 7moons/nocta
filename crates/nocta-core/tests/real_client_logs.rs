//! Reads the Roblox logs on this computer the way Nocta follows a running client.
//!
//! `cargo test -p nocta-core --release --test real_client_logs -- --ignored --nocapture`
use nocta_core::launch::client_logs::{logs_dir, LogEvent, LogWatch};

#[test]
#[ignore = "reads this computer's Roblox logs"]
fn real_roblox_logs_show_joins_and_disconnects() {
    let dir = logs_dir().expect("Roblox logs folder");
    let mut joins = 0;
    let mut in_game = 0;
    let mut disconnects = Vec::new();
    let started = std::time::Instant::now();
    for entry in std::fs::read_dir(&dir).unwrap().flatten() {
        let path = entry.path();
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        if !name.contains("_Player_") || name.contains("CrashHandler") {
            continue;
        }
        let mut watch = LogWatch::new(path);
        while !watch.caught_up() {
            for event in watch.poll().unwrap() {
                match event {
                    LogEvent::Joining { .. } => joins += 1,
                    LogEvent::InGame => in_game += 1,
                    LogEvent::Disconnected { code, name } => disconnects.push((code, name)),
                }
            }
        }
    }
    disconnects.sort();
    disconnects.dedup();
    println!(
        "joins {joins}, in game {in_game}, disconnect reasons {disconnects:?}, read in {:.1}s",
        started.elapsed().as_secs_f32()
    );
    assert!(joins > 0 && in_game > 0, "the logs show games being joined");
}
