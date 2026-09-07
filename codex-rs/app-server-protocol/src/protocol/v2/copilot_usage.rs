use crate::JsonSchema;
use crate::TS;
use serde::Deserialize;
use serde::Serialize;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadCopilotUsageReadParams {
    pub thread_id: String,
}

/// Backend-reported usage recorded since tracking was enabled, including spawned agents.
/// One AI credit is 1,000,000,000 nano AIU. Usage is not an invoice or an amount owed.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadCopilotUsageReadResponse {
    pub nano_aiu: i64,
    /// API-equivalent USD in billionths, only for responses without a backend charge.
    /// Null means no estimates are available. Add to the USD value of nano_aiu.
    pub estimated_nano_usd: Option<i64>,
    pub responses: i64,
    /// Some attempts have neither a backend charge nor a usable token estimate.
    pub partial: bool,
    pub pending: bool,
}
