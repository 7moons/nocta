//! Launch presets and the scheduler that runs them at set times.
use super::Launcher;

mod command_line;
use crate::{
    models::{AppConfig, LaunchPreset, LaunchTargetInput, PresetTarget, Schedule},
    CoreError, CoreResult,
};
use chrono::{Datelike, NaiveDate, NaiveDateTime, Timelike};
use std::{collections::HashMap, sync::Arc, time::Duration};

pub const MAX_PRESETS: usize = 50;
pub const MAX_SCHEDULES: usize = 100;
const TICK: Duration = Duration::from_secs(20);

/// `HH:MM` as minutes after midnight.
pub fn parse_time(time: &str) -> Option<u32> {
    let (hours, minutes) = time.trim().split_once(':')?;
    let (hours, minutes): (u32, u32) = (hours.parse().ok()?, minutes.parse().ok()?);
    (hours < 24 && minutes < 60 && time.trim().len() == 5).then_some(hours * 60 + minutes)
}

/// Whether `schedule` should run at `now` (local time), given the day it last ran. A schedule
/// runs once per day, at its minute or within a few minutes after (a sleeping computer or a busy
/// tick does not skip it).
pub fn due(schedule: &Schedule, now: NaiveDateTime, last_run: Option<NaiveDate>) -> bool {
    let Some(at) = parse_time(&schedule.time) else {
        return false;
    };
    let today = now.date();
    let weekday = now.weekday().num_days_from_monday() as u8;
    let minute = now.hour() * 60 + now.minute();
    schedule.enabled
        && schedule.days.contains(&weekday)
        && last_run != Some(today)
        && (at..at + 5).contains(&minute)
}

pub fn validate(presets: &[LaunchPreset], schedules: &[Schedule]) -> CoreResult<()> {
    let invalid = |message: &str| Err(CoreError::Validation(message.into()));
    if presets.len() > MAX_PRESETS || schedules.len() > MAX_SCHEDULES {
        return invalid("Keep at most 50 presets and 100 schedules.");
    }
    for preset in presets {
        if preset.name.trim().is_empty() || preset.name.len() > 60 {
            return invalid("Give every preset a name of up to 60 characters.");
        }
        if preset.account_ids.is_empty() || preset.account_ids.len() > 500 {
            return invalid("A preset needs 1 to 500 accounts.");
        }
        match &preset.target {
            PresetTarget::Place { place_id } | PresetTarget::Pick { place_id, .. } => {
                super::target::validate_place_id(place_id)?;
            }
            PresetTarget::Link { url } if url.trim().is_empty() || url.len() > 2048 => {
                return invalid("A preset link cannot be empty.");
            }
            PresetTarget::Follow { username } if username.trim().is_empty() => {
                return invalid("Enter the player a preset follows.");
            }
            _ => {}
        }
    }
    for schedule in schedules {
        if parse_time(&schedule.time).is_none() {
            return invalid("Write schedule times as HH:MM, for example 07:30.");
        }
        if schedule.days.is_empty() || schedule.days.iter().any(|day| *day > 6) {
            return invalid("Pick at least one day for every schedule.");
        }
        if schedule.close_after_minutes > 7 * 24 * 60 {
            return invalid("Close clients at most a week after a scheduled launch.");
        }
        if !presets.iter().any(|preset| preset.id == schedule.preset_id) {
            return invalid("A schedule points to a preset that no longer exists.");
        }
    }
    Ok(())
}

impl PresetTarget {
    pub fn launch_target(&self) -> LaunchTargetInput {
        match self.clone() {
            Self::Place { place_id } => LaunchTargetInput::Public { place_id },
            Self::Link { url } => LaunchTargetInput::Invite { url },
            Self::Pick { place_id, strategy } => LaunchTargetInput::Pick {
                place_id,
                strategy,
                exclude_job: None,
            },
            Self::Follow { username } => LaunchTargetInput::Follow { username },
        }
    }
}

/// Which clients each schedule run started: launch tasks until their client appears, then the
/// client's instance ID.
#[derive(Default)]
pub(crate) struct ScheduleRuns {
    inner: std::sync::Mutex<RunsInner>,
}

#[derive(Default)]
struct RunsInner {
    tasks: HashMap<u32, String>,
    instances: HashMap<String, Vec<String>>,
}

impl ScheduleRuns {
    pub(crate) fn tag(&self, run: &str, tasks: &[u32]) {
        let mut inner = self.inner.lock().unwrap();
        inner.instances.entry(run.to_owned()).or_default();
        for task in tasks {
            inner.tasks.insert(*task, run.to_owned());
        }
    }

    /// Called when a launch task's client appears.
    pub(crate) fn adopt(&self, task: u32, instance: &str) {
        let mut inner = self.inner.lock().unwrap();
        if let Some(run) = inner.tasks.remove(&task) {
            if let Some(list) = inner.instances.get_mut(&run) {
                list.push(instance.to_owned());
            }
        }
    }

    pub(crate) fn run_of(&self, instance: &str) -> Option<String> {
        let inner = self.inner.lock().unwrap();
        inner
            .instances
            .iter()
            .find(|(_, list)| list.iter().any(|item| item == instance))
            .map(|(run, _)| run.clone())
    }

    /// Forgets a run and returns its clients.
    pub(crate) fn take(&self, run: &str) -> Vec<String> {
        let mut inner = self.inner.lock().unwrap();
        inner.tasks.retain(|_, owner| owner != run);
        inner.instances.remove(run).unwrap_or_default()
    }
}

/// Runs presets, schedules and command-line launches.
#[derive(Clone)]
pub struct Automation {
    pub(super) launcher: Arc<Launcher>,
}

impl Automation {
    pub fn new(launcher: Arc<Launcher>) -> Self {
        Self { launcher }
    }

    /// Launches a preset's accounts that still exist. Its AFK switch turns on the AFK profile for
    /// this launch only.
    pub async fn run_preset(
        &self,
        preset: &LaunchPreset,
        mut config: AppConfig,
    ) -> CoreResult<Vec<u32>> {
        let existing = self.launcher.accounts.list().await?;
        let ids: Vec<u32> = preset
            .account_ids
            .iter()
            .copied()
            .filter(|id| {
                existing
                    .iter()
                    .any(|account| account.id == *id && account.deleted_at.is_none())
            })
            .collect();
        if ids.is_empty() {
            return Err(CoreError::Validation(format!(
                "None of the accounts of \"{}\" exist any more.",
                preset.name
            )));
        }
        config.afk_mode |= preset.afk;
        self.launcher
            .start(&ids, preset.target.launch_target(), config)
            .await
    }

    /// Runs a schedule's preset and remembers the clients it starts, so closing them later
    /// leaves clients of the same accounts that were started some other way alone.
    pub async fn start_scheduled(
        &self,
        preset: &LaunchPreset,
        config: AppConfig,
    ) -> CoreResult<(String, Vec<u32>)> {
        let tasks = self.run_preset(preset, config).await?;
        let run = uuid::Uuid::new_v4().to_string();
        self.launcher.runs.tag(&run, &tasks);
        Ok((run, tasks))
    }

    /// Closes the clients a schedule run started (rejoined ones included); returns how many.
    pub async fn close_run(&self, run: &str) -> u32 {
        let owned = self.launcher.runs.take(run);
        let running: Vec<String> = self
            .launcher
            .instances
            .snapshot()
            .into_iter()
            .filter(|instance| {
                owned.contains(&instance.id)
                    && instance.status != crate::models::InstanceStatus::Exited
            })
            .map(|instance| instance.id)
            .collect();
        let mut closed = 0;
        for id in running {
            if self.launcher.instances.close(&id).await.is_ok() {
                closed += 1;
            }
        }
        closed
    }

    /// Checks the schedules every few seconds and runs the ones that are due. Call once.
    pub fn spawn_scheduler(&self) {
        let service = self.clone();
        self.launcher.tracker.spawn(async move {
            let mut last_runs: HashMap<String, NaiveDate> = HashMap::new();
            loop {
                tokio::select! {
                    _ = service.launcher.stop.cancelled() => break,
                    _ = tokio::time::sleep(TICK) => {}
                }
                let Ok(config) = service.launcher.accounts.config().await else {
                    continue;
                };
                let now = chrono::Local::now().naive_local();
                for schedule in &config.schedules {
                    if !due(schedule, now, last_runs.get(&schedule.id).copied()) {
                        continue;
                    }
                    last_runs.insert(schedule.id.clone(), now.date());
                    service.run_schedule(schedule, &config).await;
                }
            }
        });
    }

    async fn run_schedule(&self, schedule: &Schedule, config: &AppConfig) {
        let Some(preset) = config
            .launch_presets
            .iter()
            .find(|preset| preset.id == schedule.preset_id)
            .cloned()
        else {
            return;
        };
        let (run, message) = match self.start_scheduled(&preset, config.clone()).await {
            Ok((run, tasks)) => (
                Some(run),
                format!(
                    "Schedule started \"{}\" ({} accounts).",
                    preset.name,
                    tasks.len()
                ),
            ),
            Err(error) => (
                None,
                format!("Schedule could not start \"{}\": {error}", preset.name),
            ),
        };
        let level = if run.is_some() { "INFO" } else { "ERROR" };
        let _ = self
            .launcher
            .accounts
            .log(level, "SCHEDULE", &message, None)
            .await;
        self.launcher
            .webhook
            .notify(&config.discord_webhook, &message)
            .await;
        let Some(run) = run else {
            return;
        };
        if schedule.close_after_minutes == 0 {
            self.launcher.runs.take(&run);
            return;
        }
        let service = self.clone();
        let minutes = schedule.close_after_minutes;
        self.launcher.tracker.spawn(async move {
            tokio::select! {
                _ = service.launcher.stop.cancelled() => {}
                _ = tokio::time::sleep(Duration::from_secs(minutes as u64 * 60)) => {
                    let closed = service.close_run(&run).await;
                    let message = format!("Schedule closed {closed} clients of \"{}\" after {minutes} min.", preset.name);
                    let _ = service.launcher.accounts.log("INFO", "SCHEDULE", &message, None).await;
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::JoinStrategy;

    fn schedule(time: &str, days: &[u8]) -> Schedule {
        Schedule {
            id: "s".into(),
            preset_id: "p".into(),
            days: days.to_vec(),
            time: time.into(),
            close_after_minutes: 0,
            enabled: true,
        }
    }
    fn at(date: &str, time: &str) -> NaiveDateTime {
        NaiveDateTime::parse_from_str(&format!("{date} {time}"), "%Y-%m-%d %H:%M").unwrap()
    }

    #[test]
    fn schedules_run_once_on_their_days_within_a_few_minutes() {
        // 2026-10-12 is a Monday.
        let monday = schedule("07:30", &[0]);
        assert!(due(&monday, at("2026-10-12", "07:30"), None));
        assert!(
            due(&monday, at("2026-10-12", "07:34"), None),
            "a late tick still runs it"
        );
        assert!(!due(&monday, at("2026-10-12", "07:35"), None));
        assert!(!due(&monday, at("2026-10-12", "07:29"), None));
        assert!(
            !due(&monday, at("2026-10-13", "07:30"), None),
            "Tuesday is not picked"
        );
        let ran = NaiveDate::from_ymd_opt(2026, 10, 12);
        assert!(
            !due(&monday, at("2026-10-12", "07:31"), ran),
            "once per day"
        );
        assert!(
            due(&monday, at("2026-10-19", "07:30"), ran),
            "next week again"
        );
        let mut off = monday.clone();
        off.enabled = false;
        assert!(!due(&off, at("2026-10-12", "07:30"), None));
    }

    #[test]
    fn times_and_presets_are_validated() {
        assert_eq!(parse_time("07:30"), Some(450));
        for bad in ["7:30", "24:00", "12:60", "noon", ""] {
            assert_eq!(parse_time(bad), None, "{bad}");
        }
        let preset = LaunchPreset {
            id: "p".into(),
            name: "Farm".into(),
            account_ids: vec![1, 2],
            target: PresetTarget::Pick {
                place_id: "1818".into(),
                strategy: JoinStrategy::Squad,
            },
            afk: true,
            hotkey: String::new(),
        };
        assert!(validate(std::slice::from_ref(&preset), &[schedule("07:30", &[0, 6])]).is_ok());
        assert!(validate(std::slice::from_ref(&preset), &[schedule("07:30", &[7])]).is_err());
        assert!(
            validate(&[], &[schedule("07:30", &[0])]).is_err(),
            "unknown preset"
        );
        let mut empty = preset;
        empty.account_ids.clear();
        assert!(validate(&[empty], &[]).is_err());
        assert!(matches!(
            PresetTarget::Follow {
                username: "builderman".into()
            }
            .launch_target(),
            LaunchTargetInput::Follow { .. }
        ));
    }
}
