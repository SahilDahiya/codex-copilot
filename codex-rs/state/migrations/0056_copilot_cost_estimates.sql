ALTER TABLE copilot_usage_responses ADD COLUMN estimated_nano_usd INTEGER
    CHECK (estimated_nano_usd >= 0);
