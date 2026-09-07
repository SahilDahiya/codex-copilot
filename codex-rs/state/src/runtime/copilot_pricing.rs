//! Standard API-equivalent text pricing, verified 2026-09-07:
//! https://developers.openai.com/api/docs/pricing
//! Older models: https://developers.openai.com/api/docs/models/gpt-5.4
//! and the corresponding gpt-5.5 / gpt-5.4-mini model pages.
//! This is an estimate, not a Copilot invoice. Excludes tool fees and service-tier
//! surcharges. Long-context multipliers are applied per completed request.
use codex_protocol::protocol::TokenUsage;

pub(super) fn estimate_nano_usd(model: &str, usage: &TokenUsage) -> Option<i64> {
    // Nano USD per token (equivalently, USD per million tokens multiplied by 1000).
    let (input, cached, write, output, long_context): (i128, i128, i128, i128, bool) = match model {
        "gpt-6-astra" => (10_000, 1_000, 12_500, 50_000, true),
        "gpt-5.6-sol" => (4_000, 400, 5_000, 20_000, true),
        "gpt-5.6-terra" => (2_000, 200, 2_500, 12_000, true),
        "gpt-5.6-luna" => (200, 20, 250, 1_200, true),
        "gpt-5.5" => (5_000, 500, 5_000, 30_000, true),
        "gpt-5.4" => (2_500, 250, 2_500, 15_000, true),
        "gpt-5.4-mini" => (750, 75, 750, 4_500, false),
        _ => return None,
    };
    let total_input = i128::from(usage.input_tokens);
    let reads = i128::from(usage.cached_input_tokens);
    let writes = i128::from(usage.cache_write_input_tokens);
    let outputs = i128::from(usage.output_tokens);
    if total_input < 0 || reads < 0 || writes < 0 || outputs < 0 || reads + writes > total_input {
        return None;
    }
    let input_cost = (total_input - reads - writes) * input + reads * cached + writes * write;
    // Reasoning is already included in output_tokens; do not charge it twice.
    let cost = if long_context && total_input > 272_000 {
        input_cost * 2 + outputs * output * 3 / 2
    } else {
        input_cost + outputs * output
    };
    i64::try_from(cost).ok()
}

#[cfg(test)]
#[path = "copilot_pricing_tests.rs"]
mod tests;
