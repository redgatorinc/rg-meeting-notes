//! SQLite access for the `speakers` table (per-meeting diarization clusters).
//!
//! Scaffolding for the speaker-diarization feature. Writes are atomic via
//! `replace_meeting_speakers`, which drops and recreates the rows in a
//! single transaction — required because re-running diarization yields a
//! fresh set of cluster indices that must match whatever `speaker_id`s we
//! write back onto the `transcripts` rows.

use crate::database::models::Speaker;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct SpeakerNameCandidateRow {
    pub id: String,
    pub meeting_id: String,
    pub cluster_idx: i64,
    pub candidate_name: String,
    pub source: String,
    pub confidence: f64,
}

pub struct SpeakerNameCandidatesRepository;

impl SpeakerNameCandidatesRepository {
    pub async fn insert(
        pool: &SqlitePool,
        meeting_id: &str,
        cluster_idx: i64,
        candidate_name: &str,
        source: &str,
        confidence: f32,
    ) -> Result<(), sqlx::Error> {
        let id = Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO speaker_name_candidates \
             (id, meeting_id, cluster_idx, candidate_name, source, confidence) \
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(meeting_id)
        .bind(cluster_idx)
        .bind(candidate_name)
        .bind(source)
        .bind(confidence as f64)
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn list_for_meeting(
        pool: &SqlitePool,
        meeting_id: &str,
    ) -> Result<Vec<SpeakerNameCandidateRow>, sqlx::Error> {
        sqlx::query_as::<_, SpeakerNameCandidateRow>(
            "SELECT id, meeting_id, cluster_idx, candidate_name, source, confidence \
             FROM speaker_name_candidates \
             WHERE meeting_id = ? \
             ORDER BY cluster_idx, confidence DESC",
        )
        .bind(meeting_id)
        .fetch_all(pool)
        .await
    }

    pub async fn clear_for_meeting(
        pool: &SqlitePool,
        meeting_id: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM speaker_name_candidates WHERE meeting_id = ?")
            .bind(meeting_id)
            .execute(pool)
            .await?;
        Ok(())
    }
}

pub struct MeetingParticipantsRepository;

impl MeetingParticipantsRepository {
    /// Best-effort insert — uses `OR IGNORE` because the PK is
    /// (meeting_id, display_name, source) and re-captures during a
    /// retranscribe shouldn't error.
    pub async fn insert_many(
        pool: &SqlitePool,
        meeting_id: &str,
        entries: &[(String, String)], // (display_name, source)
    ) -> Result<(), sqlx::Error> {
        for (name, source) in entries {
            sqlx::query(
                "INSERT OR IGNORE INTO meeting_participants \
                 (meeting_id, display_name, source) VALUES (?, ?, ?)",
            )
            .bind(meeting_id)
            .bind(name)
            .bind(source)
            .execute(pool)
            .await?;
        }
        Ok(())
    }
}

/// Row written when diarization produces a new cluster.
pub struct NewSpeaker {
    pub cluster_idx: i64,
    pub total_speaking_ms: i64,
    pub centroid_embedding: Option<Vec<u8>>,
    pub embedding_model: String,
}

pub struct SpeakersRepository;

impl SpeakersRepository {
    /// Load all speakers for a meeting, ordered by cluster index.
    pub async fn list_for_meeting(
        pool: &SqlitePool,
        meeting_id: &str,
    ) -> Result<Vec<Speaker>, sqlx::Error> {
        sqlx::query_as::<_, Speaker>(
            r#"
            SELECT id, meeting_id, cluster_idx, display_name,
                   total_speaking_ms, centroid_embedding, embedding_model
            FROM speakers
            WHERE meeting_id = ?
            ORDER BY cluster_idx
            "#,
        )
        .bind(meeting_id)
        .fetch_all(pool)
        .await
    }

    /// Drop every speaker for the meeting and insert the new set atomically.
    /// Also nulls out `transcripts.speaker_id` for the meeting so the caller
    /// can re-assign them in the same flow (the caller is expected to update
    /// `transcripts.speaker_id` right after this returns).
    ///
    /// Returns the inserted rows (with fresh UUIDs) in `cluster_idx` order so
    /// the caller can map a cluster index back to the stable UUID.
    pub async fn replace_meeting_speakers(
        pool: &SqlitePool,
        meeting_id: &str,
        new_rows: &[NewSpeaker],
    ) -> Result<Vec<Speaker>, sqlx::Error> {
        let mut tx = pool.begin().await?;

        sqlx::query("UPDATE transcripts SET speaker_id = NULL WHERE meeting_id = ?")
            .bind(meeting_id)
            .execute(&mut *tx)
            .await?;

        sqlx::query("DELETE FROM speakers WHERE meeting_id = ?")
            .bind(meeting_id)
            .execute(&mut *tx)
            .await?;

        let mut inserted = Vec::with_capacity(new_rows.len());
        let now = Utc::now().to_rfc3339();
        for row in new_rows {
            let id = Uuid::new_v4().to_string();
            sqlx::query(
                r#"
                INSERT INTO speakers (id, meeting_id, cluster_idx, display_name,
                    total_speaking_ms, centroid_embedding, embedding_model,
                    created_at, updated_at)
                VALUES (?, ?, ?, NULL, ?, ?, ?, ?, ?)
                "#,
            )
            .bind(&id)
            .bind(meeting_id)
            .bind(row.cluster_idx)
            .bind(row.total_speaking_ms)
            .bind(row.centroid_embedding.as_ref())
            .bind(&row.embedding_model)
            .bind(&now)
            .bind(&now)
            .execute(&mut *tx)
            .await?;

            inserted.push(Speaker {
                id,
                meeting_id: meeting_id.to_string(),
                cluster_idx: row.cluster_idx,
                display_name: None,
                total_speaking_ms: row.total_speaking_ms,
                centroid_embedding: row.centroid_embedding.clone(),
                embedding_model: row.embedding_model.clone(),
            });
        }

        tx.commit().await?;
        Ok(inserted)
    }

    /// Update a transcript row's `speaker_id` FK.
    pub async fn assign_transcript_speaker(
        pool: &SqlitePool,
        transcript_id: &str,
        speaker_id: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE transcripts SET speaker_id = ? WHERE id = ?")
            .bind(speaker_id)
            .bind(transcript_id)
            .execute(pool)
            .await?;
        Ok(())
    }

    /// Set / clear the display name for a speaker. Null clears back to
    /// `Speaker N`.
    pub async fn rename(
        pool: &SqlitePool,
        speaker_id: &str,
        display_name: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "UPDATE speakers SET display_name = ?, updated_at = ? WHERE id = ?",
        )
        .bind(display_name)
        .bind(&now)
        .bind(speaker_id)
        .execute(pool)
        .await?;
        Ok(())
    }
}
