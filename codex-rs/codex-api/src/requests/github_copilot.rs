//! Request compatibility for the locally configured GitHub Copilot provider.

use codex_client::EncodedJsonBody;
use http::HeaderMap;
use http::HeaderValue;
use serde_json::Value;

pub(crate) fn prepare(
    body: EncodedJsonBody,
    headers: &mut HeaderMap,
) -> Result<EncodedJsonBody, serde_json::Error> {
    let mut request: Value = serde_json::from_slice(body.as_bytes())?;
    // Copilot does not accept ChatGPT subscription service tiers.
    if let Some(object) = request.as_object_mut() {
        object.remove("service_tier");
    }
    let input = request.get("input").and_then(Value::as_array);
    let user_initiated = !headers.contains_key("x-openai-subagent")
        && input
            .and_then(|items| items.last())
            .is_some_and(|item| item.get("role").and_then(Value::as_str) == Some("user"));
    headers.insert(
        "x-initiator",
        HeaderValue::from_static(if user_initiated { "user" } else { "agent" }),
    );
    headers.insert(
        "openai-intent",
        HeaderValue::from_static("conversation-edits"),
    );
    // Inspect only content containers; tool argument strings are opaque.
    let has_images = input.is_some_and(|items| {
        items.iter().any(|item| {
            ["content", "output"].iter().any(|key| {
                item.get(key)
                    .and_then(Value::as_array)
                    .is_some_and(|content| {
                        content.iter().any(|part| {
                            part.get("type").and_then(Value::as_str) == Some("input_image")
                        })
                    })
            })
        })
    });
    if has_images {
        headers.insert("copilot-vision-request", HeaderValue::from_static("true"));
    }
    EncodedJsonBody::encode(&request)
}
