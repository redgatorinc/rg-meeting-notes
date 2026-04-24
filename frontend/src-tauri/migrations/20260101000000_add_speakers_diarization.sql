-- Migration: speaker diarization scaffolding
--
-- Adds a `speakers` table (one row per distinct voice cluster detected in a
-- meeting) plus a `speaker_id` FK column on `transcripts` that points at it.
-- The existing `speaker` column added in 20251110000001_add_speaker_field.sql
-- is kept as-is (it stores 'mic' vs 'system' stream tags, a different concept
-- than diarization clusters).
--
-- `cluster_idx` is the local (per-meeting) cluster index returned by the
-- diarizer. It is kept separate from `id` so re-running diarization with a
-- different model pack can drop and recreate the rows in-place without losing
-- the stable UUID used by foreign keys and URLs.
--
-- `centroid_embedding` stores an L2-normalized f32 vector as raw bytes
-- (len * 4 bytes, little-endian). Populated by Phase 1 (post-hoc), read by
-- Phase 2 (online matching) and Phase 3 (cross-meeting voiceprints).
--
-- `embedding_model` identifies which embedding model produced the centroid
-- (e.g. "3dspeaker_eres2net_base"), so centroids from different model packs
-- are never compared by cosine similarity.

CREATE TABLE IF NOT EXISTS speakers (
    id TEXT PRIMARY KEY,
    meeting_id TEXT NOT NULL,
    cluster_idx INTEGER NOT NULL,
    display_name TEXT,
    total_speaking_ms INTEGER NOT NULL DEFAULT 0,
    centroid_embedding BLOB,
    embedding_model TEXT NOT NULL DEFAULT '',
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (meeting_id) REFERENCES meetings(id) ON DELETE CASCADE
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_speakers_meeting_cluster
    ON speakers(meeting_id, cluster_idx);
CREATE INDEX IF NOT EXISTS idx_speakers_meeting
    ON speakers(meeting_id);

ALTER TABLE transcripts ADD COLUMN speaker_id TEXT REFERENCES speakers(id);
CREATE INDEX IF NOT EXISTS idx_transcripts_speaker_id
    ON transcripts(speaker_id);
