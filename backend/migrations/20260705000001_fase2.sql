-- Fase 2: multi-notifier, status pages, heartbeats
CREATE TABLE IF NOT EXISTS monitor_notifiers (
    monitor_id TEXT NOT NULL REFERENCES monitors(id) ON DELETE CASCADE,
    notifier_id TEXT NOT NULL REFERENCES notifiers(id) ON DELETE CASCADE,
    PRIMARY KEY (monitor_id, notifier_id)
);

CREATE TABLE IF NOT EXISTS status_pages (
    id TEXT PRIMARY KEY,
    slug TEXT UNIQUE NOT NULL,
    title TEXT NOT NULL,
    description TEXT,
    monitors TEXT NOT NULL DEFAULT '[]',
    public INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS heartbeats (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    token TEXT UNIQUE NOT NULL,
    grace_seconds INTEGER NOT NULL DEFAULT 3600,
    last_seen_at TEXT,
    status TEXT NOT NULL DEFAULT 'pending',
    notifier_id TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);