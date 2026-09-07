use super::App;
use crate::app_event::AppEvent;
use crate::app_server_session::AppServerSession;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadCopilotUsageReadParams;
use codex_protocol::ThreadId;
use std::time::Duration;

impl App {
    pub(super) fn refresh_copilot_usage(&self, app_server: &AppServerSession, thread_id: ThreadId) {
        let request_handle = app_server.request_handle();
        let tx = self.app_event_tx.clone();
        tokio::spawn(async move {
            let request = ClientRequest::ThreadCopilotUsageRead {
                request_id: RequestId::String(format!("copilot-usage-{}", uuid::Uuid::new_v4())),
                params: ThreadCopilotUsageReadParams {
                    thread_id: thread_id.to_string(),
                },
            };
            let result = tokio::time::timeout(
                Duration::from_secs(/*secs*/ 10),
                request_handle.request_typed(request),
            )
            .await
            .map_err(|error| error.to_string())
            .and_then(|result| result.map_err(|error| error.to_string()));
            tx.send(AppEvent::CopilotUsageLoaded { thread_id, result });
        });
    }
}
