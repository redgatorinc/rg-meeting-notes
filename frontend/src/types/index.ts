export interface Message {
  id: string;
  content: string;
  timestamp: string;
}

export interface Transcript {
  id: string;
  text: string;
  timestamp: string; // Wall-clock time (e.g., "14:30:05")
  sequence_id?: number;
  chunk_start_time?: number; // Legacy field
  is_partial?: boolean;
  confidence?: number;
  // NEW: Recording-relative timestamps for playback sync
  audio_start_time?: number; // Seconds from recording start (e.g., 125.3)
  audio_end_time?: number;   // Seconds from recording start (e.g., 128.6)
  duration?: number;          // Segment duration in seconds (e.g., 3.3)
  is_refinement?: boolean;   // True for full-run refinement segments that should replace chunks
  // NEW: Speaker diarization FK to the speakers table. `null`/`undefined`
  // means diarization has not been run (or this row was captured before
  // the speaker was clustered).
  speaker_id?: string | null;
}

export interface TranscriptUpdate {
  text: string;
  timestamp: string; // Wall-clock time for reference
  source: string;
  sequence_id: number;
  chunk_start_time: number; // Legacy field
  is_partial: boolean;
  confidence: number;
  // NEW: Recording-relative timestamps for playback sync
  audio_start_time: number; // Seconds from recording start
  audio_end_time: number;   // Seconds from recording start
  duration: number;          // Segment duration in seconds
  is_refinement?: boolean;  // True for full-run refinement segments that should replace chunks
  speaker_id?: string | null; // Populated after diarization runs for this meeting.
}

// One distinct voice cluster detected by diarization in a given meeting.
// `display_name` is null until the user renames — the UI renders
// `Speaker {cluster_idx + 1}` in that case.
export interface Speaker {
  id: string;
  meeting_id: string;
  cluster_idx: number;
  display_name: string | null;
  total_speaking_ms: number;
  embedding_model: string;
}

export type DiarizationModelPack = 'default' | 'fast' | 'accurate';

export interface DiarizationModelPackInfo {
  pack: DiarizationModelPack;
  installed: boolean;
  size_mb: number;
}

// Tagged union matching the Rust `DiarizationStatus` enum via
// `#[serde(tag = "state", rename_all = "lowercase")]`.
export type DiarizationStatus =
  | { state: 'idle' }
  | { state: 'downloading'; progress: number }
  | { state: 'running'; progress: number }
  | { state: 'done'; speaker_count: number }
  | { state: 'error'; message: string };

export interface Block {
  id: string;
  type: string;
  content: string;
  color: string;
}

export interface Section {
  title: string;
  blocks: Block[];
}

export interface Summary {
  [key: string]: Section;
}

export interface ApiResponse {
  message: string;
  num_chunks: number;
  data: any[];
}

export interface SummaryResponse {
  status: string;
  summary: Summary;
  raw_summary?: string;
  usage?: {
    prompt_tokens: number;
    completion_tokens: number;
    total_tokens: number;
  };
}

// BlockNote-specific types
export type SummaryFormat = 'legacy' | 'markdown' | 'blocknote';

export interface BlockNoteBlock {
  id: string;
  type: string;
  props?: Record<string, any>;
  content?: any[];
  children?: BlockNoteBlock[];
}

export interface SummaryDataResponse {
  markdown?: string;
  summary_json?: BlockNoteBlock[];
  // Legacy format fields
  MeetingName?: string;
  _section_order?: string[];
  [key: string]: any; // For legacy section data
}

// Pagination types for optimized transcript loading
export interface MeetingMetadata {
  id: string;
  title: string;
  created_at: string;
  updated_at: string;
  folder_path?: string;
}

export interface PaginatedTranscriptsResponse {
  transcripts: Transcript[];
  total_count: number;
  has_more: boolean;
}

// Transcript segment data for virtualized display
export interface TranscriptSegmentData {
  id: string;
  timestamp: number; // audio_start_time in seconds
  endTime?: number; // audio_end_time in seconds
  text: string;
  confidence?: number;
}
