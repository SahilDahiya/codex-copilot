//! Exact Copilot charges, kept separately from rollouts and model-visible history.
use super::StateRuntime;
use codex_protocol::ThreadId;
use codex_protocol::protocol::EventMsg;
use sqlx::Row;
use std::sync::OnceLock;

#[derive(Debug, Default, PartialEq, Eq)]
pub struct CopilotUsage {
    pub nano_aiu: i64,
    pub responses: i64,
    pub partial: bool,
    pub pending: bool,
}

fn owner() -> &'static str {
    static OWNER: OnceLock<String> = OnceLock::new();
    OWNER.get_or_init(|| uuid::Uuid::new_v4().to_string())
}

impl StateRuntime {
    /// Record only accounting events. A repeated response ID never adds a second charge.
    pub async fn record_copilot_usage(
        &self,
        thread_id: ThreadId,
        turn_id: &str,
        model: &str,
        event: &EventMsg,
    ) -> anyhow::Result<()> {
        let (status, incomplete) = match event {
            EventMsg::RawResponseCompleted(response) => {
                let charge = response
                    .usage_metadata
                    .as_ref()
                    .and_then(|usage| usage.metadata.as_ref())
                    .and_then(|usage| usage.get("copilot_total_nano_aiu"))
                    .and_then(serde_json::Value::as_i64)
                    .filter(|charge| *charge >= 0);
                let model = response
                    .usage_metadata
                    .as_ref()
                    .and_then(|usage| usage.metadata.as_ref())
                    .and_then(|usage| usage.get("copilot_model"))
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or(model);
                // Bound upstream identifiers, and never persist prompts or credentials.
                anyhow::ensure!(
                    response.response_id.len() <= 512 && model.len() <= 256,
                    "Copilot usage identifier exceeds limit"
                );
                sqlx::query(
                    "INSERT INTO copilot_usage_responses
                    (response_id, thread_id, turn_id, model, nano_aiu) VALUES (?, ?, ?, ?, ?)
                    ON CONFLICT(response_id) DO UPDATE SET
                    nano_aiu = COALESCE(copilot_usage_responses.nano_aiu, excluded.nano_aiu)",
                )
                .bind(&response.response_id)
                .bind(thread_id.to_string())
                .bind(turn_id)
                .bind(model)
                .bind(charge)
                .execute(self.pool.as_ref())
                .await?;
                return Ok(());
            }
            EventMsg::TurnStarted(_) => ("pending", false),
            EventMsg::StreamError(_) => ("pending", true),
            EventMsg::TurnComplete(turn) => ("complete", turn.error.is_some()),
            EventMsg::TurnAborted(_) => ("complete", true),
            _ => return Ok(()),
        };
        sqlx::query(
            "INSERT INTO copilot_usage_turns (thread_id, turn_id, owner, status, incomplete)
            VALUES (?, ?, ?, ?, ?) ON CONFLICT(thread_id, turn_id) DO UPDATE SET
            owner = excluded.owner, status = excluded.status,
            incomplete = MAX(copilot_usage_turns.incomplete, excluded.incomplete)",
        )
        .bind(thread_id.to_string())
        .bind(turn_id)
        .bind(owner())
        .bind(status)
        .bind(incomplete)
        .execute(self.pool.as_ref())
        .await?;
        Ok(())
    }

    /// Include spawned descendants, but never a fork's copied history.
    /// Unfinished turns from a previous process are incomplete, rather than still pending.
    pub async fn copilot_usage(&self, thread_id: ThreadId) -> anyhow::Result<CopilotUsage> {
        let row = sqlx::query("WITH RECURSIVE scope(id) AS (
                SELECT ? UNION SELECT child_thread_id FROM thread_spawn_edges
                JOIN scope ON parent_thread_id = scope.id
            ) SELECT
            (SELECT COALESCE(SUM(nano_aiu), 0) FROM copilot_usage_responses
                WHERE thread_id IN scope) AS total,
            (SELECT COUNT(*) FROM copilot_usage_responses WHERE thread_id IN scope) AS responses,
            (EXISTS(SELECT 1 FROM copilot_usage_responses WHERE thread_id IN scope AND nano_aiu IS NULL)
                OR EXISTS(SELECT 1 FROM copilot_usage_turns WHERE thread_id IN scope
                    AND (incomplete = 1 OR (status = 'pending' AND owner != ?)))) AS partial,
            EXISTS(SELECT 1 FROM copilot_usage_turns WHERE thread_id IN scope
                AND status = 'pending' AND owner = ?) AS pending")
            .bind(thread_id.to_string()).bind(owner()).bind(owner())
            .fetch_one(self.pool.as_ref()).await?;
        Ok(CopilotUsage {
            nano_aiu: row.try_get("total")?,
            responses: row.try_get("responses")?,
            partial: row.try_get("partial")?,
            pending: row.try_get("pending")?,
        })
    }
}

#[cfg(test)]
#[path = "copilot_usage_tests.rs"]
mod tests;
