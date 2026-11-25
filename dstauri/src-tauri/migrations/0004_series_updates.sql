-- Series provenance + update tracking.
--
-- `source_key`/`source_url` record where a series came from so the Home
-- "Updates" section can re-resolve the remote page without guessing. They stay
-- empty for series that were added by hand or imported from the legacy CLI.
--
-- `check_updates` is the per-series opt-in; `update_count` caches how many
-- remote episodes were missing at the last check and `last_checked_at` records
-- when that check ran.

ALTER TABLE series ADD COLUMN source_key TEXT NOT NULL DEFAULT '';
ALTER TABLE series ADD COLUMN source_url TEXT NOT NULL DEFAULT '';
ALTER TABLE series ADD COLUMN check_updates INTEGER NOT NULL DEFAULT 0;
ALTER TABLE series ADD COLUMN update_count INTEGER NOT NULL DEFAULT 0;
ALTER TABLE series ADD COLUMN last_checked_at TEXT;
