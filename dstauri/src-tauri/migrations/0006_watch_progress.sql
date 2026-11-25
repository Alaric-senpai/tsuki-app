-- Playback resume state for downloaded episodes.
--
-- Kept separate from `episodes` because progress is written on nearly every
-- seek/pause; a focused table keeps those writes cheap and lets the player
-- read one row without joining the catalog. `position_sec`/`duration_sec` are
-- real seconds (fractional) as reported by the media element.

CREATE TABLE IF NOT EXISTS watch_progress (
    episode_id   INTEGER PRIMARY KEY REFERENCES episodes(id) ON DELETE CASCADE,
    position_sec REAL    NOT NULL DEFAULT 0,
    duration_sec REAL    NOT NULL DEFAULT 0,
    updated_at   TEXT    NOT NULL,
    completed    INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_watch_progress_updated
    ON watch_progress(updated_at DESC);
