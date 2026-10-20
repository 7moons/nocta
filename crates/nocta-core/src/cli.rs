//! Launching from the command line or a shortcut:
//!
//! - `nocta --preset "Morning farm"` runs a launch preset (by name or ID),
//! - `nocta --launch <Place ID or Roblox link> --accounts alice,bob` launches accounts (by
//!   username, alias or Nocta ID).
//!
//! When Nocta is already open, the running window receives the command instead.
use crate::{CoreError, CoreResult};

#[derive(Debug, PartialEq, Eq)]
pub enum CliCommand {
    Preset(String),
    Launch {
        target: String,
        accounts: Vec<String>,
    },
}

/// Reads a command from the program's arguments; other arguments are ignored.
pub fn parse<S: AsRef<str>>(args: &[S]) -> CoreResult<Option<CliCommand>> {
    let args: Vec<&str> = args.iter().map(AsRef::as_ref).collect();
    let value = |flag: &str| -> CoreResult<Option<String>> {
        match args.iter().position(|arg| *arg == flag) {
            None => Ok(None),
            Some(index) => args
                .get(index + 1)
                .filter(|value| !value.starts_with("--") && !value.trim().is_empty())
                .map(|value| Some(value.trim().to_owned()))
                .ok_or_else(|| CoreError::Validation(format!("{flag} needs a value."))),
        }
    };
    if let Some(preset) = value("--preset")? {
        return Ok(Some(CliCommand::Preset(preset)));
    }
    let Some(target) = value("--launch")? else {
        return Ok(None);
    };
    let accounts: Vec<String> = value("--accounts")?
        .ok_or_else(|| {
            CoreError::Validation("--launch needs --accounts with the accounts to launch.".into())
        })?
        .split(',')
        .map(|name| name.trim().to_owned())
        .filter(|name| !name.is_empty())
        .collect();
    Ok(Some(CliCommand::Launch { target, accounts }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_presets_and_launches_and_ignores_other_arguments() {
        assert_eq!(parse(&["nocta.exe"]).unwrap(), None);
        assert_eq!(
            parse(&["nocta.exe", "--minimized", "--preset", "Morning farm"]).unwrap(),
            Some(CliCommand::Preset("Morning farm".into()))
        );
        assert_eq!(
            parse(&["nocta", "--launch", "1818", "--accounts", "alice, Main ,,3"]).unwrap(),
            Some(CliCommand::Launch {
                target: "1818".into(),
                accounts: vec!["alice".into(), "Main".into(), "3".into()],
            })
        );
        assert!(parse(&["nocta", "--launch", "1818"]).is_err());
        assert!(parse(&["nocta", "--preset"]).is_err());
        assert!(parse(&["nocta", "--preset", "--launch"]).is_err());
    }
}
