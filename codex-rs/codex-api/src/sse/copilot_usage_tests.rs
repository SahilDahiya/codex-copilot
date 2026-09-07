use super::*;
use pretty_assertions::assert_eq;

#[test]
fn completion_preserves_model_without_a_reported_charge() {
    let event = serde_json::from_value(serde_json::json!({
        "type": "response.completed",
        "response": {"id": "response-one", "model": "gpt-6-astra",
            "usage": {"input_tokens": 1000, "output_tokens": 100, "total_tokens": 1100}}
    }))
    .unwrap();
    let ResponseEvent::Completed { usage_metadata, .. } =
        process_responses_event(event).unwrap().unwrap()
    else {
        panic!("expected completion");
    };
    assert_eq!(
        usage_metadata,
        Some(ResponseUsageMetadata {
            amount: None,
            metadata: Some(
                serde_json::json!({"input_tokens": 1000, "output_tokens": 100,
            "total_tokens": 1100, "copilot_model": "gpt-6-astra"})
            ),
        })
    );
}

#[test]
fn copilot_completion_preserves_exact_charge_without_token_estimates() {
    for charge in [
        serde_json::json!(29_500_000),
        serde_json::json!(0),
        serde_json::json!(-1),
        serde_json::json!("bad"),
        serde_json::Value::Null,
    ] {
        let event = serde_json::from_value(serde_json::json!({
            "type": "response.completed",
            "response": {"id": "response-one", "copilot_usage": {"total_nano_aiu": charge}}
        }))
        .unwrap();
        let result = process_responses_event(event).unwrap().unwrap();
        let ResponseEvent::Completed { usage_metadata, .. } = result else {
            panic!("expected completion");
        };
        let expected =
            charge
                .as_i64()
                .filter(|charge| *charge >= 0)
                .map(|charge| ResponseUsageMetadata {
                    amount: None,
                    metadata: Some(serde_json::json!({"copilot_total_nano_aiu": charge})),
                });
        assert_eq!(usage_metadata, expected);
    }
}
