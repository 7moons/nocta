//! Invite resolution with an account/session-scoped short-lived cache and request coalescing.
use super::target::{validate_job_id, validate_place_id, ResolvedKind, ResolvedTarget};
use crate::roblox::invite::*;
use crate::roblox::web::RobloxWeb;
use crate::{
    browser::{BrowserManager, BrowserOptions, BrowserPurpose},
    secret::SecretString,
    CoreError, CoreResult,
};
use reqwest::Url;
use serde_json::json;
use std::{
    collections::{BTreeSet, HashMap},
    hash::{Hash, Hasher},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::sync::OnceCell;
use tokio_util::sync::CancellationToken;

pub const CACHE_TTL: Duration = Duration::from_secs(60);
pub const BROWSER_TOTAL: Duration = Duration::from_secs(45);
pub const BROWSER_NAVIGATION: Duration = Duration::from_secs(20);
pub const BROWSER_POLL: Duration = Duration::from_millis(750);

#[derive(Clone, PartialEq, Eq, Hash)]
struct CacheKey {
    account_id: u32,
    session: u64,
    invite: String,
    fallback: String,
}
struct CacheEntry {
    cell: Arc<OnceCell<ResolvedTarget>>,
    created: Instant,
}

pub struct InviteResolver {
    pub web: Arc<RobloxWeb>,
    browsers: Option<Arc<BrowserManager>>,
    cache: Mutex<HashMap<CacheKey, CacheEntry>>,
    ttl: Duration,
    #[cfg(test)]
    pub(crate) uncached_calls: std::sync::atomic::AtomicU32,
}

/// Fingerprint of the session cookie so a cookie change invalidates cached resolutions. Never persisted.
fn session_fingerprint(cookie: &str) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    cookie.hash(&mut hasher);
    hasher.finish()
}

impl InviteResolver {
    pub fn new(web: Arc<RobloxWeb>, browsers: Option<Arc<BrowserManager>>) -> Self {
        Self {
            web,
            browsers,
            cache: Mutex::new(HashMap::new()),
            ttl: CACHE_TTL,
            #[cfg(test)]
            uncached_calls: Default::default(),
        }
    }
    #[cfg(test)]
    pub(crate) fn with_ttl(mut self, ttl: Duration) -> Self {
        self.ttl = ttl;
        self
    }
    pub fn invalidate_account(&self, account_id: u32) {
        self.cache
            .lock()
            .unwrap()
            .retain(|key, _| key.account_id != account_id);
    }

    /// Empty invite uses the configured public place. A non-empty invite that cannot be resolved is an error and
    /// never silently becomes a public launch.
    pub async fn resolve(
        &self,
        account_id: u32,
        cookie: &str,
        invite: &str,
        configured_place_id: Option<&str>,
        cancel: &CancellationToken,
    ) -> CoreResult<ResolvedTarget> {
        let invite = invite.trim();
        let fallback = configured_place_id
            .map(str::trim)
            .filter(|place| !place.is_empty())
            .map(validate_place_id)
            .transpose()?;
        if invite.is_empty() {
            let place_id = fallback.ok_or_else(|| {
                CoreError::Validation("Set a Place ID or an invite link before launching.".into())
            })?;
            return Ok(ResolvedTarget {
                place_id,
                kind: ResolvedKind::Public,
                used_fallback: false,
            });
        }
        self.web.validate_invite_url(invite)?;
        let key = CacheKey {
            account_id,
            session: session_fingerprint(cookie),
            invite: invite.to_owned(),
            fallback: fallback.clone().unwrap_or_default(),
        };
        let cell = {
            let mut cache = self.cache.lock().unwrap();
            let ttl = self.ttl;
            cache.retain(|_, entry| entry.created.elapsed() < ttl || !entry.cell.initialized());
            if cache.len() > 512 {
                cache.retain(|_, entry| !entry.cell.initialized());
            }
            cache
                .entry(key)
                .or_insert_with(|| CacheEntry {
                    cell: Arc::new(OnceCell::new()),
                    created: Instant::now(),
                })
                .cell
                .clone()
        };
        // Concurrent callers wait for one resolution; errors are not cached, so the next caller retries.
        let target = cell
            .get_or_try_init(|| async {
                #[cfg(test)]
                self.uncached_calls
                    .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                self.resolve_uncached(account_id, cookie, invite, fallback.as_deref(), cancel)
                    .await
            })
            .await?;
        Ok(target.clone())
    }

    async fn resolve_uncached(
        &self,
        account_id: u32,
        cookie: &str,
        invite: &str,
        fallback: Option<&str>,
        cancel: &CancellationToken,
    ) -> CoreResult<ResolvedTarget> {
        let parsed = self.web.validate_invite_url(invite)?;
        let page = match self
            .web
            .get_page(parsed.clone(), cookie, "https://www.roblox.com/", cancel)
            .await
        {
            Ok(page) => Some(page),
            Err(CoreError::Cancelled) => return Err(CoreError::Cancelled),
            Err(error) => {
                tracing::debug!(%error, "Invite page request failed");
                None
            }
        };
        let final_url = page
            .as_ref()
            .map(|page| page.final_url.clone())
            .unwrap_or_else(|| parsed.clone());
        let html = page.as_ref().map(|page| page.body.as_str()).unwrap_or("");
        let place_from_invite =
            extract_place_id_from_url(&final_url).or_else(|| extract_place_id_from_url(&parsed));
        let mut place = place_from_invite.clone();
        let mut link_code = query(&parsed, "privateServerLinkCode")
            .or_else(|| query(&final_url, "privateServerLinkCode"))
            .map(|code| decode_link_code(&code))
            .filter(|code| !code.is_empty())
            .or_else(|| extract_private_server_link_code(invite));
        let server_share = is_server_share_link(&parsed);
        if link_code.is_none() && server_share {
            let from_http = extract_private_server_link_code(final_url.as_str())
                .or_else(|| extract_private_server_link_code(html))
                .and_then(|code| {
                    extract_place_id_from_url(&final_url)
                        .or_else(|| extract_place_id_from_text(html))
                        .or_else(|| extract_place_id_from_text(final_url.path()))
                        .map(|place| (place, code))
                });
            let resolved = match from_http {
                Some(found) => Some(found),
                None => {
                    self.browser_extract(account_id, cookie, &parsed, cancel, |texts| {
                        let mut place = None;
                        let mut code = None;
                        for text in texts {
                            place = place.or_else(|| place_from_any(text));
                            code = code.or_else(|| extract_private_server_link_code(text));
                            if let (Some(place), Some(code)) = (&place, &code) {
                                return Some((place.clone(), code.clone()));
                            }
                        }
                        None
                    })
                    .await?
                }
            };
            if let Some((share_place, code)) = resolved {
                place = Some(share_place);
                link_code = Some(code);
            }
        }
        let mut job_id = extract_job_id_from_url(&final_url)
            .or_else(|| extract_job_id_from_url(&parsed))
            .or_else(|| extract_job_id_from_text(invite));
        let experience_share = is_experience_share_link(&parsed);
        if experience_share && (place.is_none() || job_id.is_none()) {
            let http_place = extract_place_id_from_url(&final_url)
                .or_else(|| extract_place_id_from_text(html))
                .or_else(|| extract_place_id_from_text(final_url.path()));
            let http_job =
                extract_job_id_from_url(&final_url).or_else(|| extract_job_id_from_text(html));
            let found = if http_place.is_some() || http_job.is_some() {
                Some((http_place, http_job))
            } else {
                self.browser_extract(account_id, cookie, &parsed, cancel, |texts| {
                    let mut place = None;
                    let mut job = None;
                    for text in texts {
                        place = place.or_else(|| place_from_any(text));
                        job = job.or_else(|| extract_job_id_from_text(text));
                        if place.is_some() || job.is_some() {
                            return Some((place.clone(), job.clone()));
                        }
                    }
                    None
                })
                .await?
            };
            if let Some((found_place, found_job)) = found {
                place = found_place.or(place);
                job_id = found_job.or(job_id);
            }
        }
        if experience_share && job_id.is_none() {
            return Err(CoreError::Validation(
                "This experience invite did not expose a server instance (job ID). Public fallback is blocked for this link type.".into(),
            ));
        }
        if link_code.is_none() && server_share {
            return Err(CoreError::Validation(
                "The share link could not be resolved to a private invite code for this account. Use a direct /games/...?privateServerLinkCode=... link.".into(),
            ));
        }
        if let Some(link_code) = link_code {
            let place = place.ok_or_else(|| {
                CoreError::Validation(
                    "The private invite has no Place ID and cannot be joined safely.".into(),
                )
            })?;
            let place = validate_place_id(&place)?;
            if let Some(access_code) = self
                .private_access_code(&place, &link_code, cookie, cancel)
                .await?
            {
                return Ok(ResolvedTarget {
                    place_id: place,
                    kind: ResolvedKind::Private {
                        access_code: SecretString::new(access_code),
                        link_code: Some(SecretString::new(link_code)),
                    },
                    used_fallback: false,
                });
            }
            // An unresolved private link may still carry an explicit server instance; never a public place.
            if let Some(job_id) = job_id {
                return Ok(ResolvedTarget {
                    place_id: place,
                    kind: ResolvedKind::Job {
                        job_id: validate_job_id(&job_id)?,
                    },
                    used_fallback: false,
                });
            }
            return Err(CoreError::Validation(
                "The private invite could not be resolved to an access code for this account. Verify the invite and account permissions.".into(),
            ));
        }
        if let Some(job_id) = job_id {
            let place = place.ok_or_else(|| {
                CoreError::Validation(
                    "The invite includes a server job ID but no Place ID. It cannot be joined safely."
                        .into(),
                )
            })?;
            return Ok(ResolvedTarget {
                place_id: validate_place_id(&place)?,
                kind: ResolvedKind::Job {
                    job_id: validate_job_id(&job_id)?,
                },
                used_fallback: false,
            });
        }
        if let Some(place) = place {
            let place = validate_place_id(&place)?;
            let used_fallback = fallback == Some(place.as_str())
                && place_from_invite.as_deref() != Some(place.as_str());
            return Ok(ResolvedTarget {
                place_id: place,
                kind: ResolvedKind::Public,
                used_fallback,
            });
        }
        Err(CoreError::Validation(
            "The invite link could not be resolved to a join target for this account.".into(),
        ))
    }

    /// Tries `www.roblox.com`, then `web.roblox.com`, and extracts the access code from the returned HTML.
    async fn private_access_code(
        &self,
        place: &str,
        link_code: &str,
        cookie: &str,
        cancel: &CancellationToken,
    ) -> CoreResult<Option<String>> {
        let referer = format!("https://www.roblox.com/games/{place}/");
        for base in [&self.web.endpoints.www, &self.web.endpoints.web] {
            let Ok(mut url) = Url::parse(&format!("{base}/games/{place}")) else {
                continue;
            };
            url.query_pairs_mut()
                .append_pair("privateServerLinkCode", link_code);
            match self.web.get_page(url, cookie, &referer, cancel).await {
                Ok(page) => {
                    if let Some(code) = parse_private_access_code(&page.body) {
                        return Ok(Some(code));
                    }
                }
                Err(CoreError::Cancelled) => return Err(CoreError::Cancelled),
                Err(error) => tracing::debug!(%error, "Private invite candidate failed"),
            }
        }
        Ok(None)
    }

    /// Managed-browser fallback: fresh profile, seeded cookie, request/frame URL observation set up before
    /// navigation, polling every 750 ms, 20 s navigation and 45 s total limits, cleanup in every path.
    async fn browser_extract<T>(
        &self,
        account_id: u32,
        cookie: &str,
        url: &Url,
        cancel: &CancellationToken,
        extract: impl Fn(&[String]) -> Option<T>,
    ) -> CoreResult<Option<T>> {
        if account_id == 0 || cookie.is_empty() {
            return Err(CoreError::Validation(
                "Select an account to resolve this invite link in a managed browser.".into(),
            ));
        }
        let Some(browsers) = &self.browsers else {
            return Ok(None);
        };
        let deadline = tokio::time::Instant::now() + BROWSER_TOTAL;
        let child = cancel.child_token();
        let owned = std::sync::atomic::AtomicBool::new(false);
        let operation = async {
            let browser = browsers
                .start_with_options(
                    account_id,
                    BrowserPurpose::Task,
                    BrowserOptions::default(),
                    &child,
                )
                .await?;
            owned.store(true, std::sync::atomic::Ordering::SeqCst);
            browser.set_roblox_cookie(cookie, &child).await?;
            browser.client.take_events().await;
            let _ =
                tokio::time::timeout(BROWSER_NAVIGATION, browser.navigate(url.as_str(), &child))
                    .await;
            let mut seen = BTreeSet::new();
            loop {
                if child.is_cancelled() {
                    return Err(CoreError::Cancelled);
                }
                let page = browser
                    .evaluate(
                        "[location.href, (document.documentElement ? document.documentElement.outerHTML : '').slice(0, 2097152)]",
                        &child,
                    )
                    .await
                    .unwrap_or(json!([]));
                for event in browser.client.take_events().await {
                    let observed = match event["method"].as_str() {
                        Some("Network.requestWillBeSent") => {
                            event["params"]["request"]["url"].as_str()
                        }
                        Some("Page.frameNavigated") => event["params"]["frame"]["url"].as_str(),
                        _ => None,
                    };
                    if let Some(observed) = observed.filter(|value| value.len() <= 4096) {
                        if seen.len() < 512 {
                            seen.insert(observed.to_owned());
                        }
                    }
                }
                let mut texts = page
                    .as_array()
                    .map(|values| {
                        values
                            .iter()
                            .filter_map(|value| value.as_str().map(str::to_owned))
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                texts.extend(seen.iter().cloned());
                if let Some(found) = extract(&texts) {
                    return Ok(Some(found));
                }
                tokio::select! {
                    _ = child.cancelled() => return Err(CoreError::Cancelled),
                    _ = tokio::time::sleep(BROWSER_POLL) => {}
                }
            }
        };
        tokio::pin!(operation);
        let result = tokio::select! {
            biased;
            result = &mut operation => result,
            _ = tokio::time::sleep_until(deadline) => {
                child.cancel();
                let _ = operation.await;
                Ok(None)
            },
        };
        child.cancel();
        let cleanup = if owned.load(std::sync::atomic::Ordering::SeqCst) {
            browsers.close(account_id).await
        } else {
            Ok(())
        };
        match result {
            Err(CoreError::Cancelled) => Err(CoreError::Cancelled),
            Ok(found) => {
                cleanup?;
                Ok(found)
            }
            Err(error) => {
                cleanup?;
                tracing::debug!(%error, "Browser-assisted invite resolution failed");
                Ok(None)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::roblox::web::fixture::{response, serve};
    use std::sync::atomic::Ordering;

    fn resolver(base: &str) -> InviteResolver {
        InviteResolver::new(Arc::new(RobloxWeb::fixture(base)), None)
    }
    fn ok(body: &str) -> String {
        response("200 OK", &[], body)
    }

    #[tokio::test]
    async fn empty_invite_uses_configured_public_place() {
        let resolver = resolver("http://127.0.0.1:9");
        let target = resolver
            .resolve(1, "c", "  ", Some("0042"), &CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(target.place_id, "42");
        assert!(matches!(target.kind, ResolvedKind::Public));
        assert!(resolver
            .resolve(1, "c", "", None, &CancellationToken::new())
            .await
            .is_err());
        assert!(resolver
            .resolve(
                1,
                "c",
                "https://evil.test/games/1",
                Some("1"),
                &CancellationToken::new()
            )
            .await
            .is_err());
    }

    #[tokio::test]
    async fn private_link_resolves_access_code_via_www_then_web() {
        let (base, server) = serve(vec![
            ok("invite"),
            ok("no code on www"),
            ok("Roblox.GameLauncher.joinPrivateGame(7, 'synthetic-access')"),
        ])
        .await;
        let target = resolver(&base)
            .resolve(
                1,
                "synthetic-cookie",
                &format!("{base}/games/7/x?privateServerLinkCode=link-1"),
                Some("99"),
                &CancellationToken::new(),
            )
            .await
            .unwrap();
        assert_eq!(target.place_id, "7");
        match target.kind {
            ResolvedKind::Private {
                access_code,
                link_code,
            } => {
                assert_eq!(access_code.expose(), "synthetic-access");
                assert_eq!(link_code.unwrap().expose(), "link-1");
            }
            _ => panic!("expected private target"),
        }
        let requests = server.await.unwrap();
        assert!(requests[1].starts_with("GET /games/7?privateServerLinkCode=link-1"));
        assert!(
            requests[1].contains("referer: https://www.roblox.com/games/7/")
                || requests[1].contains("Referer: https://www.roblox.com/games/7/")
        );
    }

    #[tokio::test]
    async fn unresolved_private_link_uses_job_or_fails_but_never_public() {
        let (base, server) = serve(vec![ok(""), ok(""), ok("")]).await;
        let error = resolver(&base)
            .resolve(
                1,
                "c",
                &format!("{base}/games/7?privateServerLinkCode=link"),
                Some("7"),
                &CancellationToken::new(),
            )
            .await
            .err()
            .unwrap();
        assert!(error.to_string().contains("access code"));
        server.await.unwrap();

        let (base, server) = serve(vec![ok(""), ok(""), ok("")]).await;
        let target = resolver(&base)
            .resolve(
                1,
                "c",
                &format!("{base}/games/7?privateServerLinkCode=link&gameInstanceId=job-1"),
                None,
                &CancellationToken::new(),
            )
            .await
            .unwrap();
        assert_eq!(target.job_id(), Some("job-1"));
        server.await.unwrap();
    }

    #[tokio::test]
    async fn share_links_resolve_through_redirects_or_fail_closed() {
        let (base, server) = serve(vec![
            response(
                "302 Found",
                &[("Location", "/games/55/x?privateServerLinkCode=shared")],
                "",
            ),
            ok("landing"),
            ok("Roblox.GameLauncher.joinPrivateGame(55, 'code-55')"),
        ])
        .await;
        let target = resolver(&base)
            .resolve(
                1,
                "c",
                &format!("{base}/share?code=abc&type=Server"),
                Some("1"),
                &CancellationToken::new(),
            )
            .await
            .unwrap();
        assert_eq!(target.place_id, "55");
        assert!(matches!(target.kind, ResolvedKind::Private { .. }));
        server.await.unwrap();

        let (base, server) = serve(vec![ok("nothing useful")]).await;
        let error = resolver(&base)
            .resolve(
                1,
                "c",
                &format!("{base}/share?code=abc&type=Server"),
                Some("1"),
                &CancellationToken::new(),
            )
            .await
            .err()
            .unwrap();
        assert!(error.to_string().contains("share link"));
        server.await.unwrap();

        let (base, server) = serve(vec![ok("<a href=\"/games/8/x\">")]).await;
        let error = resolver(&base)
            .resolve(
                1,
                "c",
                &format!("{base}/share?code=abc&type=ExperienceInvite"),
                Some("1"),
                &CancellationToken::new(),
            )
            .await
            .err()
            .unwrap();
        assert!(error.to_string().contains("job ID"));
        server.await.unwrap();

        let (base, server) = serve(vec![ok("/games/8/x?gameInstanceId=inst-1")]).await;
        let target = resolver(&base)
            .resolve(
                1,
                "c",
                &format!("{base}/share?code=abc&type=ExperienceInvite"),
                None,
                &CancellationToken::new(),
            )
            .await
            .unwrap();
        assert_eq!(target.place_id, "8");
        assert_eq!(target.job_id(), Some("inst-1"));
        server.await.unwrap();
    }

    #[tokio::test]
    async fn public_job_and_unresolvable_invites() {
        let (base, server) = serve(vec![ok("")]).await;
        let target = resolver(&base)
            .resolve(
                1,
                "c",
                &format!("{base}/games/3/Name?jobId=j-2"),
                None,
                &CancellationToken::new(),
            )
            .await
            .unwrap();
        assert_eq!(target.job_id(), Some("j-2"));
        server.await.unwrap();

        let (base, server) = serve(vec![ok("")]).await;
        let target = resolver(&base)
            .resolve(
                1,
                "c",
                &format!("{base}/games/3/Name"),
                Some("3"),
                &CancellationToken::new(),
            )
            .await
            .unwrap();
        assert!(matches!(target.kind, ResolvedKind::Public));
        assert!(!target.used_fallback);
        server.await.unwrap();

        let (base, server) = serve(vec![ok("")]).await;
        assert!(resolver(&base)
            .resolve(
                1,
                "c",
                &format!("{base}/home"),
                Some("3"),
                &CancellationToken::new()
            )
            .await
            .is_err());
        server.await.unwrap();
    }

    #[tokio::test]
    async fn cache_is_scoped_coalesced_and_expires() {
        let (base, server) = serve(vec![ok(""), ok(""), ok("")]).await;
        let resolver = Arc::new(resolver(&base));
        let invite = format!("{base}/games/3/Name");
        let cancel = CancellationToken::new();
        let (first, second) = tokio::join!(
            resolver.resolve(1, "cookie-a", &invite, None, &cancel),
            resolver.resolve(1, "cookie-a", &invite, None, &cancel)
        );
        assert_eq!(first.unwrap().place_id, "3");
        assert_eq!(second.unwrap().place_id, "3");
        assert_eq!(resolver.uncached_calls.load(Ordering::SeqCst), 1);
        // A different session or account resolves independently.
        resolver
            .resolve(1, "cookie-b", &invite, None, &cancel)
            .await
            .unwrap();
        resolver
            .resolve(2, "cookie-a", &invite, None, &cancel)
            .await
            .unwrap();
        assert_eq!(resolver.uncached_calls.load(Ordering::SeqCst), 3);
        server.await.unwrap();

        let (base, server) = serve(vec![ok(""), ok("")]).await;
        let resolver = self::resolver(&base).with_ttl(Duration::from_millis(1));
        let invite = format!("{base}/games/3/Name");
        resolver
            .resolve(1, "c", &invite, None, &cancel)
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_millis(5)).await;
        resolver
            .resolve(1, "c", &invite, None, &cancel)
            .await
            .unwrap();
        assert_eq!(resolver.uncached_calls.load(Ordering::SeqCst), 2);
        server.await.unwrap();

        let stopped = CancellationToken::new();
        stopped.cancel();
        let resolver = self::resolver("http://127.0.0.1:9");
        assert!(matches!(
            resolver
                .resolve(1, "c", "http://127.0.0.1:9/games/1", None, &stopped)
                .await,
            Err(CoreError::Cancelled)
        ));
    }
}
