CREATE TABLE copilot_usage_responses (
    response_id TEXT PRIMARY KEY,
    thread_id TEXT NOT NULL,
    turn_id TEXT NOT NULL,
    model TEXT NOT NULL,
    nano_aiu INTEGER CHECK (nano_aiu >= 0),
    recorded_at INTEGER NOT NULL DEFAULT (unixepoch())
);
CREATE INDEX copilot_usage_thread ON copilot_usage_responses(thread_id);
CREATE TABLE copilot_usage_turns (
    thread_id TEXT NOT NULL,
    turn_id TEXT NOT NULL,
    owner TEXT NOT NULL,
    status TEXT NOT NULL,
    incomplete INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY(thread_id, turn_id)
);
