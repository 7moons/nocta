//! Launching through the native handler and claiming the Roblox processes that appear,
//! then following them until they exit.
use super::*;

impl InstanceManager {
    pub(super) fn claimed(&self) -> HashSet<(u32, u64)> {
        self.tracked
            .lock()
            .unwrap()
            .iter()
            .filter(|tracked| tracked.instance.status != InstanceStatus::Exited)
            .flat_map(|tracked| {
                tracked
                    .process
                    .iter()
                    .chain(tracked.launcher.iter())
                    .map(ProcessInfo::key)
            })
            .collect()
    }

    /// Dispatches the URI and claims only processes that appear afterwards. Dispatch and registration are
    /// serialized so concurrent launches cannot claim each other's clients.
    /// With `client`, the URI is passed straight to that game executable (Nocta's own install) instead of the
    /// system `roblox-player` handler.
    pub async fn launch(
        self: &Arc<Self>,
        record: LaunchRecord<'_>,
        uri: crate::secret::SecretString,
        client: Option<&std::path::Path>,
        cancel: &CancellationToken,
    ) -> CoreResult<GameInstance> {
        let platform = self.platform.clone();
        if client.is_none() && !matches!(platform.handler(), Ok(Some(_))) {
            return Err(CoreError::Validation(
                "Roblox is not installed or its roblox-player link handler is not registered. Reinstall Roblox and try again.".into(),
            ));
        }
        let _serial = tokio::select! {
            biased;
            _ = cancel.cancelled() => return Err(CoreError::Cancelled),
            guard = self.dispatch.lock() => guard,
        };
        let enumerate = {
            let platform = platform.clone();
            move || platform.processes()
        };
        let baseline = blocking(enumerate.clone())
            .await?
            .into_iter()
            .map(|process| process.key())
            .collect::<HashSet<_>>();
        if cancel.is_cancelled() {
            return Err(CoreError::Cancelled);
        }
        let dispatched_ms = now_ms();
        match client {
            Some(executable) => {
                let executable = executable.to_path_buf();
                blocking(move || spawn_client(&executable, uri.expose())).await?
            }
            None => {
                blocking({
                    let platform = platform.clone();
                    move || platform.dispatch(uri.expose())
                })
                .await?
            }
        }
        let id = format!("instance-{}", uuid::Uuid::new_v4());
        // An unobserved earlier dispatch cannot be assigned a later launch's client.
        // Observed launchers retain their explicit parent→child handoff.
        for previous in self.tracked.lock().unwrap().iter_mut() {
            if previous.process.is_none() && previous.launcher.is_none() {
                previous.claim_open = false;
            }
        }
        let instance = GameInstance {
            id: id.clone(),
            account_id: record.account_id,
            username: record.username.to_owned(),
            place_id: record.place_id.to_owned(),
            mode: record.mode,
            status: InstanceStatus::Starting,
            pid: None,
            process_name: None,
            started_at: Utc::now().to_rfc3339(),
            exited_at: None,
            detail: "Launch request handed to the system. Waiting for the Roblox client.".into(),
            stage: Default::default(),
            disconnect: None,
            memory_mb: None,
        };
        self.tracked.lock().unwrap().push(Tracked {
            instance,
            handle: None,
            process: None,
            launcher: None,
            dispatched_ms,
            baseline: baseline.clone(),
            claim_open: true,
        });
        self.changed.notify_one();
        let deadline = tokio::time::Instant::now() + self.registration_window;
        loop {
            // Registration keeps going after cancellation of the waiting task would be wrong: the OS already has
            // the URI. Cancellation stops waiting, and the monitor continues the handoff.
            if cancel.is_cancelled() {
                break;
            }
            match blocking(enumerate.clone()).await {
                Ok(processes) => {
                    let claimed = self.claimed();
                    let fresh = processes
                        .into_iter()
                        .filter(|process| {
                            !baseline.contains(&process.key())
                                && !claimed.contains(&process.key())
                                && process.created_ms + CREATION_SLACK_MS >= dispatched_ms
                        })
                        .collect::<Vec<_>>();
                    if self.claim(&id, &fresh) {
                        break;
                    }
                }
                Err(error) => {
                    tracing::debug!(%error, "Process enumeration failed during registration")
                }
            }
            if tokio::time::Instant::now() >= deadline {
                break;
            }
            tokio::select! {
                _ = cancel.cancelled() => break,
                _ = tokio::time::sleep(self.poll) => {}
            }
        }
        let mut tracked = self.tracked.lock().unwrap();
        let entry = tracked
            .iter_mut()
            .find(|tracked| tracked.instance.id == id)
            .ok_or(CoreError::NotFound)?;
        if entry.process.is_none() && entry.launcher.is_none() {
            entry.instance.status = InstanceStatus::Unknown;
            entry.instance.detail =
                "Launch dispatched, but no new Roblox process was observed. The client may still be updating.".into();
        }
        self.changed.notify_one();
        Ok(entry.instance.clone())
    }

    /// Claims a fresh player (preferring a child of an already observed launcher) or records a launcher handoff.
    /// Returns true when a player process is owned.
    pub(super) fn claim(&self, id: &str, fresh: &[ProcessInfo]) -> bool {
        let mut tracked = self.tracked.lock().unwrap();
        let Some(entry) = tracked.iter_mut().find(|tracked| tracked.instance.id == id) else {
            return true;
        };
        if entry.process.is_some() {
            return true;
        }
        let launcher = entry.launcher.clone();
        let mut players = fresh
            .iter()
            .filter(|process| process.player)
            .collect::<Vec<_>>();
        players.sort_by_key(|process| {
            (
                launcher.as_ref().is_none_or(|launcher| {
                    process.parent_pid != launcher.pid || process.created_ms < launcher.created_ms
                }),
                std::cmp::Reverse(process.created_ms),
            )
        });
        for player in players {
            match self.platform.open(player) {
                Ok(handle) => {
                    entry.handle = Some(Arc::from(handle));
                    entry.process = Some(player.clone());
                    entry.instance.pid = Some(player.pid);
                    entry.instance.process_name = Some(player.name.clone());
                    entry.instance.status = InstanceStatus::Running;
                    entry.instance.detail = "Roblox client process is running.".into();
                    self.changed.notify_one();
                    return true;
                }
                Err(error) => tracing::debug!(%error, "Could not open a candidate Roblox process"),
            }
        }
        if entry.launcher.is_none() {
            if let Some(launcher) = fresh
                .iter()
                .filter(|process| !process.player)
                .max_by_key(|process| process.created_ms)
            {
                if let Ok(handle) = self.platform.open(launcher) {
                    entry.handle = Some(Arc::from(handle));
                    entry.launcher = Some(launcher.clone());
                    entry.instance.pid = Some(launcher.pid);
                    entry.instance.process_name = Some(launcher.name.clone());
                    entry.instance.detail =
                        "Roblox launcher is running. Waiting for it to start the game client."
                            .into();
                    self.changed.notify_one();
                }
            }
        }
        false
    }

    /// One monitor pass: updates liveness, finishes launcher→player handoff and prunes old exited records.
    /// Enumeration failures keep the last known state instead of pruning.
    pub async fn poll(self: &Arc<Self>) {
        // Registration and monitor claims share the dispatch lock. A monitor pass must not
        // observe an incomplete baseline or race a second claim of the same process.
        let Ok(_serial) = self.dispatch.try_lock() else {
            return;
        };
        let processes = blocking({
            let platform = self.platform.clone();
            move || platform.processes()
        })
        .await;
        let pending = {
            let tracked = self.tracked.lock().unwrap();
            tracked
                .iter()
                .filter(|tracked| {
                    tracked.process.is_none()
                        && matches!(
                            tracked.instance.status,
                            InstanceStatus::Starting | InstanceStatus::Unknown
                        )
                })
                .map(|tracked| {
                    (
                        tracked.instance.id.clone(),
                        tracked.launcher.clone(),
                        tracked.dispatched_ms,
                        tracked.baseline.clone(),
                        tracked.claim_open,
                    )
                })
                .collect::<Vec<_>>()
        };
        if let Ok(processes) = &processes {
            for (id, launcher, dispatched_ms, baseline, claim_open) in pending {
                // Without a launcher, only a short window after dispatch may be claimed by the monitor.
                if launcher.is_none() && (!claim_open || now_ms() > dispatched_ms + 60_000) {
                    continue;
                }
                let claimed = self.claimed();
                let fresh = processes
                    .iter()
                    .filter(|process| {
                        !claimed.contains(&process.key())
                            && !baseline.contains(&process.key())
                            && process.player
                            && match &launcher {
                                Some(launcher) => {
                                    process.parent_pid == launcher.pid
                                        && process.created_ms >= launcher.created_ms
                                }
                                None => process.created_ms + CREATION_SLACK_MS >= dispatched_ms,
                            }
                    })
                    .cloned()
                    .collect::<Vec<_>>();
                if !fresh.is_empty() {
                    self.claim(&id, &fresh);
                }
            }
        }
        let mut changed = false;
        let mut exits = Vec::new();
        {
            let closing = self.closing.lock().unwrap().clone();
            let mut tracked = self.tracked.lock().unwrap();
            for entry in tracked.iter_mut() {
                if entry.instance.status == InstanceStatus::Exited {
                    continue;
                }
                let Some(handle) = &entry.handle else {
                    continue;
                };
                match handle.alive() {
                    Ok(true) => {}
                    Ok(false) if entry.process.is_none() && entry.launcher.is_some() => {
                        // The launcher exited; keep waiting briefly for its player child.
                        entry.handle = None;
                        entry.instance.pid = None;
                        if now_ms() > entry.dispatched_ms + 120_000 {
                            entry.instance.status = InstanceStatus::Unknown;
                            entry.instance.detail =
                                "The Roblox launcher exited without a tracked game client.".into();
                            entry.launcher = None;
                        }
                        changed = true;
                    }
                    Ok(false) => {
                        entry.instance.status = InstanceStatus::Exited;
                        entry.instance.exited_at = Some(Utc::now().to_rfc3339());
                        entry.instance.detail = "The game client exited.".into();
                        entry.handle = None;
                        changed = true;
                        if !closing.contains(&entry.instance.id) {
                            exits.push(super::ExitEvent {
                                instance_id: entry.instance.id.clone(),
                                account_id: entry.instance.account_id,
                                username: entry.instance.username.clone(),
                                place_id: entry.instance.place_id.clone(),
                            });
                        }
                    }
                    Err(error) => tracing::debug!(%error, "Instance liveness check failed"),
                }
            }
            let exited = tracked
                .iter()
                .filter(|entry| entry.instance.status == InstanceStatus::Exited)
                .count();
            if exited > KEEP_EXITED {
                let mut remove = exited - KEEP_EXITED;
                tracked.retain(|entry| {
                    if remove > 0 && entry.instance.status == InstanceStatus::Exited {
                        remove -= 1;
                        false
                    } else {
                        true
                    }
                });
            }
        }
        if let Ok(processes) = processes {
            let claimed = self.claimed();
            let external = processes
                .into_iter()
                .filter(|process| process.player && !claimed.contains(&process.key()))
                .collect::<Vec<_>>();
            let mut current = self.external.lock().unwrap();
            if *current != external {
                *current = external;
                changed = true;
            }
        }
        for exit in exits {
            self.report_exit(exit);
        }
        if changed {
            self.changed.notify_one();
        }
    }
}
