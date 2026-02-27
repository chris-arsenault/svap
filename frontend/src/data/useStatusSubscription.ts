/**
 * useStatusSubscription — polls pipeline status while a run is active.
 *
 * Activates when run_id is set and any stage is "running".
 * Polls GET /api/status every 3 seconds and merges stage updates
 * into the Zustand store. Triggers a full dashboard refresh when
 * the pipeline finishes.
 *
 * Transport-agnostic: swap the poll loop for a WebSocket later
 * without changing any consumer code.
 */

import { useEffect, useRef } from "react";
import { usePipelineStore } from "./pipelineStore";
import { config } from "../config";
import type { PipelineStageStatus } from "../types";

const API_BASE = config.apiBaseUrl || "/api";
const POLL_INTERVAL_MS = 3000;

function hasRunningStage(stages: PipelineStageStatus[]): boolean {
  return stages.some((s) => s.status === "running");
}

export function useStatusSubscription() {
  const activeRef = useRef(false);

  useEffect(() => {
    // Subscribe to store changes outside of React render cycle
    const unsubscribe = usePipelineStore.subscribe(
      (state) => {
        const shouldPoll = Boolean(state.run_id) && hasRunningStage(state.pipeline_status);

        if (shouldPoll && !activeRef.current) {
          activeRef.current = true;
          startPolling(state.run_id, state._token);
        }
      },
    );

    // Check initial state
    const { run_id, pipeline_status, _token } = usePipelineStore.getState();
    if (run_id && hasRunningStage(pipeline_status) && !activeRef.current) {
      activeRef.current = true;
      startPolling(run_id, _token);
    }

    return () => {
      activeRef.current = false;
      unsubscribe();
    };
  }, []);

  function startPolling(runId: string, token: string) {
    const poll = async () => {
      while (activeRef.current) {
        try {
          const res = await fetch(`${API_BASE}/status`, {
            headers: token ? { Authorization: `Bearer ${token}` } : {},
            signal: AbortSignal.timeout(8000),
          });
          if (!res.ok) break;

          const data = await res.json();
          const stages: PipelineStageStatus[] = data.stages || [];

          usePipelineStore.getState().updatePipelineStatus(stages);

          // Pipeline finished — do a full dashboard refresh for new data
          if (stages.length > 0 && !hasRunningStage(stages)) {
            activeRef.current = false;
            await usePipelineStore.getState().refresh();
            return;
          }
        } catch {
          // Network error — keep trying
        }

        await new Promise((r) => setTimeout(r, POLL_INTERVAL_MS));
      }
    };

    poll();
  }
}
