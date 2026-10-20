//! Frame-rate cap through Roblox's own in-game setting.
//!
//! Roblox stopped honoring the FPS fast flag in September 2025, but its "Maximum Frame Rate"
//! setting still works. That setting lives in `GlobalBasicSettings_<n>.xml` (as `FramerateCap`,
//! `-1` meaning Roblox's default) and is shared by every client on the computer, including the
//! ones the user starts with Roblox's own launcher. A client reads it only when it starts, so Nocta
//! writes its cap just before starting a client and puts the user's value back as soon as that
//! client has read it: the user's own clients never get Nocta's cap.
use crate::{models::AppConfig, CoreError, CoreResult};
use std::{
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
};

pub const MIN_CAP: u32 = 5;
pub const MAX_CAP: u32 = 1000;

/// Roblox's settings folder, `%LOCALAPPDATA%\Roblox`.
pub fn settings_dir() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA").map(|dir| PathBuf::from(dir).join("Roblox"))
}

/// The newest `GlobalBasicSettings_<n>.xml`; Roblox bumps the number when the format changes.
pub fn settings_file(dir: &Path) -> Option<PathBuf> {
    std::fs::read_dir(dir)
        .ok()?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            let version: u32 = name
                .strip_prefix("GlobalBasicSettings_")?
                .strip_suffix(".xml")?
                .parse()
                .ok()?;
            Some((version, entry.path()))
        })
        .max_by_key(|(version, _)| *version)
        .map(|(_, path)| path)
}

const KEY: &str = r#"<int name="FramerateCap">"#;

pub fn read_cap(xml: &str) -> Option<i32> {
    let start = xml.find(KEY)? + KEY.len();
    let end = start + xml[start..].find("</int>")?;
    xml[start..end].trim().parse().ok()
}

/// The XML with `FramerateCap` set to `cap`, added to the user settings when it is missing.
pub fn write_cap(xml: &str, cap: i32) -> Option<String> {
    if let Some(found) = xml.find(KEY) {
        let start = found + KEY.len();
        let end = start + xml[start..].find("</int>")?;
        return Some(format!("{}{cap}{}", &xml[..start], &xml[end..]));
    }
    let item = xml.find(r#"class="UserGameSettings""#)?;
    let close = item + xml[item..].find("</Properties>")?;
    Some(format!(
        "{}\t{KEY}{cap}</int>\n\t\t{}",
        &xml[..close],
        &xml[close..]
    ))
}

/// What [`hold`] or [`release`] did, for the log.
#[derive(Debug, PartialEq, Eq)]
pub enum Applied {
    Unchanged,
    Set(u32),
    Restored(i32),
}

/// What Nocta remembers while its cap may be in the file: the user's own value and the cap it
/// wrote.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Held {
    original: i32,
    /// `None` in files from older versions, which kept only the user's value while the cap stayed
    /// in the file.
    cap: Option<i32>,
}

impl Held {
    fn wrote(&self, value: Option<i32>) -> bool {
        self.cap.is_none_or(|cap| value == Some(cap))
    }
}

fn read_held(state: &Path) -> Option<Held> {
    let text = std::fs::read_to_string(state).ok()?;
    let mut parts = text.split_whitespace().map(str::parse::<i32>);
    let original = parts.next()?.ok()?;
    let cap = parts.next().and_then(Result::ok);
    Some(Held { original, cap })
}

fn update(file: &Path, xml: &str, cap: i32) -> CoreResult<()> {
    let updated = write_cap(xml, cap).ok_or_else(|| {
        CoreError::Validation("Roblox's settings file has an unknown format.".into())
    })?;
    write_atomically(file, &updated)
}

/// Writes `cap` for a client that is about to start; Roblox reads it once, at start-up. The user's
/// own value is remembered in `state` so [`release`] can put it back as soon as the client has
/// read the cap. Missing Roblox settings (Roblox never started) are not an error.
pub fn hold(settings: &Path, state: &Path, cap: u32) -> CoreResult<Applied> {
    if !(MIN_CAP..=MAX_CAP).contains(&cap) {
        return Err(CoreError::Validation(format!(
            "Use a frame-rate cap between {MIN_CAP} and {MAX_CAP}, or 0 for Roblox's own setting."
        )));
    }
    let Some(file) = settings_file(settings) else {
        return Ok(Applied::Unchanged);
    };
    let xml = std::fs::read_to_string(&file)?;
    let current = read_cap(&xml).unwrap_or(-1);
    // The file holds the user's value unless it still holds the cap Nocta wrote last time.
    let original = match read_held(state) {
        Some(held) if held.wrote(Some(current)) => held.original,
        _ => current,
    };
    if let Some(parent) = state.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(state, format!("{original} {cap}"))?;
    if current == cap as i32 {
        return Ok(Applied::Unchanged);
    }
    update(&file, &xml, cap as i32)?;
    Ok(Applied::Set(cap))
}

/// Puts the user's own value back when the file holds Nocta's cap: after a client has read it,
/// and again after a capped client closes (Roblox saves its settings, cap included, on the way
/// out). Any other value is the user's own change and is kept. `state` stays, for those closes.
pub fn release(settings: &Path, state: &Path) -> CoreResult<Applied> {
    let Some(held) = read_held(state) else {
        return Ok(Applied::Unchanged);
    };
    let file = settings_file(settings);
    let mut applied = Applied::Unchanged;
    if let Some(file) = &file {
        let xml = std::fs::read_to_string(file)?;
        let current = read_cap(&xml);
        if held.wrote(current) && current != Some(held.original) {
            update(file, &xml, held.original)?;
            applied = Applied::Restored(held.original);
        }
    }
    Ok(applied)
}

/// The cap of the clients Nocta starts, in Roblox's shared settings only while one starts.
pub struct FrameRate {
    /// Roblox's settings folder; none in tests, which never touch the real one.
    settings: Mutex<Option<PathBuf>>,
    /// Where the user's own value is remembered.
    state: PathBuf,
    /// Clients started with the cap that have not read it yet.
    holds: Arc<AtomicUsize>,
}

impl FrameRate {
    pub fn new(settings: Option<PathBuf>, state: PathBuf) -> Self {
        Self {
            settings: Mutex::new(settings),
            state,
            holds: Arc::default(),
        }
    }

    /// The cap in effect: the AFK profile's, the user's, or none (0).
    pub fn cap(config: &AppConfig) -> u32 {
        if config.afk_mode {
            config.afk_fps_cap
        } else {
            config.fps_cap
        }
    }

    #[cfg(test)]
    pub(crate) fn use_settings(&self, dir: Option<PathBuf>) {
        *self.settings.lock().unwrap() = dir;
    }

    /// Writes the cap for a client about to start. It stays only while the returned hold lives:
    /// drop it once the client has read the cap.
    pub fn hold(&self, config: &AppConfig) -> CoreResult<Option<Hold>> {
        let cap = Self::cap(config);
        let settings = self.settings.lock().unwrap().clone();
        let Some(settings) = settings.filter(|_| cap > 0) else {
            return Ok(None);
        };
        let held = Hold::new(self.holds.clone(), settings.clone(), self.state.clone());
        hold(&settings, &self.state, cap)?;
        Ok(Some(held))
    }

    /// Puts the user's own value back if Nocta's cap is in the file and no client is about to
    /// read it: after a capped client closed (Roblox saves its settings on the way out), at
    /// start-up, and on every upkeep tick.
    pub fn settle(&self) -> CoreResult<Applied> {
        if self.holds.load(Ordering::SeqCst) > 0 {
            return Ok(Applied::Unchanged);
        }
        let settings = self.settings.lock().unwrap().clone();
        match settings {
            Some(settings) => release(&settings, &self.state),
            None => Ok(Applied::Unchanged),
        }
    }
}

/// Keeps Nocta's cap in the file while a client is starting. When the last hold is dropped, the
/// user's own value goes back.
pub struct Hold {
    pending: Arc<AtomicUsize>,
    settings: PathBuf,
    state: PathBuf,
}

impl Hold {
    /// Counts a start in `pending` before its cap is written, so no release can slip in between.
    pub fn new(pending: Arc<AtomicUsize>, settings: PathBuf, state: PathBuf) -> Self {
        pending.fetch_add(1, Ordering::SeqCst);
        Self {
            pending,
            settings,
            state,
        }
    }
}

impl Drop for Hold {
    fn drop(&mut self) {
        if self.pending.fetch_sub(1, Ordering::SeqCst) == 1 {
            if let Err(error) = release(&self.settings, &self.state) {
                tracing::warn!(%error, "The user's frame-rate cap was not put back");
            }
        }
    }
}

fn write_atomically(file: &Path, text: &str) -> CoreResult<()> {
    let temp = file.with_extension("xml.nocta");
    std::fs::write(&temp, text)?;
    std::fs::rename(&temp, file)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const XML: &str = "<roblox>\n\t<Item class=\"UserGameSettings\" referent=\"x\">\n\t\t<Properties>\n\t\t\t<bool name=\"Fullscreen\">false</bool>\n\t\t\t<int name=\"FramerateCap\">-1</int>\n\t\t</Properties>\n\t</Item>\n</roblox>\n";

    #[test]
    fn reads_and_writes_the_cap_and_adds_it_when_missing() {
        assert_eq!(read_cap(XML), Some(-1));
        let capped = write_cap(XML, 30).unwrap();
        assert_eq!(read_cap(&capped), Some(30));
        assert!(capped.contains("Fullscreen"), "other settings stay");
        let without = XML.replace("\t\t\t<int name=\"FramerateCap\">-1</int>\n", "");
        assert_eq!(read_cap(&without), None);
        assert_eq!(read_cap(&write_cap(&without, 15).unwrap()), Some(15));
        assert!(write_cap("<roblox/>", 15).is_none());
    }

    #[test]
    fn the_cap_is_held_for_a_start_and_the_users_value_comes_back() {
        let dir = tempfile::tempdir().unwrap();
        let settings = dir.path().join("Roblox");
        std::fs::create_dir_all(&settings).unwrap();
        std::fs::write(settings.join("GlobalBasicSettings_12.xml"), XML).unwrap();
        let newest = settings.join("GlobalBasicSettings_13.xml");
        std::fs::write(&newest, XML.replace(">-1<", ">144<")).unwrap();
        let state = dir.path().join("nocta/fps-cap-original");
        let cap = || read_cap(&std::fs::read_to_string(&newest).unwrap());

        assert_eq!(hold(&settings, &state, 30).unwrap(), Applied::Set(30));
        assert_eq!(cap(), Some(30));
        // The next start while the cap is still in the file keeps the user's 144, not Nocta's 30.
        assert_eq!(hold(&settings, &state, 15).unwrap(), Applied::Set(15));
        assert_eq!(release(&settings, &state).unwrap(), Applied::Restored(144));
        assert_eq!(cap(), Some(144));
        // A capped client that closes saves its cap again; it is put back once more.
        std::fs::write(&newest, XML.replace(">-1<", ">15<")).unwrap();
        assert_eq!(release(&settings, &state).unwrap(), Applied::Restored(144));
        // The user changed it in their own client: that value is theirs and is kept.
        std::fs::write(&newest, XML.replace(">-1<", ">240<")).unwrap();
        assert_eq!(release(&settings, &state).unwrap(), Applied::Unchanged);
        assert_eq!(cap(), Some(240));
        assert_eq!(release(&settings, &state).unwrap(), Applied::Unchanged);
        // ...and it is what the next hold remembers.
        hold(&settings, &state, 30).unwrap();
        assert_eq!(release(&settings, &state).unwrap(), Applied::Restored(240));
        // The older file was never touched.
        assert_eq!(
            std::fs::read_to_string(settings.join("GlobalBasicSettings_12.xml")).unwrap(),
            XML
        );
        assert!(hold(&settings, &state, 2).is_err());
        assert!(hold(&settings, &state, 0).is_err());
        // No Roblox settings yet: nothing to do.
        assert_eq!(
            hold(&dir.path().join("missing"), &state, 30).unwrap(),
            Applied::Unchanged
        );
    }

    #[test]
    fn a_state_file_from_an_older_version_still_gives_the_users_value_back() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("GlobalBasicSettings_13.xml"),
            XML.replace(">-1<", ">15<"),
        )
        .unwrap();
        let state = dir.path().join("state");
        // Older versions kept only the user's value and left the cap in the file.
        std::fs::write(&state, "60").unwrap();
        assert_eq!(hold(dir.path(), &state, 15).unwrap(), Applied::Unchanged);
        assert_eq!(release(dir.path(), &state).unwrap(), Applied::Restored(60));
    }

    #[test]
    fn the_users_value_comes_back_when_the_last_start_has_read_the_cap() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("GlobalBasicSettings_13.xml");
        std::fs::write(&file, XML).unwrap();
        let state = dir.path().join("state");
        let pending = Arc::new(AtomicUsize::new(0));
        let start = || Hold::new(pending.clone(), dir.path().into(), state.clone());
        let cap = || read_cap(&std::fs::read_to_string(&file).unwrap());

        let first = start();
        hold(dir.path(), &state, 15).unwrap();
        let second = start();
        hold(dir.path(), &state, 15).unwrap();
        drop(first);
        assert_eq!(cap(), Some(15), "the second client has not read it yet");
        drop(second);
        assert_eq!(cap(), Some(-1));
        assert_eq!(pending.load(Ordering::SeqCst), 0);
    }
}
