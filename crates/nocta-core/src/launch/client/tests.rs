use super::install::{entry_path, parse_manifest};
use super::*;
use std::{collections::HashMap, io::Write};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

/// Serves fixed bodies by path, in any order and concurrently; records requested paths.
async fn serve(routes: HashMap<String, (u16, Vec<u8>)>) -> (String, Arc<Mutex<Vec<String>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let seen = Arc::new(Mutex::new(Vec::new()));
    let log = seen.clone();
    let routes = Arc::new(routes);
    tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                break;
            };
            let routes = routes.clone();
            let log = log.clone();
            tokio::spawn(async move {
                let mut buffer = vec![0u8; 8192];
                let mut read = 0;
                while !buffer[..read].windows(4).any(|part| part == b"\r\n\r\n") {
                    match socket.read(&mut buffer[read..]).await {
                        Ok(0) | Err(_) => return,
                        Ok(count) => read += count,
                    }
                }
                let request = String::from_utf8_lossy(&buffer[..read]);
                let path = request.split_whitespace().nth(1).unwrap_or("/").to_owned();
                log.lock().unwrap().push(path.clone());
                let (status, body) = routes.get(&path).cloned().unwrap_or((404, Vec::new()));
                let head = format!(
                    "HTTP/1.1 {status} X\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = socket.write_all(head.as_bytes()).await;
                let _ = socket.write_all(&body).await;
            });
        }
    });
    (base, seen)
}

fn zip_bytes(files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (name, data) in files {
        writer
            .start_file(*name, zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(data).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

fn md5_hex(bytes: &[u8]) -> String {
    Md5::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

const GUID: &str = "version-0123456789abcdef";

fn fixture(app: &[u8], corrupt_ssl: bool) -> HashMap<String, (u16, Vec<u8>)> {
    let ssl = zip_bytes(&[("cacert.pem", b"cert")]);
    let manifest =
        format!(
        "v0\nRobloxApp.zip\n{}\n{}\n1\nssl.zip\n{}\n{}\n1\nRobloxPlayerInstaller.exe\n{}\n3\n3\n",
        md5_hex(app),
        app.len(),
        if corrupt_ssl { "0".repeat(32) } else { md5_hex(&ssl) },
        ssl.len(),
        md5_hex(b"exe"),
    );
    HashMap::from([
        (
            "/v".to_owned(),
            (
                200,
                format!(r#"{{"version":"0.1.0.1","clientVersionUpload":"{GUID}"}}"#).into_bytes(),
            ),
        ),
        (
            format!("/{GUID}-rbxPkgManifest.txt"),
            (200, manifest.into_bytes()),
        ),
        (format!("/{GUID}-RobloxApp.zip"), (200, app.to_vec())),
        (format!("/{GUID}-ssl.zip"), (200, ssl)),
    ])
}

fn install_at(root: &Path, base: &str) -> Arc<ClientInstall> {
    Arc::new(
        ClientInstall::with_endpoints(
            root.to_path_buf(),
            Arc::new(Notify::new()),
            Endpoints {
                versions: vec![format!("{base}/v")],
                cdns: vec![base.to_owned()],
            },
        )
        .unwrap(),
    )
}

#[test]
fn manifest_guid_and_paths_are_validated() {
    let parsed = parse_manifest("v0\nA.zip\n0123456789abcdef0123456789ABCDEF\n10\n20\n").unwrap();
    assert_eq!(parsed[0].signature, "0123456789abcdef0123456789abcdef");
    assert!(parse_manifest("v1\n").is_err());
    assert!(parse_manifest("v0\nA.zip\nnot-a-hash\n1\n1\n").is_err());
    assert!(validate_guid(GUID).is_ok());
    assert!(validate_guid("version-0123456789ABCDEF").is_err());
    assert!(validate_guid("version-../../evil0000").is_err());
    let base = Path::new("C:/root/v");
    assert_eq!(
        entry_path(base, "content\\fonts\\a.ttf"),
        Some(base.join("content").join("fonts").join("a.ttf"))
    );
    assert_eq!(
        entry_path(base, "\\CrossExpVoicePatchConfig\\a.json"),
        Some(base.join("CrossExpVoicePatchConfig").join("a.json")),
        "a leading backslash is relative, as in real Roblox packages"
    );
    assert_eq!(entry_path(base, "\\"), None);
    assert_eq!(entry_path(base, "..\\..\\evil.exe"), None);
    assert_eq!(entry_path(base, "a\\..\\..\\evil.exe"), None);
    assert_eq!(entry_path(base, "C:\\Windows\\evil.exe"), None);
}

#[tokio::test]
async fn installs_verified_packages_atomically_and_manages_versions() {
    // Real Roblox packages name entries with a leading backslash.
    let app = zip_bytes(&[(EXECUTABLE, b"player"), ("\\lib\\x.dll", b"dll")]);
    let (base, seen) = serve(fixture(&app, false)).await;
    let root = tempfile::tempdir().unwrap();
    let install = install_at(root.path(), &base);

    let latest = install.latest(false).await.unwrap();
    assert_eq!(
        (latest.guid.as_str(), latest.version.as_str()),
        (GUID, "0.1.0.1")
    );
    install.latest(false).await.unwrap();
    assert_eq!(
        seen.lock()
            .unwrap()
            .iter()
            .filter(|path| *path == "/v")
            .count(),
        1,
        "the version check is cached"
    );

    let client = install.install(GUID, Some(latest.version)).await.unwrap();
    let dir = install.version_dir(GUID);
    assert_eq!(std::fs::read(dir.join(EXECUTABLE)).unwrap(), b"player");
    assert_eq!(std::fs::read(dir.join("lib/x.dll")).unwrap(), b"dll");
    assert_eq!(std::fs::read(dir.join("ssl/cacert.pem")).unwrap(), b"cert");
    assert!(dir.join("AppSettings.xml").is_file());
    assert!(!dir.join(".downloads").exists());
    assert!(!root.path().join(".staging").join(GUID).exists());
    assert!(
        !seen
            .lock()
            .unwrap()
            .iter()
            .any(|path| path.contains("Installer")),
        "unmapped manifest entries are skipped"
    );
    assert_eq!(install.installed(), vec![client.clone()]);

    let mut config = AppConfig {
        managed_client: true,
        ..AppConfig::default()
    };
    let executable = install
        .ready(&config, &CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(executable, dir.join(EXECUTABLE));

    // Graphics flags are written into Nocta's copy only, and removed when turned off.
    let flags_file = dir.join("ClientSettings").join("ClientAppSettings.json");
    assert!(!flags_file.exists());
    let graphics = AppConfig {
        client_low_graphics: true,
        client_graphics_api: ClientGraphicsApi::Vulkan,
        ..config.clone()
    };
    install
        .ready(&graphics, &CancellationToken::new())
        .await
        .unwrap();
    let written: Value = serde_json::from_slice(&std::fs::read(&flags_file).unwrap()).unwrap();
    assert_eq!(written["DFIntDebugFRMQualityLevelOverride"], "1");
    assert_eq!(written["FFlagDebugGraphicsPreferVulkan"], "True");
    assert!(written.get("DFIntTaskSchedulerTargetFps").is_none());
    install
        .ready(&config, &CancellationToken::new())
        .await
        .unwrap();
    assert!(!flags_file.exists());

    // The active version is protected, and so is one with a running client.
    assert!(install.remove(GUID, &config, &[]).is_err());
    let other = install.version_dir("version-aaaaaaaaaaaaaaaa");
    std::fs::create_dir_all(&other).unwrap();
    std::fs::write(other.join(EXECUTABLE), b"old").unwrap();
    std::fs::write(
        other.join(MARKER),
        serde_json::to_vec(&InstalledClient {
            guid: "version-aaaaaaaaaaaaaaaa".into(),
            version: None,
            installed_at: "2000-01-01T00:00:00Z".into(),
            size_mb: 1,
        })
        .unwrap(),
    )
    .unwrap();
    let running = vec![other.join(EXECUTABLE).to_string_lossy().into_owned()];
    assert!(install
        .remove("version-aaaaaaaaaaaaaaaa", &config, &running)
        .is_err());
    config.client_keep_versions = 0;
    assert_eq!(install.cleanup(&config, &running), 0);
    assert_eq!(install.cleanup(&config, &[]), 1);
    assert_eq!(install.installed().len(), 1);
}

#[tokio::test]
async fn corrupt_download_fails_without_leaving_a_version() {
    let app = zip_bytes(&[(EXECUTABLE, b"player")]);
    let (base, _) = serve(fixture(&app, true)).await;
    let root = tempfile::tempdir().unwrap();
    let install = install_at(root.path(), &base);
    let error = install.install(GUID, None).await.unwrap_err();
    assert!(error.to_string().contains("integrity"), "{error}");
    assert!(install.installed().is_empty());
    assert!(!install.version_dir(GUID).exists());
    assert!(install
        .snapshot(&AppConfig::default())
        .last_error
        .unwrap()
        .contains("integrity"));
    let config = AppConfig {
        client_update_mode: ClientUpdateMode::Manual,
        ..AppConfig::default()
    };
    assert!(install
        .ready(&config, &CancellationToken::new())
        .await
        .is_err());
}
