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
import { getToken } from "../auth";
import type { PipelineStageStatus } from "../types";

const API_BASE = config.apiBaseUrl || "/api";
const POLL_INTERVAL_MS = 3000;

function hasRunningStage(stages: PipelineStageStatus[]): boolean {
  return stages.some((s) => s.status === "running");
}

export function useStatusSubscription() {
  const activeRef = useRef(false);

  useEffect(() => {
    const unsubscribe = usePipelineStore.subscribe(
      (state) => {
        const shouldPoll = Boolean(state.run_id) && hasRunningStage(state.pipeline_status);

        if (shouldPoll && !activeRef.current) {
          activeRef.current = true;
          startPolling();
        }
      },
    );

    const { run_id, pipeline_status } = usePipelineStore.getState();
    if (run_id && hasRunningStage(pipeline_status) && !activeRef.current) {
      activeRef.current = true;
      startPolling();
    }

    return () => {
      activeRef.current = false;
      unsubscribe();
    };
  }, []);

  function startPolling() {
    const poll = async () => {
      while (activeRef.current) {
        try {
          const token = await getToken();
          const headers: Record<string, string> = {};
          if (token) headers["Authorization"] = `Bearer ${token}`;
          const res = await fetch(`${API_BASE}/status`, {
            headers,
            signal: AbortSignal.timeout(8000),
          });
          if (!res.ok) break;

          const data = await res.json();
          const stages: PipelineStageStatus[] = data.stages || [];

          usePipelineStore.getState().updatePipelineStatus(stages);

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
