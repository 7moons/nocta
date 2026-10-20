//! Downloading a version: the package manifest, parallel verified downloads and extraction
//! into a staging folder that becomes the version only when complete.
use super::*;

impl ClientInstall {
    /// Installs a version. Runs in its own task, so a caller that stops waiting (for example a cancelled
    /// launch) does not leave a half-finished install behind. Concurrent calls wait for each other.
    pub async fn install(
        self: &Arc<Self>,
        guid: &str,
        version: Option<String>,
    ) -> CoreResult<InstalledClient> {
        validate_guid(guid)?;
        let this = self.clone();
        let guid = guid.to_owned();
        tokio::spawn(async move {
            let _serial = this.install_lock.lock().await;
            if let Some(existing) = this
                .installed()
                .into_iter()
                .find(|client| client.guid == guid)
            {
                return Ok(existing);
            }
            let cancel = CancellationToken::new();
            {
                let mut state = this.state.lock().unwrap();
                state.cancel = Some(cancel.clone());
                state.last_error = None;
            }
            let result = this.install_inner(&guid, version, &cancel).await;
            let _ = std::fs::remove_dir_all(this.root.join(".staging").join(&guid));
            {
                let mut state = this.state.lock().unwrap();
                state.cancel = None;
                state.progress = None;
                state.last_error = match &result {
                    Ok(_) => None,
                    Err(CoreError::Cancelled) => Some("Installation was cancelled.".into()),
                    Err(error) => Some(error.to_string()),
                };
            }
            this.changed.notify_one();
            result
        })
        .await
        .map_err(|_| {
            CoreError::Validation("The Roblox installation stopped unexpectedly.".into())
        })?
    }

    pub(super) async fn install_inner(
        self: &Arc<Self>,
        guid: &str,
        version: Option<String>,
        cancel: &CancellationToken,
    ) -> CoreResult<InstalledClient> {
        let mut progress = ClientInstallProgress {
            guid: guid.to_owned(),
            phase: "preparing".into(),
            done_mb: 0,
            total_mb: 0,
            packages_done: 0,
            packages_total: 0,
        };
        self.set_progress(Some(progress.clone()));
        let (mirror, manifest) = self.manifest(guid, cancel).await?;
        let packages = parse_manifest(&manifest)?
            .into_iter()
            .filter(|package| PACKAGE_DIRS.iter().any(|(name, _)| *name == package.name))
            .collect::<Vec<_>>();
        if !packages
            .iter()
            .any(|package| package.name == "RobloxApp.zip")
        {
            return Err(CoreError::Validation(
                "This Roblox version has no game client package.".into(),
            ));
        }
        let staging = self.root.join(".staging").join(guid);
        let _ = tokio::fs::remove_dir_all(&staging).await;
        tokio::fs::create_dir_all(staging.join(".downloads")).await?;

        let total: u64 = packages.iter().map(|package| package.packed).sum();
        let size: u64 = packages.iter().map(|package| package.size).sum();
        progress.phase = "downloading".into();
        progress.total_mb = mb(total);
        progress.packages_total = packages.len() as u32;
        self.set_progress(Some(progress.clone()));
        let downloaded = Arc::new(AtomicU64::new(0));
        let finished = Arc::new(AtomicU64::new(0));
        stream::iter(packages.into_iter().map(Ok::<_, CoreError>))
            .try_for_each_concurrent(PARALLEL_DOWNLOADS, |package| {
                let staging = staging.clone();
                let downloaded = downloaded.clone();
                let finished = finished.clone();
                let template = progress.clone();
                async move {
                    let file = self
                        .download(
                            guid,
                            mirror,
                            &package,
                            &staging,
                            &downloaded,
                            &template,
                            cancel,
                        )
                        .await?;
                    let destination = PACKAGE_DIRS
                        .iter()
                        .find(|(name, _)| *name == package.name)
                        .map(|(_, dir)| staging.join(dir))
                        .ok_or(CoreError::NotFound)?;
                    let root = staging.clone();
                    tokio::task::spawn_blocking(move || extract(&file, &root, &destination))
                        .await
                        .map_err(|_| {
                            CoreError::Validation("Extracting a Roblox package failed.".into())
                        })??;
                    let done = finished.fetch_add(1, Ordering::SeqCst) + 1;
                    let mut state = self.state.lock().unwrap();
                    if let Some(progress) = state.progress.as_mut() {
                        progress.packages_done = done as u32;
                    }
                    drop(state);
                    self.changed.notify_one();
                    Ok(())
                }
            })
            .await?;

        progress.phase = "finishing".into();
        progress.done_mb = progress.total_mb;
        progress.packages_done = progress.packages_total;
        self.set_progress(Some(progress));
        let _ = tokio::fs::remove_dir_all(staging.join(".downloads")).await;
        tokio::fs::write(staging.join("AppSettings.xml"), APP_SETTINGS).await?;
        if !staging.join(EXECUTABLE).is_file() {
            return Err(CoreError::Validation(
                "The downloaded Roblox client is missing its executable.".into(),
            ));
        }
        let client = InstalledClient {
            guid: guid.to_owned(),
            version: version.filter(|version| !version.is_empty()),
            installed_at: Utc::now().to_rfc3339(),
            size_mb: mb(size),
        };
        tokio::fs::write(staging.join(MARKER), serde_json::to_vec_pretty(&client)?).await?;
        let target = self.version_dir(guid);
        tokio::fs::create_dir_all(self.versions_dir()).await?;
        if tokio::fs::try_exists(&target).await.unwrap_or(false) {
            // A folder without a marker is an old interrupted copy; replace it.
            tokio::fs::remove_dir_all(&target).await?;
        }
        tokio::fs::rename(&staging, &target).await?;
        Ok(client)
    }

    /// Fetches the manifest from the first mirror that has it; that mirror serves the packages first.
    pub(super) async fn manifest(
        &self,
        guid: &str,
        cancel: &CancellationToken,
    ) -> CoreResult<(usize, String)> {
        let mut failure = CoreError::Network;
        for (index, base) in self.endpoints.cdns.iter().enumerate() {
            if cancel.is_cancelled() {
                return Err(CoreError::Cancelled);
            }
            let request = self
                .http
                .get(format!("{base}/{guid}-rbxPkgManifest.txt"))
                .send();
            let response = tokio::select! {
                _ = cancel.cancelled() => return Err(CoreError::Cancelled),
                response = request => response,
            };
            match response {
                Ok(response) if response.status().is_success() => {
                    if let Ok(text) = response.text().await {
                        return Ok((index, text));
                    }
                }
                Ok(response)
                    if response.status().as_u16() == 403 || response.status().as_u16() == 404 =>
                {
                    failure = CoreError::Validation(format!(
                        "Roblox has no download for {guid}. It may be too old or mistyped."
                    ));
                }
                _ => {}
            }
        }
        Err(failure)
    }

    /// Streams one package to disk while hashing it, retrying on other mirrors.
    #[allow(clippy::too_many_arguments)]
    pub(super) async fn download(
        &self,
        guid: &str,
        mirror: usize,
        package: &Package,
        staging: &Path,
        downloaded: &AtomicU64,
        template: &ClientInstallProgress,
        cancel: &CancellationToken,
    ) -> CoreResult<PathBuf> {
        let path = staging.join(".downloads").join(&package.name);
        let mut failure = CoreError::Network;
        for attempt in 0..DOWNLOAD_ATTEMPTS {
            let base = &self.endpoints.cdns[(mirror + attempt) % self.endpoints.cdns.len()];
            let mut counted = 0u64;
            let result: CoreResult<String> = async {
                let mut response = tokio::select! {
                    _ = cancel.cancelled() => return Err(CoreError::Cancelled),
                    response = self.http.get(format!("{base}/{guid}-{}", package.name)).send() => {
                        response.map_err(|_| CoreError::Network)?
                    }
                };
                if !response.status().is_success() {
                    return Err(CoreError::Validation(format!(
                        "Downloading {} failed with HTTP {}.",
                        package.name,
                        response.status().as_u16()
                    )));
                }
                let mut file = tokio::fs::File::create(&path).await?;
                let mut hasher = Md5::new();
                loop {
                    let chunk = tokio::select! {
                        _ = cancel.cancelled() => return Err(CoreError::Cancelled),
                        chunk = response.chunk() => chunk.map_err(|_| CoreError::Network)?,
                    };
                    let Some(chunk) = chunk else { break };
                    hasher.update(&chunk);
                    file.write_all(&chunk).await?;
                    counted += chunk.len() as u64;
                    let total = downloaded.fetch_add(chunk.len() as u64, Ordering::SeqCst)
                        + chunk.len() as u64;
                    self.report(template, total);
                }
                file.flush().await?;
                Ok(hasher
                    .finalize()
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect())
            }
            .await;
            match result {
                Ok(hash) if hash == package.signature => return Ok(path),
                Ok(_) => {
                    failure = CoreError::Validation(format!(
                        "{} failed its integrity check.",
                        package.name
                    ))
                }
                Err(CoreError::Cancelled) => return Err(CoreError::Cancelled),
                Err(error) => failure = error,
            }
            // Bytes from a failed attempt are taken back out of the progress total.
            downloaded.fetch_sub(counted, Ordering::SeqCst);
        }
        Err(failure)
    }

    /// Updates the progress only when the whole-megabyte figure changes, to keep snapshots quiet.
    pub(super) fn report(&self, template: &ClientInstallProgress, bytes: u64) {
        let done = mb(bytes);
        let mut state = self.state.lock().unwrap();
        match state.progress.as_mut() {
            Some(progress) if progress.done_mb != done => {
                progress.done_mb = done;
                progress.phase = template.phase.clone();
            }
            _ => return,
        }
        drop(state);
        self.changed.notify_one();
    }
}

pub(super) const PARALLEL_DOWNLOADS: usize = 4;

pub(super) const DOWNLOAD_ATTEMPTS: usize = 3;

pub(super) const APP_SETTINGS: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<Settings>\r\n\t<ContentFolder>content</ContentFolder>\r\n\t<BaseUrl>http://www.roblox.com</BaseUrl>\r\n</Settings>\r\n";

/// Where each player package is extracted, relative to the version folder.
/// Unknown manifest entries (for example the installer executable) are skipped.
pub(super) const PACKAGE_DIRS: &[(&str, &str)] = &[
    ("RobloxApp.zip", ""),
    ("Libraries.zip", ""),
    ("redist.zip", ""),
    ("WebView2.zip", ""),
    ("WebView2RuntimeInstaller.zip", "WebView2RuntimeInstaller"),
    ("shaders.zip", "shaders"),
    ("ssl.zip", "ssl"),
    ("content-avatar.zip", "content/avatar"),
    ("content-configs.zip", "content/configs"),
    ("content-fonts.zip", "content/fonts"),
    ("content-sky.zip", "content/sky"),
    ("content-sounds.zip", "content/sounds"),
    ("content-textures2.zip", "content/textures"),
    ("content-models.zip", "content/models"),
    ("content-textures3.zip", "PlatformContent/pc/textures"),
    ("content-terrain.zip", "PlatformContent/pc/terrain"),
    ("content-platform-fonts.zip", "PlatformContent/pc/fonts"),
    (
        "content-platform-dictionaries.zip",
        "PlatformContent/pc/shared_compression_dictionaries",
    ),
    ("extracontent-luapackages.zip", "ExtraContent/LuaPackages"),
    ("extracontent-translations.zip", "ExtraContent/translations"),
    ("extracontent-models.zip", "ExtraContent/models"),
    ("extracontent-textures.zip", "ExtraContent/textures"),
    ("extracontent-places.zip", "ExtraContent/places"),
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Package {
    pub(super) name: String,
    pub(super) signature: String,
    pub(super) packed: u64,
    pub(super) size: u64,
}

pub(super) fn parse_manifest(text: &str) -> CoreResult<Vec<Package>> {
    let invalid =
        || CoreError::Validation("Roblox returned an unreadable package manifest.".into());
    let mut lines = text.lines().map(str::trim);
    if lines.next() != Some("v0") {
        return Err(invalid());
    }
    let mut packages = Vec::new();
    while let Some(name) = lines.next().filter(|name| !name.is_empty()) {
        let signature = lines.next().ok_or_else(invalid)?.to_ascii_lowercase();
        let packed = lines
            .next()
            .and_then(|value| value.parse().ok())
            .ok_or_else(invalid)?;
        let size = lines
            .next()
            .and_then(|value| value.parse().ok())
            .ok_or_else(invalid)?;
        if signature.len() != 32 || !signature.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(invalid());
        }
        packages.push(Package {
            name: name.to_owned(),
            signature,
            packed,
            size,
        });
    }
    Ok(packages)
}

/// Joins a zip entry name onto `base`, rejecting `..` and drive prefixes so an entry can never escape
/// the version folder. Roblox packages use backslashes and often a leading one (`\shaders\x`), which
/// means "relative to the package folder", not the drive root.
pub(super) fn entry_path(base: &Path, name: &str) -> Option<PathBuf> {
    let mut path = base.to_path_buf();
    let mut any = false;
    let relative = name.replace('\\', "/");
    for component in Path::new(relative.trim_start_matches('/')).components() {
        match component {
            Component::Normal(part) => {
                path.push(part);
                any = true;
            }
            Component::CurDir => {}
            _ => return None,
        }
    }
    any.then_some(path)
}

/// Extracts a package into `destination`, refusing entries that would leave `root`.
pub(super) fn extract(file: &Path, root: &Path, destination: &Path) -> CoreResult<()> {
    let invalid = || CoreError::Validation("A Roblox package is not a valid archive.".into());
    let mut archive = zip::ZipArchive::new(std::fs::File::open(file)?).map_err(|_| invalid())?;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(|_| invalid())?;
        let Some(path) = entry_path(destination, entry.name()) else {
            if entry.name().replace('\\', "/").trim_matches('/').is_empty() {
                continue;
            }
            return Err(CoreError::Validation(format!(
                "A Roblox package contains an unsafe path: {}",
                entry.name()
            )));
        };
        if !path.starts_with(root) {
            return Err(invalid());
        }
        if entry.is_dir() {
            std::fs::create_dir_all(&path)?;
            continue;
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut output = std::fs::File::create(&path)?;
        std::io::copy(&mut entry, &mut output)?;
    }
    std::fs::remove_file(file)?;
    Ok(())
}
