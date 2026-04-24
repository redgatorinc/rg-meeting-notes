'use client';

import { useCallback, useEffect, useMemo, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { toast } from 'sonner';
import { Check, X, Sparkles, ChevronDown, ChevronRight, Loader2, Zap } from 'lucide-react';
import { Button } from '@/components/ui/button';

const HIGH_CONFIDENCE_THRESHOLD = 0.7;

interface NameCandidatesPanelProps {
  meetingId: string;
  /**
   * Called after the user applies the panel so the caller can refresh the
   * speakers list / transcript labels.
   */
  onApplied?: () => void | Promise<void>;
}

interface SpeakerNameCandidateRow {
  id: string;
  meeting_id: string;
  cluster_idx: number;
  candidate_name: string;
  source: string;
  confidence: number;
}

interface ClusterGroup {
  cluster_idx: number;
  top: SpeakerNameCandidateRow;
  alternatives: SpeakerNameCandidateRow[];
}

/**
 * Review panel shown when the diarization pipeline has produced
 * speaker-name suggestions. Users accept / reject / override per cluster;
 * final assignments go through `diarization_apply_names` which writes to
 * `speakers.display_name` and clears the candidate rows.
 */
export function NameCandidatesPanel({ meetingId, onApplied }: NameCandidatesPanelProps) {
  const [rows, setRows] = useState<SpeakerNameCandidateRow[]>([]);
  const [decisions, setDecisions] = useState<Record<number, string | null>>({});
  const [overrides, setOverrides] = useState<Record<number, string>>({});
  const [expanded, setExpanded] = useState(true);
  const [applying, setApplying] = useState(false);

  const refresh = useCallback(async () => {
    try {
      const list = await invoke<SpeakerNameCandidateRow[]>(
        'diarization_name_candidates',
        { meetingId },
      );
      setRows(list ?? []);
    } catch (err) {
      console.error('diarization_name_candidates failed:', err);
      setRows([]);
    }
  }, [meetingId]);

  useEffect(() => {
    void refresh();
    const unlisteners: UnlistenFn[] = [];
    void listen<{ meeting_id: string }>(
      'diarization-name-candidates-ready',
      (e) => {
        if (e.payload.meeting_id === meetingId) {
          setExpanded(true);
          void refresh();
        }
      },
    ).then((u) => unlisteners.push(u));
    return () => unlisteners.forEach((u) => u());
  }, [meetingId, refresh]);

  // Group by cluster_idx; top = highest-confidence candidate.
  const groups = useMemo<ClusterGroup[]>(() => {
    const byCluster = new Map<number, SpeakerNameCandidateRow[]>();
    for (const r of rows) {
      const arr = byCluster.get(r.cluster_idx) ?? [];
      arr.push(r);
      byCluster.set(r.cluster_idx, arr);
    }
    const out: ClusterGroup[] = [];
    byCluster.forEach((arr, cluster_idx) => {
      const sorted = arr.slice().sort((a, b) => b.confidence - a.confidence);
      out.push({
        cluster_idx,
        top: sorted[0],
        alternatives: sorted.slice(1),
      });
    });
    out.sort((a, b) => a.cluster_idx - b.cluster_idx);
    return out;
  }, [rows]);

  if (groups.length === 0) return null;

  const acceptName = (clusterIdx: number, name: string) => {
    setDecisions((prev) => ({ ...prev, [clusterIdx]: name }));
  };

  const reject = (clusterIdx: number) => {
    setDecisions((prev) => ({ ...prev, [clusterIdx]: null }));
  };

  const setOverride = (clusterIdx: number, value: string) => {
    setOverrides((prev) => ({ ...prev, [clusterIdx]: value }));
  };

  const applyAll = async () => {
    setApplying(true);
    try {
      const assignments: Record<number, string> = {};
      for (const group of groups) {
        const ov = overrides[group.cluster_idx]?.trim();
        if (ov) {
          assignments[group.cluster_idx] = ov;
          continue;
        }
        const decided = decisions[group.cluster_idx];
        if (decided === undefined) continue;
        if (decided === null) {
          assignments[group.cluster_idx] = '';
        } else {
          assignments[group.cluster_idx] = decided;
        }
      }
      if (Object.keys(assignments).length === 0) {
        toast.info('Nothing to apply', { description: 'Accept or override at least one suggestion.' });
        setApplying(false);
        return;
      }
      await invoke('diarization_apply_names', { meetingId, assignments });
      toast.success(`Applied ${Object.keys(assignments).length} name${Object.keys(assignments).length === 1 ? '' : 's'}`);
      setRows([]);
      setDecisions({});
      setOverrides({});
      await onApplied?.();
    } catch (err) {
      toast.error('Failed to apply names', {
        description: err instanceof Error ? err.message : String(err),
      });
    } finally {
      setApplying(false);
    }
  };

  return (
    <div className="mx-6 my-3 rounded-lg border border-amber-200 bg-amber-50/50 shadow-sm">
      <button
        type="button"
        onClick={() => setExpanded((v) => !v)}
        className="flex w-full items-center gap-2 px-4 py-2 text-left"
      >
        {expanded ? (
          <ChevronDown className="h-4 w-4 text-amber-700" />
        ) : (
          <ChevronRight className="h-4 w-4 text-amber-700" />
        )}
        <Sparkles className="h-4 w-4 text-amber-700" />
        <span className="text-sm font-semibold text-amber-900">
          Review name suggestions
        </span>
        <span className="text-xs text-amber-700">
          · {groups.length} cluster{groups.length === 1 ? '' : 's'}
        </span>
      </button>

      {expanded && (
        <div className="px-4 pb-3">
          <div className="space-y-3">
            {groups.map((group) => {
              const decided = decisions[group.cluster_idx];
              const override = overrides[group.cluster_idx] ?? '';
              return (
                <div
                  key={group.cluster_idx}
                  className="rounded-md border border-amber-200 bg-white p-3"
                >
                  <div className="mb-2 flex items-center justify-between">
                    <span className="text-xs font-medium uppercase tracking-wide text-gray-500">
                      Speaker {group.cluster_idx + 1}
                    </span>
                    {decided !== undefined && (
                      <span className="text-[11px] text-gray-500">
                        {override.trim()
                          ? `→ ${override.trim()}`
                          : decided === null
                            ? 'rejected'
                            : `→ ${decided}`}
                      </span>
                    )}
                  </div>

                  <div className="flex flex-wrap items-center gap-2">
                    <Button
                      size="sm"
                      variant={decided === group.top.candidate_name && !override.trim() ? 'default' : 'outline'}
                      onClick={() => acceptName(group.cluster_idx, group.top.candidate_name)}
                      className="gap-1"
                    >
                      <Check className="h-3.5 w-3.5" />
                      {group.top.candidate_name}
                    </Button>
                    <span className="text-[11px] text-gray-500">
                      {group.top.source} · {Math.round(group.top.confidence * 100)}%
                    </span>
                    <Button
                      size="sm"
                      variant={decided === null && !override.trim() ? 'default' : 'outline'}
                      onClick={() => reject(group.cluster_idx)}
                      className="gap-1 text-gray-500"
                    >
                      <X className="h-3.5 w-3.5" />
                      Reject
                    </Button>
                    <input
                      type="text"
                      placeholder="Other name…"
                      value={override}
                      onChange={(e) => setOverride(group.cluster_idx, e.target.value)}
                      className="flex-1 min-w-[140px] rounded-md border border-gray-200 bg-white px-2 py-1 text-xs focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500"
                    />
                  </div>

                  {group.alternatives.length > 0 && (
                    <div className="mt-2 flex flex-wrap gap-1.5 text-[11px] text-gray-500">
                      <span>Also:</span>
                      {group.alternatives.map((alt) => (
                        <button
                          key={alt.id}
                          type="button"
                          onClick={() => acceptName(group.cluster_idx, alt.candidate_name)}
                          className="underline decoration-gray-300 hover:text-gray-800"
                        >
                          {alt.candidate_name} ({alt.source} · {Math.round(alt.confidence * 100)}%)
                        </button>
                      ))}
                    </div>
                  )}
                </div>
              );
            })}
          </div>

          <div className="mt-3 flex items-center justify-between gap-2">
            <AcceptHighConfidenceButton
              groups={groups}
              onAcceptAll={(map) => setDecisions((prev) => ({ ...prev, ...map }))}
              disabled={applying}
            />
            <Button
              size="sm"
              onClick={() => void applyAll()}
              disabled={applying}
              className="bg-amber-600 hover:bg-amber-700"
            >
              {applying ? (
                <>
                  <Loader2 className="mr-1.5 h-3.5 w-3.5 animate-spin" /> Applying…
                </>
              ) : (
                'Apply'
              )}
            </Button>
          </div>
        </div>
      )}
    </div>
  );
}

function AcceptHighConfidenceButton({
  groups,
  onAcceptAll,
  disabled,
}: {
  groups: ClusterGroup[];
  onAcceptAll: (decisions: Record<number, string>) => void;
  disabled: boolean;
}) {
  const highConfGroups = groups.filter(
    (g) => g.top.confidence >= HIGH_CONFIDENCE_THRESHOLD,
  );
  if (highConfGroups.length === 0) return <span />; // hold the flex slot

  const handleClick = () => {
    const map: Record<number, string> = {};
    highConfGroups.forEach((g) => {
      map[g.cluster_idx] = g.top.candidate_name;
    });
    onAcceptAll(map);
  };

  return (
    <Button
      size="sm"
      variant="outline"
      onClick={handleClick}
      disabled={disabled}
      className="gap-1.5 text-amber-700 hover:text-amber-800"
      title={`Pre-accept the ${highConfGroups.length} cluster${highConfGroups.length === 1 ? '' : 's'} with ≥${Math.round(HIGH_CONFIDENCE_THRESHOLD * 100)}% confidence — you still need to click Apply`}
    >
      <Zap className="h-3.5 w-3.5" />
      Accept {highConfGroups.length} high-confidence
    </Button>
  );
}
