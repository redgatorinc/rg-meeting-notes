-- Migration: diarization history + participant capture + name-candidate review
--
-- Three new tables backing the v2 diarization work:
--
--   1. `diarization_runs` — audit log of every diarize invocation. Today
--      `replace_meeting_speakers` drops prior clusters on each run, so the
--      previous model/timestamp was lost. Insert one row per run; latest
--      row per meeting is the source of truth for "which pack last ran".
--
--   2. `meeting_participants` — snapshot of the Teams/Zoom/Meet integrated
--      adapter's participant list captured at recording start. Survives app
--      restarts so the adapter name-matcher can run later during post-hoc
--      diarization even if the meeting app has since closed.
--
--   3. `speaker_name_candidates` — transient. Populated by the three
--      name-identification passes (cue parser, LLM, adapter). Cleared by
--      `diarization_apply_names` once the user has accepted/rejected each
--      suggestion.

CREATE TABLE IF NOT EXISTS diarization_runs (
    id TEXT PRIMARY KEY,
    meeting_id TEXT NOT NULL,
    model_pack TEXT NOT NULL,
    started_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    completed_at DATETIME,
    speaker_count INTEGER NOT NULL DEFAULT 0,
    error TEXT,
    FOREIGN KEY (meeting_id) REFERENCES meetings(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_diarization_runs_meeting
    ON diarization_runs(meeting_id, started_at DESC);

CREATE TABLE IF NOT EXISTS meeting_participants (
    meeting_id TEXT NOT NULL,
    display_name TEXT NOT NULL,
    source TEXT NOT NULL,
    captured_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (meeting_id, display_name, source),
    FOREIGN KEY (meeting_id) REFERENCES meetings(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS speaker_name_candidates (
    id TEXT PRIMARY KEY,
    meeting_id TEXT NOT NULL,
    cluster_idx INTEGER NOT NULL,
    candidate_name TEXT NOT NULL,
    source TEXT NOT NULL,
    confidence REAL NOT NULL,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (meeting_id) REFERENCES meetings(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_speaker_name_candidates_meeting_cluster
    ON speaker_name_candidates(meeting_id, cluster_idx);
