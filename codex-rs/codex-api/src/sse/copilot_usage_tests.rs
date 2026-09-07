use super::*;
use pretty_assertions::assert_eq;

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
