use super::*;
use crate::runtime::test_support::unique_temp_dir;
use codex_protocol::protocol::RawResponseCompletedEvent;
use codex_utils_absolute_path::test_support::PathExt;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn estimates_survive_resume_deduplicate_and_yield_to_reported_charges() {
    let home = unique_temp_dir();
    let config = crate::SqliteConfig::new_for_testing(home.as_path().abs());
    let db = StateRuntime::init(config.clone(), "github_copilot".into())
        .await
        .unwrap();
    let root = ThreadId::new();
    let child = ThreadId::new();
    db.upsert_thread_spawn_edge(root, child, crate::DirectionalThreadSpawnEdgeStatus::Open)
        .await
        .unwrap();
    for (thread, id, model) in [
        (root, "root", "gpt-6-astra"),
        (child, "child", "gpt-5.6-luna"),
    ] {
        let mut event = response(id, None);
        if let EventMsg::RawResponseCompleted(ref mut response) = event {
            response.token_usage = Some(codex_protocol::protocol::TokenUsage {
                input_tokens: 1_000,
                output_tokens: 100,
                ..Default::default()
            });
            response.usage_metadata.as_mut().unwrap().metadata =
                Some(serde_json::json!({"copilot_model": model}));
        }
        for _ in 0..2 {
            db.record_copilot_usage(thread, "turn", "different-selected-model", &event)
                .await
                .unwrap();
        }
    }
    drop(db);
    let db = StateRuntime::init(config, "github_copilot".into())
        .await
        .unwrap();
    assert_eq!(
        db.copilot_usage(root).await.unwrap(),
        CopilotUsage {
            estimated_nano_usd: Some(15_320_000),
            responses: 2,
            ..Default::default()
        }
    );
    db.record_copilot_usage(
        root,
        "turn",
        "gpt-6-astra",
        &response("root", Some(100_000_000)),
    )
    .await
    .unwrap();
    assert_eq!(
        db.copilot_usage(root).await.unwrap(),
        CopilotUsage {
            nano_aiu: 100_000_000,
            estimated_nano_usd: Some(320_000),
            responses: 2,
            ..Default::default()
        }
    );
    db.record_copilot_usage(child, "turn", "unknown", &response("unknown", None))
        .await
        .unwrap();
    assert_eq!(
        db.copilot_usage(root).await.unwrap(),
        CopilotUsage {
            nano_aiu: 100_000_000,
            estimated_nano_usd: Some(320_000),
            responses: 3,
            partial: true,
            ..Default::default()
        }
    );
}

fn response(id: &str, charge: Option<i64>) -> EventMsg {
    EventMsg::RawResponseCompleted(RawResponseCompletedEvent {
        response_id: id.into(),
        token_usage: None,
        usage_metadata: Some(codex_protocol::ResponseUsageMetadata {
            amount: None,
            metadata: Some(serde_json::json!({"copilot_total_nano_aiu": charge})),
        }),
    })
}

#[tokio::test]
async fn ledger_deduplicates_and_includes_children_but_not_other_threads_or_forks() {
    let home = unique_temp_dir();
    let config = crate::SqliteConfig::new_for_testing(home.as_path().abs());
    let db = StateRuntime::init(config.clone(), "github_copilot".into())
        .await
        .unwrap();
    let root = ThreadId::new();
    let child = ThreadId::new();
    let other = ThreadId::new();
    db.upsert_thread_spawn_edge(root, child, crate::DirectionalThreadSpawnEdgeStatus::Open)
        .await
        .unwrap();
    for (thread, model, event) in [
        (root, "gpt-5.4", response("one", Some(29_500_000))),
        (root, "gpt-5.4", response("one", Some(29_500_000))),
        (child, "gpt-5.3", response("two", None)),
        (other, "gpt-5.4", response("other", Some(999))),
    ] {
        db.record_copilot_usage(thread, "turn", model, &event)
            .await
            .unwrap();
    }
    assert_eq!(
        db.copilot_usage(root).await.unwrap(),
        CopilotUsage {
            nano_aiu: 29_500_000,
            estimated_nano_usd: None,
            responses: 2,
            partial: true,
            pending: false,
        }
    );
    db.record_copilot_usage(child, "turn", "gpt-5.3", &response("two", Some(1)))
        .await
        .unwrap();
    drop(db);
    let db = StateRuntime::init(config, "github_copilot".into())
        .await
        .unwrap();
    assert_eq!(
        db.copilot_usage(root).await.unwrap(),
        CopilotUsage {
            nano_aiu: 29_500_001,
            estimated_nano_usd: None,
            responses: 2,
            partial: false,
            pending: false,
        }
    );
    assert_eq!(
        db.copilot_usage(ThreadId::new()).await.unwrap(),
        CopilotUsage::default()
    );
}

#[tokio::test]
async fn unfinished_previous_process_and_missing_or_invalid_charges_are_partial() {
    let home = unique_temp_dir();
    let db = StateRuntime::init(
        crate::SqliteConfig::new_for_testing(home.as_path().abs()),
        "github_copilot".into(),
    )
    .await
    .unwrap();
    let thread = ThreadId::new();
    sqlx::query("INSERT INTO copilot_usage_turns VALUES (?, 'turn', ?, 'pending', 0)")
        .bind(thread.to_string())
        .bind(owner())
        .execute(db.pool.as_ref())
        .await
        .unwrap();
    assert_eq!(
        db.copilot_usage(thread).await.unwrap(),
        CopilotUsage {
            pending: true,
            ..Default::default()
        }
    );
    sqlx::query("UPDATE copilot_usage_turns SET owner = 'previous-process'")
        .execute(db.pool.as_ref())
        .await
        .unwrap();
    db.record_copilot_usage(thread, "turn", "gpt-5.4", &response("bad", Some(-1)))
        .await
        .unwrap();
    assert_eq!(
        db.copilot_usage(thread).await.unwrap(),
        CopilotUsage {
            partial: true,
            responses: 1,
            ..Default::default()
        }
    );
}

#[tokio::test]
async fn successful_retry_does_not_hide_a_previously_unbilled_attempt() {
    let home = unique_temp_dir();
    let db = StateRuntime::init(
        crate::SqliteConfig::new_for_testing(home.as_path().abs()),
        "github_copilot".into(),
    )
    .await
    .unwrap();
    let thread = ThreadId::new();
    for event in [
        serde_json::json!({"type":"stream_error", "message":"interrupted stream"}),
        serde_json::json!({"type":"turn_complete", "turn_id":"turn", "last_agent_message":null}),
    ] {
        db.record_copilot_usage(
            thread,
            "turn",
            "gpt-5.4",
            &serde_json::from_value(event).unwrap(),
        )
        .await
        .unwrap();
    }
    assert_eq!(
        db.copilot_usage(thread).await.unwrap(),
        CopilotUsage {
            partial: true,
            ..Default::default()
        }
    );
}
