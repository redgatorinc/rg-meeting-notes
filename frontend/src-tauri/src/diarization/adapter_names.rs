//! Match integrated-adapter participants (Teams UIA / Zoom / Meet) to
//! diarization clusters.
//!
//! At recording start, `participant_detection::session::detect_and_lock`
//! captures the adapter's participant list and persists it via
//! `MeetingParticipantsRepository::insert_many` under the meeting id
//! that eventually materializes in `save_transcript`. At diarize time,
//! we pull the list back out and produce candidate (cluster, name) pairs.
//!
//! Strategy: the adapter only tells us *who is in the room*, not which
//! cluster they map to. So:
//!
//!   - If cluster count == participant count, rank candidates by total
//!     speaking time (most talkative cluster → most-active participant).
//!     This is heuristic but often right for 1:1s and small calls.
//!   - Otherwise, emit every (participant, cluster) pair at a low
//!     confidence floor (0.3) and let the user pick via the approval UI.
//!
//! The cross-meeting voiceprint path (matching by `speakers.centroid_embedding`)
//! will eventually upgrade this; for now we only know names from the
//! adapter snapshot.

use sqlx::SqlitePool;

use crate::database::models::Speaker;

use super::cue_parser::Candidate;

pub async fn extract_candidates(
    pool: &SqlitePool,
    meeting_id: &str,
    speakers: &[Speaker],
) -> Vec<Candidate> {
    let participants: Vec<(String, String)> = match sqlx::query_as::<_, (String, String)>(
        "SELECT display_name, source FROM meeting_participants WHERE meeting_id = ?",
    )
    .bind(meeting_id)
    .fetch_all(pool)
    .await
    {
        Ok(rows) => rows,
        Err(e) => {
            log::warn!(
                "adapter_names: failed to read meeting_participants for {}: {}",
                meeting_id,
                e
            );
            return Vec::new();
        }
    };

    if participants.is_empty() || speakers.is_empty() {
        return Vec::new();
    }

    // Sort clusters by total_speaking_ms descending so index 0 is the
    // most talkative cluster; if counts match we pair positionally.
    let mut clusters_sorted: Vec<&Speaker> = speakers.iter().collect();
    clusters_sorted.sort_by(|a, b| b.total_speaking_ms.cmp(&a.total_speaking_ms));

    let mut out: Vec<Candidate> = Vec::new();

    if clusters_sorted.len() == participants.len() {
        // Positional match, high confidence.
        for (cluster, (name, _source)) in clusters_sorted.iter().zip(participants.iter()) {
            out.push(Candidate {
                cluster_idx: cluster.cluster_idx,
                name: name.clone(),
                confidence: 0.75,
            });
        }
    } else {
        // Cross-join, low-confidence fallback. User picks in the approval UI.
        for cluster in clusters_sorted.iter() {
            for (name, _source) in participants.iter() {
                out.push(Candidate {
                    cluster_idx: cluster.cluster_idx,
                    name: name.clone(),
                    confidence: 0.35,
                });
            }
        }
    }

    out
}
