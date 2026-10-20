//! Exporting accounts with their credentials as CSV or JSON, for other tools or spreadsheets.
//! Like copying, this is an explicit request that reveals secrets; backups stay the way to move
//! accounts between Nocta installs.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
pub enum ExportFormat {
    Csv,
    Json,
}

const COLUMNS: [&str; 7] = [
    "username", "password", "cookie", "userId", "alias", "note", "folder",
];

/// A CSV field, quoted when it holds a separator, quote or line break. Fields that start like a
/// formula get a leading apostrophe so spreadsheets show them as text.
fn csv_field(value: &str) -> String {
    let value = if value.starts_with(['=', '+', '-', '@']) {
        format!("'{value}")
    } else {
        value.to_owned()
    };
    if value.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value
    }
}

impl Accounts {
    pub async fn export(&self, ids: &[u32], format: ExportFormat) -> CoreResult<Zeroizing<String>> {
        if ids.is_empty() {
            return Err(CoreError::Validation("Select accounts to export.".into()));
        }
        let accounts = self.list().await?;
        let folders = self.folders().await?;
        let mut rows = Vec::new();
        for id in ids {
            let account = accounts
                .iter()
                .find(|account| account.id == *id)
                .ok_or(CoreError::NotFound)?;
            let secrets = self.secrets(*id).await?;
            let folder = account
                .folder_id
                .and_then(|folder| folders.iter().find(|item| item.id == folder))
                .map(|folder| folder.name.clone())
                .unwrap_or_default();
            rows.push([
                account.username.clone(),
                secrets.password.clone(),
                secrets.cookie.clone(),
                account.user_id.clone().unwrap_or_default(),
                account.alias.clone(),
                account.note.clone(),
                folder,
            ]);
        }
        Ok(Zeroizing::new(match format {
            ExportFormat::Csv => {
                let mut text = COLUMNS.join(",");
                for row in &rows {
                    text.push_str("\r\n");
                    text.push_str(
                        &row.iter()
                            .map(|value| csv_field(value))
                            .collect::<Vec<_>>()
                            .join(","),
                    );
                }
                text.push_str("\r\n");
                text
            }
            ExportFormat::Json => {
                let items: Vec<serde_json::Value> = rows
                    .iter()
                    .map(|row| {
                        serde_json::Value::Object(
                            COLUMNS
                                .iter()
                                .zip(row)
                                .map(|(key, value)| {
                                    ((*key).to_owned(), serde_json::Value::String(value.clone()))
                                })
                                .collect(),
                        )
                    })
                    .collect();
                serde_json::to_string_pretty(&items)?
            }
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_fields_are_quoted_and_formulas_neutralized() {
        assert_eq!(csv_field("plain"), "plain");
        assert_eq!(csv_field("a,b"), "\"a,b\"");
        assert_eq!(csv_field("say \"hi\""), "\"say \"\"hi\"\"\"");
        assert_eq!(csv_field("=cmd()"), "'=cmd()");
        assert_eq!(csv_field("line\nbreak"), "\"line\nbreak\"");
    }
}
