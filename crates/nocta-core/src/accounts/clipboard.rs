//! Copying accounts to the clipboard through user-defined templates such as `{username}:{password}`.
use super::*;

impl Accounts {
    pub async fn copy(&self, ids: &[u32], format: &str) -> CoreResult<String> {
        let accounts = self
            .list()
            .await?
            .into_iter()
            .map(|account| (account.id, account))
            .collect::<std::collections::HashMap<_, _>>();
        // Named formats from earlier versions map onto templates.
        let template = match format {
            "username" => "{username}",
            "password" => "{password}",
            "cookie" => "{cookie}",
            "user:pass" => "{username}:{password}",
            "user:randompass" => "{username}:{random}",
            "user:cookie" => "{username}:{cookie}",
            "user:pass:cookie" => "{username}:{password}:{cookie}",
            "user:random:cookie" => "{username}:{random}:{cookie}",
            template => template,
        };
        let parts = parse_copy_template(template)?;
        let needs_secrets = parts.iter().any(|part| {
            matches!(
                part,
                CopyPart::Key(CopyKey::Password) | CopyPart::Key(CopyKey::Cookie)
            )
        });
        let folders = self
            .folders()
            .await?
            .into_iter()
            .map(|folder| (folder.id, folder.name))
            .collect::<std::collections::HashMap<_, _>>();
        let mut lines = Vec::new();
        let mut seen = HashSet::new();
        for id in ids.iter().filter(|id| seen.insert(**id)) {
            let account = accounts.get(id).ok_or(CoreError::NotFound)?;
            // Secrets are decrypted only when the template actually uses them.
            let secrets = if needs_secrets {
                Some(self.secrets(*id).await?)
            } else {
                None
            };
            let mut line = String::new();
            for part in &parts {
                match part {
                    CopyPart::Text(text) => line.push_str(text),
                    CopyPart::Key(key) => line.push_str(&match key {
                        CopyKey::Username => account.username.clone(),
                        CopyKey::Password => secrets
                            .as_ref()
                            .map(|secrets| secrets.password.to_string())
                            .unwrap_or_default(),
                        CopyKey::Cookie => secrets
                            .as_ref()
                            .map(|secrets| secrets.cookie.to_string())
                            .unwrap_or_default(),
                        CopyKey::Random => rand::thread_rng()
                            .sample_iter(&Alphanumeric)
                            .take(16)
                            .map(char::from)
                            .collect(),
                        CopyKey::UserId => account.user_id.clone().unwrap_or_default(),
                        CopyKey::Id => account.id.to_string(),
                        CopyKey::Folder => account
                            .folder_id
                            .and_then(|folder| folders.get(&folder).cloned())
                            .unwrap_or_else(|| "General space".into()),
                    }),
                }
            }
            lines.push(line);
        }
        Ok(lines.join("\n"))
    }
}

/// Account fields a clipboard template can reference as `{key}`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CopyKey {
    Username,
    Password,
    Cookie,
    Random,
    UserId,
    Id,
    Folder,
}

#[derive(Debug, PartialEq, Eq)]
enum CopyPart {
    Text(String),
    Key(CopyKey),
}

/// Parses a clipboard template such as `{username}:{password}`. `{{` and `}}` are literal braces.
fn parse_copy_template(template: &str) -> CoreResult<Vec<CopyPart>> {
    if template.is_empty() || template.len() > 500 {
        return Err(CoreError::Validation(
            "A clipboard format must be 1-500 characters long.".into(),
        ));
    }
    let mut parts = Vec::new();
    let mut text = String::new();
    let mut chars = template.chars().peekable();
    while let Some(char) = chars.next() {
        match char {
            '{' if chars.peek() == Some(&'{') => {
                chars.next();
                text.push('{');
            }
            '}' if chars.peek() == Some(&'}') => {
                chars.next();
                text.push('}');
            }
            '{' => {
                let name = chars
                    .by_ref()
                    .take_while(|char| *char != '}')
                    .collect::<String>();
                let key = match name.trim().to_ascii_lowercase().as_str() {
                    "username" | "user" => CopyKey::Username,
                    "password" | "pass" => CopyKey::Password,
                    "cookie" => CopyKey::Cookie,
                    "random" => CopyKey::Random,
                    "userid" | "robloxid" => CopyKey::UserId,
                    "id" => CopyKey::Id,
                    "folder" => CopyKey::Folder,
                    _ => {
                        return Err(CoreError::Validation(format!(
                            "Unknown clipboard key {{{name}}}. Use username, password, cookie, random, userId, id or folder."
                        )))
                    }
                };
                if !text.is_empty() {
                    parts.push(CopyPart::Text(std::mem::take(&mut text)));
                }
                parts.push(CopyPart::Key(key));
            }
            '}' => {
                return Err(CoreError::Validation(
                    "A clipboard format has an unmatched }. Write }} for a literal brace.".into(),
                ))
            }
            other => text.push(other),
        }
    }
    if !text.is_empty() {
        parts.push(CopyPart::Text(text));
    }
    Ok(parts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clipboard_templates_parse_keys_text_and_escaped_braces() {
        assert_eq!(
            parse_copy_template("{username}:{PASS} | {{x}}").unwrap(),
            vec![
                CopyPart::Key(CopyKey::Username),
                CopyPart::Text(":".into()),
                CopyPart::Key(CopyKey::Password),
                CopyPart::Text(" | {x}".into()),
            ]
        );
        assert!(parse_copy_template("{nope}").is_err());
        assert!(parse_copy_template("a}b").is_err());
        assert!(parse_copy_template("").is_err());
    }
}
