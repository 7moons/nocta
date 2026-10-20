use crate::{models::*, storage::Storage, CoreError, CoreResult};
use sqlx::FromRow;
use std::collections::HashMap;

impl Storage {
    pub async fn record_operation(
        &self,
        kind: &str,
        outcome: &str,
        duration_ms: u32,
        error: Option<&str>,
    ) -> CoreResult<()> {
        if !["check", "launch"].contains(&kind) {
            return Err(CoreError::Validation("Unknown metric operation.".into()));
        }
        let mut tx = self.pool.begin().await?;
        let mut counters = vec![format!("{kind}:total"), format!("{kind}:{outcome}")];
        if let Some(error) = error {
            counters.push(format!("error:{error}"));
        }
        for counter in counters {
            sqlx::query("INSERT INTO metric_counters (name,count) VALUES (?,1) ON CONFLICT(name) DO UPDATE SET count = count + 1").bind(counter).execute(&mut *tx).await?;
        }
        sqlx::query("INSERT INTO metric_durations (kind,duration_ms,count) VALUES (?,?,1) ON CONFLICT(kind,duration_ms) DO UPDATE SET count = count + 1")
            .bind(kind).bind(duration_ms).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(())
    }
    pub async fn metrics(&self) -> CoreResult<MetricsSummary> {
        let counters: Vec<(String, i64)> = sqlx::query_as("SELECT name,count FROM metric_counters")
            .fetch_all(&self.read)
            .await?;
        let counters = counters.into_iter().collect::<HashMap<_, _>>();
        let count = |key: &str| {
            counters
                .get(key)
                .copied()
                .unwrap_or(0)
                .clamp(0, u32::MAX as i64) as u32
        };
        #[derive(FromRow)]
        struct Row {
            kind: String,
            duration_ms: i64,
            count: i64,
        }
        let rows = sqlx::query_as::<_, Row>(
            "SELECT kind,duration_ms,count FROM metric_durations ORDER BY kind,duration_ms",
        )
        .fetch_all(&self.read)
        .await?;
        let summary = |kind: &str| {
            let samples = rows
                .iter()
                .filter(|row| row.kind == kind)
                .map(|row| row.count.max(0) as u64)
                .sum::<u64>();
            if samples == 0 {
                return DurationSummary::default();
            }
            let sum = rows
                .iter()
                .filter(|row| row.kind == kind)
                .map(|row| row.duration_ms as f64 * row.count as f64)
                .sum::<f64>();
            let rank = (samples as f64 * 0.95).ceil() as u64;
            let mut accumulated = 0u64;
            let p95 = rows
                .iter()
                .filter(|row| row.kind == kind)
                .find_map(|row| {
                    accumulated += row.count.max(0) as u64;
                    (accumulated >= rank)
                        .then_some(row.duration_ms.clamp(0, u32::MAX as i64) as u32)
                })
                .unwrap_or(0);
            DurationSummary {
                samples: samples.min(u32::MAX as u64) as u32,
                average_ms: sum / samples as f64,
                p95_ms: p95,
            }
        };
        Ok(MetricsSummary {
            checks_total: count("check:total"),
            captchas_detected_total: count("check:captcha"),
            launches_total: count("launch:total"),
            launches_success_total: count("launch:success"),
            check: summary("check"),
            launch: summary("launch"),
            errors: MetricErrors {
                check_timeout: count("error:check_timeout"),
                verify_timeout: count("error:verify_timeout"),
                ambiguous_precheck: count("error:ambiguous_precheck"),
            },
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn exact_weighted_percentiles_and_counters_survive_reopening() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("metrics.sqlite");
        let storage = Storage::open(&path).await.unwrap();
        for _ in 0..19 {
            storage
                .record_operation("check", "healthy", 100, None)
                .await
                .unwrap();
        }
        storage
            .record_operation("check", "captcha", 900, Some("ambiguous_precheck"))
            .await
            .unwrap();
        let metrics = storage.metrics().await.unwrap();
        assert_eq!(metrics.checks_total, 20);
        assert_eq!(metrics.captchas_detected_total, 1);
        assert_eq!(metrics.check.p95_ms, 100);
        assert_eq!(metrics.check.average_ms, 140.0);
        storage.close().await;
        let storage = Storage::open(&path).await.unwrap();
        assert_eq!(storage.metrics().await.unwrap().checks_total, 20);
        storage
            .record_operation("launch", "success", 500, None)
            .await
            .unwrap();
        assert_eq!(storage.metrics().await.unwrap().launches_success_total, 1);
    }
}
