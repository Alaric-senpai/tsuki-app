-- Optional in-app banner for a series.
--
-- banner_path is relative to the app data dir (e.g. `cache/banners/1.jpg`).
-- banner_source records who chose it: '' (none), 'legacy' (copied from the
-- legacy thumbnail cache) or 'custom' (chosen by the user, never overwritten
-- by the legacy backfill).

ALTER TABLE series ADD COLUMN banner_path TEXT;
ALTER TABLE series ADD COLUMN banner_source TEXT NOT NULL DEFAULT '';
