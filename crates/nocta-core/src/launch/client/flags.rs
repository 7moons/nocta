//! `ClientAppSettings.json`: the fast flags Nocta writes into its own copy before each launch.
use super::*;

impl ClientInstall {
    /// Writes the client's graphics flags before a launch; Roblox reads them when it starts.
    /// Only Nocta's own copy is touched, never the system Roblox install.
    pub fn apply_settings(&self, guid: &str, config: &AppConfig) -> CoreResult<()> {
        validate_guid(guid)?;
        let folder = self.version_dir(guid).join("ClientSettings");
        let file = folder.join("ClientAppSettings.json");
        let flags = client_flags(config);
        if flags.is_empty() {
            match std::fs::remove_file(&file) {
                Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
                    return Err(error.into())
                }
                _ => return Ok(()),
            }
        }
        let json = flags
            .into_iter()
            .map(|(name, value)| (name, Value::String(value)))
            .collect::<serde_json::Map<_, _>>();
        std::fs::create_dir_all(&folder)?;
        std::fs::write(&file, serde_json::to_vec_pretty(&Value::Object(json))?)?;
        Ok(())
    }
}

/// Fast flags for Nocta's client. Since 29 September 2025 Roblox honors only allowlisted flags from
/// `ClientAppSettings.json`, so only those are used (the old FPS-cap flag is ignored by Roblox and
/// is deliberately absent).
pub(super) fn client_flags(config: &AppConfig) -> Vec<(String, String)> {
    let mut flags: Vec<(&str, &str)> = Vec::new();
    // The AFK profile always runs at the lowest graphics.
    if config.client_low_graphics || config.afk_mode {
        flags.extend([
            // Lowest rendering quality level.
            ("DFIntDebugFRMQualityLevelOverride", "1"),
            // Lowest texture quality.
            ("DFFlagTextureQualityOverrideEnabled", "True"),
            ("DFIntTextureQualityOverride", "0"),
            // No grass.
            ("FIntFRMMinGrassDistance", "0"),
            ("FIntFRMMaxGrassDistance", "0"),
            // Lowest-detail meshes at every distance.
            ("DFIntCSGLevelOfDetailSwitchingDistance", "0"),
            ("DFIntCSGLevelOfDetailSwitchingDistanceL12", "0"),
            ("DFIntCSGLevelOfDetailSwitchingDistanceL23", "0"),
            ("DFIntCSGLevelOfDetailSwitchingDistanceL34", "0"),
            // Stop recomputing voxel lighting, which costs CPU in every client.
            ("DFFlagDebugPauseVoxelizer", "True"),
        ]);
    }
    match config.client_graphics_api {
        ClientGraphicsApi::Auto => {}
        ClientGraphicsApi::D3d11 => flags.push(("FFlagDebugGraphicsPreferD3D11", "True")),
        ClientGraphicsApi::Vulkan => flags.push(("FFlagDebugGraphicsPreferVulkan", "True")),
        ClientGraphicsApi::Opengl => flags.push(("FFlagDebugGraphicsPreferOpenGL", "True")),
    }
    let mut flags: Vec<(String, String)> = flags
        .into_iter()
        .map(|(name, value)| (name.to_owned(), value.to_owned()))
        .collect();
    // The user's own flags come last and win over the ones above.
    for flag in &config.fast_flags {
        let Some(value) = super::fast_flags::kind(&flag.name)
            .and_then(|kind| super::fast_flags::normalize(kind, &flag.value))
        else {
            continue;
        };
        flags.retain(|(name, _)| *name != flag.name);
        flags.push((flag.name.clone(), value));
    }
    flags
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::FastFlag;

    #[test]
    fn user_fast_flags_are_added_and_win_over_built_in_ones() {
        let config = AppConfig {
            client_low_graphics: true,
            fast_flags: vec![
                FastFlag {
                    name: "DFIntTextureQualityOverride".into(),
                    value: "2".into(),
                },
                FastFlag {
                    name: "DFFlagDisableDPIScale".into(),
                    value: "true".into(),
                },
            ],
            ..Default::default()
        };
        let flags = client_flags(&config);
        let value = |name: &str| {
            flags
                .iter()
                .filter(|(flag, _)| flag == name)
                .map(|(_, value)| value.as_str())
                .collect::<Vec<_>>()
        };
        assert_eq!(
            value("DFIntTextureQualityOverride"),
            vec!["2"],
            "set once, by the user"
        );
        assert_eq!(value("DFFlagDisableDPIScale"), vec!["True"]);
        assert_eq!(value("DFIntDebugFRMQualityLevelOverride"), vec!["1"]);
    }
}
