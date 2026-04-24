-- Migration: add file_size_bytes to meetings
--
-- The meeting-list UI wants to show per-meeting recording size alongside
-- title + duration + participant count, without stat()-ing every file on
-- every list render. Persist the size at save time in the Rust-side
-- recording pipeline and read it from this column.
--
-- Existing rows get 0; the frontend renders "—" for zero so pre-existing
-- meetings don't mislabel themselves.

ALTER TABLE meetings ADD COLUMN file_size_bytes INTEGER NOT NULL DEFAULT 0;
