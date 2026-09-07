use super::*;
use pretty_assertions::assert_eq;

#[test]
fn cache_reads_writes_and_reasoning_are_not_double_counted() {
    let usage = TokenUsage {
        input_tokens: 1_000,
        cached_input_tokens: 200,
        cache_write_input_tokens: 300,
        output_tokens: 100,
        reasoning_output_tokens: 80,
        ..Default::default()
    };
    assert_eq!(estimate_nano_usd("gpt-6-astra", &usage), Some(13_950_000));
    assert_eq!(estimate_nano_usd("gpt-5.6-luna", &usage), Some(299_000));
}

#[test]
fn long_context_rates_apply_only_above_the_threshold() {
    let mut usage = TokenUsage {
        input_tokens: 272_000,
        output_tokens: 100,
        ..Default::default()
    };
    assert_eq!(
        estimate_nano_usd("gpt-6-astra", &usage),
        Some(2_725_000_000)
    );
    usage.input_tokens += 1;
    assert_eq!(
        estimate_nano_usd("gpt-6-astra", &usage),
        Some(5_447_520_000)
    );
    assert_eq!(estimate_nano_usd("gpt-5.4-mini", &usage), Some(204_450_750));
}

#[test]
fn unknown_models_invalid_usage_and_overflow_do_not_invent_a_price() {
    assert_eq!(estimate_nano_usd("unknown", &TokenUsage::default()), None);
    for usage in [
        TokenUsage {
            input_tokens: -1,
            ..Default::default()
        },
        TokenUsage {
            input_tokens: 10,
            cached_input_tokens: 11,
            ..Default::default()
        },
        TokenUsage {
            input_tokens: 10,
            cached_input_tokens: 5,
            cache_write_input_tokens: 6,
            ..Default::default()
        },
        TokenUsage {
            input_tokens: i64::MAX,
            ..Default::default()
        },
    ] {
        assert_eq!(estimate_nano_usd("gpt-6-astra", &usage), None);
    }
}
