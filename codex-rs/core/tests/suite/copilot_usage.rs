//! Accounting observes real session events without modifying model history.
use anyhow::Result;
use codex_features::Feature;
use codex_state::CopilotUsage;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::test_codex::test_codex;
use pretty_assertions::assert_eq;
use serde_json::json;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn copilot_usage_is_durable_and_only_records_copilot_provider() -> Result<()> {
    for provider in ["GitHub Copilot", "Other provider"] {
        let server = start_mock_server().await;
        let mut completed = ev_completed("copilot-response");
        completed["response"]["copilot_usage"] = json!({"total_nano_aiu": 29_500_000});
        let mock = mount_sse_once(
            &server,
            sse(vec![
                ev_response_created("copilot-response"),
                ev_assistant_message("message", "OK"),
                completed,
            ]),
        )
        .await;
        let test = test_codex()
            .with_config(move |config| {
                config.model_provider.name = provider.into();
                config.features.enable(Feature::Sqlite).unwrap();
            })
            .build_with_auto_env(&server)
            .await?;
        test.submit_turn("Reply OK").await?;
        let db = test.codex.state_db().expect("usage storage");
        let thread_id = test.session_configured.thread_id;
        let expected = if provider == "GitHub Copilot" {
            CopilotUsage {
                nano_aiu: 29_500_000,
                responses: 1,
                partial: false,
                pending: false,
            }
        } else {
            CopilotUsage::default()
        };
        assert_eq!(db.copilot_usage(thread_id).await?, expected);
        assert!(
            mock.single_request()
                .body_json()
                .get("copilot_total_nano_aiu")
                .is_none()
        );
        let rollout = test.codex.rollout_path().expect("rollout");
        let home = test.home.clone();
        test.codex.shutdown_and_wait().await?;
        let resumed = test_codex()
            .with_config(move |config| {
                config.model_provider.name = provider.into();
                config.features.enable(Feature::Sqlite).unwrap();
            })
            .resume(&server, home, rollout)
            .await?;
        assert_eq!(
            resumed
                .codex
                .state_db()
                .expect("usage storage")
                .copilot_usage(thread_id)
                .await?,
            expected
        );
        resumed.codex.shutdown_and_wait().await?;
    }
    Ok(())
}
