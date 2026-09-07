use super::*;
use codex_app_server_protocol::ThreadCopilotUsageReadParams;
use codex_app_server_protocol::ThreadCopilotUsageReadResponse;

impl ThreadRequestProcessor {
    pub(crate) async fn thread_copilot_usage_read(
        &self,
        params: ThreadCopilotUsageReadParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        let thread_id = ThreadId::from_string(&params.thread_id)
            .map_err(|error| invalid_request(format!("Invalid thread ID: {error}")))?;
        let db = self
            .state_db
            .as_ref()
            .ok_or_else(|| internal_error("Usage storage unavailable"))?;
        let usage = db
            .copilot_usage(thread_id)
            .await
            .map_err(|error| internal_error(format!("Could not read Copilot usage: {error}")))?;
        Ok(Some(
            ThreadCopilotUsageReadResponse {
                nano_aiu: usage.nano_aiu,
                estimated_nano_usd: usage.estimated_nano_usd,
                responses: usage.responses,
                partial: usage.partial,
                pending: usage.pending,
            }
            .into(),
        ))
    }
}
