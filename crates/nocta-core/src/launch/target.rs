use crate::{models::LaunchMode, secret::SecretString, CoreError, CoreResult};

/// Backend launch intent. Private access codes stay inside this type and the opaque target registry.
#[derive(Clone, Debug)]
pub enum LaunchTarget {
    Public {
        place_id: String,
    },
    Job {
        place_id: String,
        job_id: String,
    },
    Private {
        place_id: String,
        access_code: SecretString,
        link_code: Option<SecretString>,
    },
    Invite {
        url: String,
        configured_place_id: Option<String>,
    },
}

/// A target that can be turned into a PlaceLauncher request without further network resolution.
#[derive(Clone, Debug)]
pub struct ResolvedTarget {
    pub place_id: String,
    pub kind: ResolvedKind,
    pub used_fallback: bool,
}

#[derive(Clone, Debug)]
pub enum ResolvedKind {
    Public,
    Job {
        job_id: String,
    },
    Private {
        access_code: SecretString,
        link_code: Option<SecretString>,
    },
}

impl ResolvedTarget {
    pub fn mode(&self) -> LaunchMode {
        match self.kind {
            ResolvedKind::Public => LaunchMode::Public,
            ResolvedKind::Job { .. } => LaunchMode::Job,
            ResolvedKind::Private { .. } => LaunchMode::Private,
        }
    }
    pub fn job_id(&self) -> Option<&str> {
        match &self.kind {
            ResolvedKind::Job { job_id } => Some(job_id),
            _ => None,
        }
    }
}

impl LaunchTarget {
    /// Validates explicit targets. Invalid private/job targets are errors and never become public launches.
    pub fn resolve_explicit(&self) -> CoreResult<Option<ResolvedTarget>> {
        Ok(Some(match self {
            Self::Public { place_id } => ResolvedTarget {
                place_id: validate_place_id(place_id)?,
                kind: ResolvedKind::Public,
                used_fallback: false,
            },
            Self::Job { place_id, job_id } => ResolvedTarget {
                place_id: validate_place_id(place_id)?,
                kind: ResolvedKind::Job {
                    job_id: validate_job_id(job_id)?,
                },
                used_fallback: false,
            },
            Self::Private {
                place_id,
                access_code,
                link_code,
            } => {
                let place_id = validate_place_id(place_id)?;
                let access_code = validate_code(access_code.expose()).ok_or_else(|| {
                    CoreError::Validation(
                        "A private server target needs a valid access code.".into(),
                    )
                })?;
                let link_code = match link_code {
                    Some(code) if !code.expose().trim().is_empty() => {
                        Some(validate_code(code.expose()).ok_or_else(|| {
                            CoreError::Validation("The private server link code is invalid.".into())
                        })?)
                    }
                    _ => None,
                };
                ResolvedTarget {
                    place_id,
                    kind: ResolvedKind::Private {
                        access_code,
                        link_code,
                    },
                    used_fallback: false,
                }
            }
            Self::Invite { .. } => return Ok(None),
        }))
    }
}

pub fn validate_place_id(value: &str) -> CoreResult<String> {
    let value = value.trim();
    if value.is_empty()
        || value.len() > 19
        || !value.bytes().all(|byte| byte.is_ascii_digit())
        || value.parse::<u64>().map_or(true, |id| id == 0)
    {
        return Err(CoreError::Validation(
            "Enter a valid positive numeric Place ID.".into(),
        ));
    }
    // Normalise leading zeroes so caches and launch URLs use one canonical decimal form.
    Ok(value.parse::<u64>().unwrap().to_string())
}

pub fn validate_job_id(value: &str) -> CoreResult<String> {
    let value = value.trim();
    if value.is_empty()
        || value.len() > 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    {
        return Err(CoreError::Validation(
            "A server job target needs a valid job ID.".into(),
        ));
    }
    Ok(value.to_owned())
}

fn validate_code(value: &str) -> Option<SecretString> {
    let value = value.trim();
    (!value.is_empty()
        && value.len() <= 256
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_'))
    .then(|| SecretString::new(value))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn place_ids_are_positive_decimal_strings_without_precision_loss() {
        assert_eq!(
            validate_place_id(" 9007199254740993 ").unwrap(),
            "9007199254740993"
        );
        assert_eq!(validate_place_id("0012").unwrap(), "12");
        for invalid in [
            "",
            "0",
            "000",
            "-1",
            "1e3",
            "12a",
            "１２",
            "99999999999999999999",
        ] {
            assert!(validate_place_id(invalid).is_err(), "{invalid}");
        }
    }

    #[test]
    fn explicit_targets_never_downgrade_to_public() {
        let job = LaunchTarget::Job {
            place_id: "1".into(),
            job_id: " ".into(),
        };
        assert!(job.resolve_explicit().is_err());
        let private = LaunchTarget::Private {
            place_id: "1".into(),
            access_code: SecretString::new(""),
            link_code: None,
        };
        assert!(private.resolve_explicit().is_err());
        let error = LaunchTarget::Private {
            place_id: "1".into(),
            access_code: SecretString::new("bad code with spaces"),
            link_code: None,
        }
        .resolve_explicit()
        .unwrap_err()
        .to_string();
        assert!(!error.contains("bad code"));
        let resolved = LaunchTarget::Private {
            place_id: "5".into(),
            access_code: SecretString::new(" abc-123 "),
            link_code: Some(SecretString::new("")),
        }
        .resolve_explicit()
        .unwrap()
        .unwrap();
        assert_eq!(resolved.mode(), LaunchMode::Private);
        match resolved.kind {
            ResolvedKind::Private {
                access_code,
                link_code,
            } => {
                assert_eq!(access_code.expose(), "abc-123");
                assert!(link_code.is_none());
            }
            _ => panic!("expected private"),
        }
        assert!(LaunchTarget::Invite {
            url: "https://www.roblox.com/games/1".into(),
            configured_place_id: None
        }
        .resolve_explicit()
        .unwrap()
        .is_none());
    }
}
