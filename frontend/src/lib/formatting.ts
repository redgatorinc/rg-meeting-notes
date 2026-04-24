// Shared formatting helpers. Consolidates duration/bytes/date formatters
// that were previously duplicated across `lib/whisper.ts`, `lib/parakeet.ts`,
// `lib/qwen-asr.ts`, `ChunkProgressDisplay`, `RecordingStatusBar`, and the
// import-audio dialog. Pure functions — safe for server and client.

export function formatDuration(ms: number): string {
  if (!Number.isFinite(ms) || ms <= 0) return '—';
  const totalSec = Math.floor(ms / 1000);
  const hours = Math.floor(totalSec / 3600);
  const minutes = Math.floor((totalSec % 3600) / 60);
  const seconds = totalSec % 60;
  if (hours > 0) return `${hours}h ${minutes}m`;
  if (minutes > 0) return `${minutes}m`;
  return `${seconds}s`;
}

/** Seconds version — some existing callers have seconds in hand, not ms. */
export function formatDurationSeconds(seconds: number): string {
  return formatDuration(seconds * 1000);
}

export function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return '—';
  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  let value = bytes;
  let unitIdx = 0;
  while (value >= 1024 && unitIdx < units.length - 1) {
    value /= 1024;
    unitIdx++;
  }
  const precision = value >= 10 || unitIdx === 0 ? 0 : 1;
  return `${value.toFixed(precision)} ${units[unitIdx]}`;
}

/** Locale-aware short date + time, e.g. "Apr 24, 2026, 10:30 AM". */
export function formatDate(iso: string): string {
  if (!iso) return '';
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  return d.toLocaleString(undefined, {
    year: 'numeric',
    month: 'short',
    day: 'numeric',
    hour: 'numeric',
    minute: '2-digit',
  });
}
