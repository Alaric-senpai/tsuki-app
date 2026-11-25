-- Episodes gain a distinct `not_started` status.
--
-- `pending` meant "in the library, no download has run yet" but read as if a
-- job were in flight. `not_started` makes that state explicit: a freshly added
-- episode, or one whose job was cancelled / cleared from the queue.
--
-- SQLite cannot alter a CHECK constraint in place, so the table is rebuilt.
-- Row ids are preserved, which keeps `watch_progress` and archive tags valid.
-- `watch_progress` rows are parked in a temp table because dropping a parent
-- table with `foreign_keys=ON` cascades into its children.

CREATE TEMP TABLE watch_progress_backup AS SELECT * FROM watch_progress;

CREATE TABLE episodes_new (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    series_id      INTEGER NOT NULL REFERENCES series(id) ON DELETE CASCADE,
    video_id       TEXT NOT NULL,
    url            TEXT NOT NULL,
    episode_number INTEGER,
    title          TEXT NOT NULL DEFAULT '',
    status         TEXT NOT NULL DEFAULT 'not_started'
                   CHECK (status IN ('not_started', 'pending', 'queued', 'downloaded', 'failed')),
    downloaded_at  TEXT,
    created_at     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    UNIQUE (series_id, video_id)
);

INSERT INTO episodes_new
    (id, series_id, video_id, url, episode_number, title, status, downloaded_at, created_at)
SELECT
    id, series_id, video_id, url, episode_number, title,
    CASE WHEN status = 'pending' THEN 'not_started' ELSE status END,
    downloaded_at, created_at
FROM episodes;

DROP TABLE episodes;

ALTER TABLE episodes_new RENAME TO episodes;

CREATE INDEX idx_episodes_series_id ON episodes(series_id);
CREATE INDEX idx_episodes_status ON episodes(status);
CREATE INDEX idx_episodes_video_id ON episodes(video_id);

-- The cascade may or may not have fired depending on whether foreign keys were
-- enforced during the drop, so clear the table before restoring the backup.
DELETE FROM watch_progress;

INSERT INTO watch_progress
    (episode_id, position_sec, duration_sec, updated_at, completed)
SELECT episode_id, position_sec, duration_sec, updated_at, completed
FROM watch_progress_backup
WHERE episode_id IN (SELECT id FROM episodes);

DROP TABLE watch_progress_backup;
