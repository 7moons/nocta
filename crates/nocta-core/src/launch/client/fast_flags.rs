//! FastFlags the user adds for Nocta's client. Since 29 September 2025 Roblox ignores every flag
//! in `ClientAppSettings.json` that is not on its allowlist, so only those are accepted.
use crate::{models::FastFlag, CoreError, CoreResult};

/// Roblox's allowlist, as published on the Developer Forum ("Allowlist for local client
/// configuration via Fast Flags"), with the kind of value each takes.
pub const ALLOWED: &[(&str, FlagKind)] = &[
    ("DFIntCSGLevelOfDetailSwitchingDistance", FlagKind::Int),
    ("DFIntCSGLevelOfDetailSwitchingDistanceL12", FlagKind::Int),
    ("DFIntCSGLevelOfDetailSwitchingDistanceL23", FlagKind::Int),
    ("DFIntCSGLevelOfDetailSwitchingDistanceL34", FlagKind::Int),
    ("FFlagHandleAltEnterFullscreenManually", FlagKind::Bool),
    ("DFFlagTextureQualityOverrideEnabled", FlagKind::Bool),
    ("DFIntTextureQualityOverride", FlagKind::Int),
    ("FIntDebugForceMSAASamples", FlagKind::Int),
    ("DFFlagDisableDPIScale", FlagKind::Bool),
    ("FFlagDebugGraphicsPreferD3D11", FlagKind::Bool),
    ("FFlagDebugSkyGray", FlagKind::Bool),
    ("DFFlagDebugPauseVoxelizer", FlagKind::Bool),
    ("DFIntDebugFRMQualityLevelOverride", FlagKind::Int),
    ("FIntFRMMaxGrassDistance", FlagKind::Int),
    ("FIntFRMMinGrassDistance", FlagKind::Int),
    ("FFlagDebugGraphicsPreferVulkan", FlagKind::Bool),
    ("FFlagDebugGraphicsPreferOpenGL", FlagKind::Bool),
    ("FIntGrassMovementReducedMotionFactor", FlagKind::Int),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
pub enum FlagKind {
    Bool,
    Int,
}

/// An allowlisted flag, for the settings editor.
#[derive(Clone, Debug, serde::Serialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
pub struct AllowedFlag {
    pub name: String,
    pub kind: FlagKind,
}

pub fn allowed() -> Vec<AllowedFlag> {
    ALLOWED
        .iter()
        .map(|(name, kind)| AllowedFlag {
            name: (*name).to_owned(),
            kind: *kind,
        })
        .collect()
}

/// The value Roblox expects: `True`/`False` or a whole number.
pub fn normalize(kind: FlagKind, value: &str) -> Option<String> {
    let value = value.trim();
    match kind {
        FlagKind::Bool => match value.to_ascii_lowercase().as_str() {
            "true" | "1" => Some("True".into()),
            "false" | "0" => Some("False".into()),
            _ => None,
        },
        FlagKind::Int => value.parse::<i64>().ok().map(|number| number.to_string()),
    }
}

pub fn kind(name: &str) -> Option<FlagKind> {
    ALLOWED
        .iter()
        .find(|(allowed, _)| *allowed == name)
        .map(|(_, kind)| *kind)
}

pub fn validate(flags: &[FastFlag]) -> CoreResult<()> {
    if flags.len() > ALLOWED.len() {
        return Err(CoreError::Validation(
            "Each FastFlag can be set once.".into(),
        ));
    }
    for (index, flag) in flags.iter().enumerate() {
        let kind = kind(&flag.name).ok_or_else(|| {
            CoreError::Validation(format!(
                "Roblox ignores {} (it is not on Roblox's FastFlag allowlist).",
                flag.name.chars().take(80).collect::<String>()
            ))
        })?;
        if normalize(kind, &flag.value).is_none() {
            return Err(CoreError::Validation(format!(
                "{} needs {}.",
                flag.name,
                match kind {
                    FlagKind::Bool => "True or False",
                    FlagKind::Int => "a whole number",
                }
            )));
        }
        if flags[..index].iter().any(|other| other.name == flag.name) {
            return Err(CoreError::Validation(format!(
                "{} is set twice.",
                flag.name
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flag(name: &str, value: &str) -> FastFlag {
        FastFlag {
            name: name.into(),
            value: value.into(),
        }
    }

    #[test]
    fn only_allowlisted_flags_with_matching_values_are_accepted() {
        assert!(validate(&[
            flag("FIntDebugForceMSAASamples", "4"),
            flag("DFFlagDisableDPIScale", "true"),
        ])
        .is_ok());
        let error = validate(&[flag("DFIntTaskSchedulerTargetFps", "999")])
            .unwrap_err()
            .to_string();
        assert!(error.contains("allowlist"), "{error}");
        assert!(validate(&[flag("FIntDebugForceMSAASamples", "lots")]).is_err());
        assert!(validate(&[flag("DFFlagDisableDPIScale", "maybe")]).is_err());
        assert!(validate(&[
            flag("FFlagDebugSkyGray", "True"),
            flag("FFlagDebugSkyGray", "False"),
        ])
        .is_err());
        assert_eq!(normalize(FlagKind::Bool, " TRUE "), Some("True".into()));
        assert_eq!(normalize(FlagKind::Int, "007"), Some("7".into()));
    }
}
