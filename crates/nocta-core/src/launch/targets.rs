//! Turning what the user chose (a place, a server, a link) into a launchable target.
use super::*;

impl Launcher {
    pub(super) fn input_target(input: &LaunchTargetInput) -> Option<LaunchTarget> {
        Some(match input {
            LaunchTargetInput::Configured
            | LaunchTargetInput::Server { .. }
            | LaunchTargetInput::Pick { .. }
            | LaunchTargetInput::Follow { .. } => return None,
            LaunchTargetInput::Public { place_id } => LaunchTarget::Public {
                place_id: place_id.clone(),
            },
            LaunchTargetInput::Job { place_id, job_id } => LaunchTarget::Job {
                place_id: place_id.clone(),
                job_id: job_id.clone(),
            },
            LaunchTargetInput::Private {
                place_id,
                access_code,
                link_code,
            } => LaunchTarget::Private {
                place_id: place_id.clone(),
                access_code: SecretString::new(access_code.as_str()),
                link_code: link_code.as_deref().map(SecretString::new),
            },
            LaunchTargetInput::Invite { url } => LaunchTarget::Invite {
                url: url.clone(),
                configured_place_id: None,
            },
        })
    }

    /// Resolves an input to a launchable target. Invite resolution uses the given account's session.
    pub async fn resolve(
        &self,
        account_id: Option<u32>,
        cookie: Option<&str>,
        input: &LaunchTargetInput,
        config: &AppConfig,
        cancel: &CancellationToken,
    ) -> CoreResult<ResolvedTarget> {
        let configured_place = Some(if config.place_id.trim().is_empty() {
            "920587237"
        } else {
            config.place_id.as_str()
        });
        let invite = match input {
            LaunchTargetInput::Server { target_id } => return self.servers.target(target_id),
            LaunchTargetInput::Configured => config.invite_url.clone(),
            LaunchTargetInput::Invite { url } => url.clone(),
            // Picked and followed servers are settled once per launch, in `start`.
            LaunchTargetInput::Pick { .. } | LaunchTargetInput::Follow { .. } => {
                return Err(CoreError::Validation(
                    "This server is chosen when the launch starts.".into(),
                ))
            }
            other => {
                return Self::input_target(other)
                    .ok_or_else(|| CoreError::Validation("Unsupported launch target.".into()))?
                    .resolve_explicit()?
                    .ok_or_else(|| CoreError::Validation("Unsupported launch target.".into()))
            }
        };
        if invite.trim().is_empty() {
            if matches!(input, LaunchTargetInput::Invite { .. }) {
                return Err(CoreError::Validation(
                    "Enter an invite or game link.".into(),
                ));
            }
            return self
                .resolver
                .resolve(0, "", "", configured_place, cancel)
                .await;
        }
        let parsed = self.web.validate_invite_url(&invite)?;
        let (invite, parsed) = match self.expand_share_link(&parsed, cookie, cancel).await? {
            Some(direct) => {
                let parsed = self.web.validate_invite_url(&direct)?;
                (direct, parsed)
            }
            None => (invite, parsed),
        };
        if invite::extract_private_server_link_code(&invite).is_none()
            && !invite::is_server_share_link(&parsed)
            && !invite::is_experience_share_link(&parsed)
        {
            if let Some(place_id) = invite::extract_place_id_from_url(&parsed) {
                let kind = match invite::extract_job_id_from_url(&parsed) {
                    Some(job_id) => ResolvedKind::Job {
                        job_id: target::validate_job_id(&job_id)?,
                    },
                    None => ResolvedKind::Public,
                };
                return Ok(ResolvedTarget {
                    place_id: target::validate_place_id(&place_id)?,
                    kind,
                    used_fallback: false,
                });
            }
        }
        self.resolver
            .resolve(
                account_id.unwrap_or(0),
                cookie.unwrap_or(""),
                &invite,
                configured_place,
                cancel,
            )
            .await
    }

    /// Turns a share link into a direct game link once and remembers it, so later launches and
    /// other accounts skip the lookup. Roblox only opens share links for signed-in users; any
    /// account will do, so without a launching account the first signed-in one is used.
    /// `Ok(None)` for links that are not share links or that Roblox did not recognise.
    pub async fn expand_share_link(
        &self,
        url: &reqwest::Url,
        cookie: Option<&str>,
        cancel: &CancellationToken,
    ) -> CoreResult<Option<String>> {
        let Some((kind, code)) = web::ShareLinkType::from_url(url) else {
            return Ok(None);
        };
        let key = format!("{kind:?}:{code}");
        if let Some(direct) = self.accounts.share_link(&key).await? {
            return Ok(Some(direct));
        }
        let mut cookies: Vec<SecretString> = cookie
            .filter(|cookie| !cookie.is_empty())
            .map(SecretString::new)
            .into_iter()
            .collect();
        let mut accounts: Vec<Account> = self
            .accounts
            .list()
            .await?
            .into_iter()
            .filter(|account| {
                account.deleted_at.is_none()
                    && account.has_cookie
                    && account.cookie_valid != Some(false)
            })
            .collect();
        // Accounts known to work are tried first.
        accounts.sort_by_key(|account| account.health != AccountHealth::Healthy);
        let mut candidates = accounts.into_iter().take(3);
        loop {
            let cookie = match cookies.pop() {
                Some(cookie) => cookie,
                None => match candidates.next() {
                    Some(account) => SecretString::new(self.accounts.secrets(account.id).await?.cookie.as_str()),
                    None => {
                        return Err(CoreError::Validation(
                            "Roblox only opens share links for signed-in accounts. Add an account with a working cookie first.".into(),
                        ))
                    }
                },
            };
            match self
                .web
                .resolve_share_link(cookie.expose(), &code, kind, cancel)
                .await
            {
                Ok(Some(direct)) => {
                    self.accounts.remember_share_link(&key, &direct).await?;
                    return Ok(Some(direct));
                }
                Ok(None) => return Ok(None),
                // A signed-out account just means trying the next one.
                Err(CoreError::Validation(message)) if message == web::SIGNED_OUT => continue,
                Err(error) => return Err(error),
            }
        }
    }

    pub fn summary(&self, target: &ResolvedTarget) -> LaunchTargetSummary {
        LaunchTargetSummary {
            mode: target.mode(),
            place_id: target.place_id.clone(),
            job_id: target.job_id().map(str::to_owned),
            used_fallback: target.used_fallback,
            target_id: matches!(target.kind, ResolvedKind::Private { .. })
                .then(|| self.servers.register(target.clone())),
        }
    }

    /// IPC `resolve_launch_target`: returns a safe summary; private targets are replaced by an opaque ID.
    pub async fn resolve_summary(
        &self,
        account_id: Option<u32>,
        input: &LaunchTargetInput,
        config: &AppConfig,
    ) -> CoreResult<LaunchTargetSummary> {
        let _tracked = self.tracker.token();
        if self.stop.is_cancelled() {
            return Err(CoreError::Cancelled);
        }
        let cookie = match account_id {
            Some(id) => Some(self.accounts.secrets(id).await?),
            None => None,
        };
        let lease = if let Some(id) = account_id {
            Some(
                self.sessions
                    .tasks
                    .reserve(&[id], TaskKind::LaunchGame)?
                    .remove(0),
            )
        } else {
            None
        };
        let cancel = lease
            .as_ref()
            .map(|(_, _, cancel)| cancel.clone())
            .unwrap_or_default();
        if let Some((id, _, _)) = &lease {
            self.stage(*id, "Resolving launch target.");
        }
        let operation = self.resolve(
            account_id,
            cookie.as_ref().map(|secrets| secrets.cookie.as_str()),
            input,
            config,
            &cancel,
        );
        tokio::pin!(operation);
        let result = tokio::select! {
            result = &mut operation => result,
            _ = self.stop.cancelled() => {
                cancel.cancel();
                let _ = operation.await;
                Err(CoreError::Cancelled)
            }
            _ = tokio::time::sleep(std::time::Duration::from_secs(60)) => {
                cancel.cancel();
                let _ = operation.await;
                Err(CoreError::Timeout)
            }
        };
        if let Some((id, _, _)) = lease {
            let (status, detail) = match &result {
                Ok(_) => (TaskStatus::Succeeded, "Launch target resolved.".into()),
                Err(CoreError::Cancelled) => {
                    (TaskStatus::Cancelled, "Target resolution cancelled.".into())
                }
                Err(error) => (TaskStatus::Failed, error.to_string()),
            };
            self.sessions.finish_task(id, status, detail).await;
        }
        result.map(|target| self.summary(&target))
    }
}
