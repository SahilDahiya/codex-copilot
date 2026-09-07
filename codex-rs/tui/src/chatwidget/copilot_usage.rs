//! Refresh the durable local-server ledger without putting accounting in model context.
use super::ChatWidget;
use super::ThreadId;
use crate::app_event::AppEvent;
use crate::bottom_pane::CopilotUsageDisplay;
use codex_app_server_protocol::ThreadCopilotUsageReadResponse;
use std::time::Duration;
use std::time::Instant;

#[derive(Default)]
pub(super) struct CopilotUsageState {
    thread_id: Option<ThreadId>,
    next_refresh: Option<Instant>,
    loading: bool,
}

impl ChatWidget {
    pub(super) fn refresh_copilot_usage(&mut self) {
        if self.config.model_provider.name != "GitHub Copilot" {
            self.bottom_pane.set_copilot_usage(None);
            return;
        }
        let Some(thread_id) = self.thread_id else {
            return;
        };
        if self.copilot_usage.thread_id != Some(thread_id) {
            self.copilot_usage = CopilotUsageState {
                thread_id: Some(thread_id),
                ..Default::default()
            };
            self.bottom_pane
                .set_copilot_usage(Some(CopilotUsageDisplay::Loading));
        }
        if self.copilot_usage.loading {
            return;
        }
        if let Some(due) = self.copilot_usage.next_refresh
            && due > Instant::now()
        {
            self.frame_requester
                .schedule_frame_in(due.saturating_duration_since(Instant::now()));
            return;
        }
        self.copilot_usage.loading = true;
        self.app_event_tx
            .send(AppEvent::RefreshCopilotUsage { thread_id });
    }

    pub(crate) fn finish_copilot_usage(
        &mut self,
        thread_id: ThreadId,
        result: Result<ThreadCopilotUsageReadResponse, String>,
    ) {
        if self.thread_id != Some(thread_id) {
            return;
        }
        self.copilot_usage.loading = false;
        let delay = Duration::from_secs(/*secs*/ 2);
        self.copilot_usage.next_refresh = Some(Instant::now() + delay);
        self.bottom_pane.set_copilot_usage(Some(match result {
            Ok(usage) => CopilotUsageDisplay::Available(usage),
            Err(_) => CopilotUsageDisplay::Unavailable,
        }));
        self.frame_requester.schedule_frame();
        self.frame_requester.schedule_frame_in(delay);
    }
}
