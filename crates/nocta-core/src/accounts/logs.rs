//! The activity log shown in the app and operation metrics.
use super::*;

impl Accounts {
    pub async fn record_operation(
        &self,
        kind: &str,
        outcome: &str,
        duration_ms: u32,
        error: Option<&str>,
    ) -> CoreResult<()> {
        self.storage
            .record_operation(kind, outcome, duration_ms, error)
            .await
    }
    #[cfg(test)]
    pub(crate) async fn storage_metrics(&self) -> MetricsSummary {
        self.storage.metrics().await.unwrap()
    }

    pub async fn logs(&self) -> CoreResult<Vec<LogEntry>> {
        #[derive(FromRow)]
        struct Row {
            id: i64,
            timestamp: String,
            level: String,
            module: String,
            message: String,
            account_id: Option<i64>,
        }
        let rows = sqlx::query_as::<_, Row>(
            "SELECT * FROM (SELECT * FROM logs ORDER BY id DESC LIMIT 500) ORDER BY id",
        )
        .fetch_all(&self.storage.read)
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| LogEntry {
                id: row.id as u32,
                timestamp: row.timestamp,
                level: row.level,
                module: row.module,
                message: row.message,
                account_id: row.account_id.map(|id| id as u32),
            })
            .collect())
    }

    pub async fn log(
        &self,
        level: &str,
        module: &str,
        message: &str,
        id: Option<u32>,
    ) -> CoreResult<()> {
        sqlx::query("INSERT INTO logs (timestamp, level, module, message, account_id) VALUES (?, ?, ?, ?, ?)")
            .bind(Utc::now().to_rfc3339()).bind(level).bind(module).bind(message).bind(id).execute(&self.storage.pool).await?;
        sqlx::query("DELETE FROM logs WHERE id <= (SELECT COALESCE(MAX(id), 0) - 10000 FROM logs)")
            .execute(&self.storage.pool)
            .await?;
        Ok(())
    }
}
