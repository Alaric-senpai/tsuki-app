-- Persist the downloaded media path so the local player keeps working after a
-- restart or after the in-memory queue snapshot has been cleared.
--
-- The path is recorded when a download completes. Older, already-downloaded
-- episodes have no value here; the read command falls back to matching a file
-- in the series folder by its stable video id.

ALTER TABLE episodes ADD COLUMN file_path TEXT;
