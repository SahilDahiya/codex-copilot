use anyhow::Result;
use app_test_support::MockResponsesConfig;
use app_test_support::TestAppServer;
use app_test_support::create_mock_responses_server_sequence;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::ThreadCopilotUsageReadParams;
use codex_app_server_protocol::ThreadCopilotUsageReadResponse;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::UserInput;
use codex_features::Feature;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::sse;
use pretty_assertions::assert_eq;
use tempfile::TempDir;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn copilot_usage_read_uses_persistent_server_storage() -> Result<()> {
    for charge in [Some(29_500_000), None] {
        let home = TempDir::new()?;
        let mut completed = ev_completed("copilot-response");
        completed["response"]["copilot_usage"] = serde_json::json!({"total_nano_aiu":charge});
        completed["response"]["model"] = serde_json::json!("gpt-6-astra");
        completed["response"]["usage"] =
            serde_json::json!({"input_tokens":1000,"output_tokens":100,"total_tokens":1100});
        let server = create_mock_responses_server_sequence(vec![sse(vec![
            ev_response_created("copilot-response"),
            ev_assistant_message("message", "OK"),
            completed,
        ])])
        .await;
        MockResponsesConfig::new(&server.uri())
            .with_provider_name("GitHub Copilot")
            .enable_feature(Feature::Sqlite)
            .write(home.path())?;
        let mut app = TestAppServer::builder()
            .with_codex_home(home.path())
            .build_initialized()
            .await?;
        let thread = app.start_thread(ThreadStartParams::default()).await?.thread;
        app.start_turn_and_wait_for_completion(TurnStartParams {
            thread_id: thread.id.clone(),
            input: vec![UserInput::Text {
                text: "Reply OK".into(),
                text_elements: vec![],
            }],
            ..Default::default()
        })
        .await?;
        let usage: ThreadCopilotUsageReadResponse = app
            .request(|request_id| ClientRequest::ThreadCopilotUsageRead {
                request_id,
                params: ThreadCopilotUsageReadParams {
                    thread_id: thread.id,
                },
            })
            .await?;
        assert_eq!(
            usage,
            ThreadCopilotUsageReadResponse {
                nano_aiu: charge.unwrap_or(0),
                estimated_nano_usd: charge.is_none().then_some(15_000_000),
                responses: 1,
                partial: false,
                pending: false,
            }
        );
    }
    Ok(())
}
