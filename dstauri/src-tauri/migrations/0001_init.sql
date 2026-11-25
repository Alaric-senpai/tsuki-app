-- Tsuki library schema.
--
-- Mirrors the legacy PostgreSQL catalog from db.py (series, episodes) but
-- drops the app_settings table: settings live in the JSON config file.
-- Timestamps are ISO-8601 UTC strings produced by the Rust side.

CREATE TABLE series (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    key         TEXT NOT NULL UNIQUE,
    name        TEXT NOT NULL,
    save_dir    TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

CREATE TABLE episodes (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    series_id      INTEGER NOT NULL REFERENCES series(id) ON DELETE CASCADE,
    video_id       TEXT NOT NULL,
    url            TEXT NOT NULL,
    episode_number INTEGER,
    title          TEXT NOT NULL DEFAULT '',
    status         TEXT NOT NULL DEFAULT 'pending'
                   CHECK (status IN ('pending', 'queued', 'downloaded', 'failed')),
    downloaded_at  TEXT,
    created_at     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    UNIQUE (series_id, video_id)
);

-- Added for the in-app player: resume position and watched state.
CREATE TABLE watch_progress (
    episode_id   INTEGER PRIMARY KEY REFERENCES episodes(id) ON DELETE CASCADE,
    position_sec REAL NOT NULL DEFAULT 0,
    duration_sec REAL NOT NULL DEFAULT 0,
    updated_at   TEXT NOT NULL,
    completed    INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX idx_series_key ON series(key);
CREATE INDEX idx_episodes_series_id ON episodes(series_id);
CREATE INDEX idx_episodes_status ON episodes(status);
CREATE INDEX idx_episodes_video_id ON episodes(video_id);
