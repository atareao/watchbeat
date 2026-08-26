-- Create initial tables for WatchBeat uptime monitor
CREATE TABLE IF NOT EXISTS monitors (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    monitor_type TEXT NOT NULL,
    target TEXT NOT NULL,
    config_json TEXT NOT NULL DEFAULT '{}',
    interval_seconds INTEGER NOT NULL DEFAULT 300,
    timeout_seconds INTEGER NOT NULL DEFAULT 30,
    enabled INTEGER NOT NULL DEFAULT 1,
    notifier_id TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS checks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    monitor_id TEXT NOT NULL REFERENCES monitors(id) ON DELETE CASCADE,
    status TEXT NOT NULL,
    status_code INTEGER,
    response_time_ms INTEGER NOT NULL DEFAULT 0,
    error_message TEXT,
    checked_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_checks_monitor
    ON checks(monitor_id, checked_at DESC);

CREATE INDEX IF NOT EXISTS idx_checks_checked_at
    ON checks(checked_at);

CREATE TABLE IF NOT EXISTS notifiers (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    notifier_type TEXT NOT NULL,
    config_json TEXT NOT NULL DEFAULT '{}',
    enabled INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);