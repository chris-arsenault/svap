/**
 * useStatusSubscription — always-on dual-rate polling for pipeline status
 * and corpus change detection.
 *
 * Polls GET /api/status which returns { run_id, stages, counts }.
 * - Fast rate (3s) while any stage is "running"
 * - Slow rate (15s) while idle
 * - Compares returned counts against store counts; triggers full
 *   dashboard refresh only when data has actually changed.
 * - Detects running→stopped transition for one final refresh.
 */

import { useEffect, useRef } from "react";
import { usePipelineStore } from "./pipelineStore";
import { apiGet } from "./api";
import type { Counts, PipelineStageStatus } from "../types";

const FAST_MS = 3_000;
const SLOW_MS = 15_000;

function hasRunningStage(stages: PipelineStageStatus[]): boolean {
  return stages.some((s) => s.status === "running");
}

function countsEqual(a: Counts, b: Counts): boolean {
  return (
    a.cases === b.cases &&
    a.taxonomy_qualities === b.taxonomy_qualities &&
    a.policies === b.policies &&
    a.exploitation_trees === b.exploitation_trees &&
    a.detection_patterns === b.detection_patterns
  );
}

export function useStatusSubscription() {
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const activeRef = useRef(true);
  const wasRunningRef = useRef(false);

  useEffect(() => {
    activeRef.current = true;

    const handlePollResponse = async (data: { stages?: PipelineStageStatus[]; counts?: Counts }) => {
      const stages: PipelineStageStatus[] = data.stages || [];
      const counts: Counts = data.counts || ({} as Counts);

      usePipelineStore.getState().updatePipelineStatus(stages);

      const isRunning = hasRunningStage(stages);
      const justStopped = wasRunningRef.current && !isRunning && stages.length > 0;
      const countsChanged = !countsEqual(usePipelineStore.getState().counts, counts);

      if (justStopped || countsChanged) {
        await usePipelineStore.getState().refresh();
      }

      wasRunningRef.current = isRunning;
      return isRunning;
    };

    const poll = async () => {
      if (!activeRef.current) return;

      try {
        const res = await apiGet("/status", { signal: AbortSignal.timeout(8000) });
        if (!res.ok) { schedule(SLOW_MS); return; }

        const isRunning = await handlePollResponse(await res.json());
        schedule(isRunning ? FAST_MS : SLOW_MS);
      } catch {
        schedule(SLOW_MS);
      }
    };

    const schedule = (ms: number) => {
      if (!activeRef.current) return;
      timerRef.current = setTimeout(poll, ms);
    };

    // Start first poll immediately
    poll();

    return () => {
      activeRef.current = false;
      if (timerRef.current) clearTimeout(timerRef.current);
    };
  }, []);
}
