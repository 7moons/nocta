//! Running commands given on the command line (see `crate::cli`).
use crate::{
    cli::CliCommand,
    models::{AppConfig, LaunchTargetInput},
    CoreError, CoreResult,
};

impl super::Automation {
    /// Runs a command-line command; returns a sentence for the activity log.
    pub async fn run_cli(&self, command: CliCommand, config: AppConfig) -> CoreResult<String> {
        match command {
            CliCommand::Preset(wanted) => {
                let preset = config
                    .launch_presets
                    .iter()
                    .find(|preset| preset.id == wanted || preset.name.eq_ignore_ascii_case(&wanted))
                    .cloned()
                    .ok_or_else(|| {
                        CoreError::Validation(format!("There is no preset called \"{wanted}\"."))
                    })?;
                let tasks = self.run_preset(&preset, config).await?;
                Ok(format!(
                    "Command line: started preset \"{}\" ({} accounts).",
                    preset.name,
                    tasks.len()
                ))
            }
            CliCommand::Launch { target, accounts } => {
                let all = self.launcher.accounts.list().await?;
                let mut ids = Vec::new();
                for wanted in &accounts {
                    let account = all
                        .iter()
                        .filter(|account| account.deleted_at.is_none())
                        .find(|account| {
                            account.username.eq_ignore_ascii_case(wanted)
                                || (!account.alias.is_empty()
                                    && account.alias.eq_ignore_ascii_case(wanted))
                                || account.id.to_string() == *wanted
                        })
                        .ok_or_else(|| {
                            CoreError::Validation(format!("There is no account \"{wanted}\"."))
                        })?;
                    if !ids.contains(&account.id) {
                        ids.push(account.id);
                    }
                }
                let input = if target.chars().all(|c| c.is_ascii_digit()) {
                    crate::launch::target::validate_place_id(&target)?;
                    LaunchTargetInput::Public {
                        place_id: target.clone(),
                    }
                } else {
                    let url = self.launcher.web.validate_invite_url(&target)?;
                    LaunchTargetInput::Invite {
                        url: url.to_string(),
                    }
                };
                let tasks = self.launcher.start(&ids, input, config).await?;
                Ok(format!("Command line: launching {} accounts.", tasks.len()))
            }
        }
    }
}
