/**
 * usePipelineData — single data hook for the entire SVAP UI
 *
 * Strategy:
 *   1. On mount, tries to fetch /api/dashboard from the FastAPI server
 *   2. If the API is available, uses live pipeline data from PostgreSQL
 *   3. If the API is unavailable (no backend running), falls back to
 *      the FALLBACK_DATA empty state — so the UI always works standalone
 *
 * The dashboard endpoint returns everything in one call (cases, taxonomy,
 * policies, predictions, patterns, convergence, calibration, HHS reference
 * data). Individual views don't need separate fetches.
 *
 * To refresh after a pipeline stage runs:
 *   call refresh() from any component
 */

/* eslint-disable react-refresh/only-export-components */
import { useState, useEffect, useCallback, useMemo, createContext, useContext } from "react";
import { FALLBACK_DATA } from "./fallback";
import { config } from "../config";
import type { FallbackData, PipelineData, PipelineStageStatus, Quality } from "../types";

const API_BASE = config.apiBaseUrl || "/api";

function buildStaticData(): FallbackData {
  return FALLBACK_DATA;
}

function deduplicatePipelineStatus(apiData: FallbackData): void {
  if (!apiData.pipeline_status) return;
  const latest: Record<number, PipelineStageStatus> = {};
  apiData.pipeline_status.forEach((s: PipelineStageStatus) => {
    latest[s.stage] = s;
  });
  apiData.pipeline_status = Object.values(latest).sort(
    (a: PipelineStageStatus, b: PipelineStageStatus) => a.stage - b.stage
  );
}

async function apiPost(path: string, body?: unknown, token?: string): Promise<unknown> {
  const headers: Record<string, string> = {};
  if (body !== undefined) headers["Content-Type"] = "application/json";
  if (token) headers["Authorization"] = `Bearer ${token}`;
  const options: RequestInit = { method: "POST", headers };
  if (body !== undefined) options.body = JSON.stringify(body);
  const res = await fetch(`${API_BASE}${path}`, options);
  if (!res.ok) throw new Error(`${path} failed: ${res.status}`);
  return res.json();
}

export function usePipelineData(token: string): PipelineData {
  const [data, setData] = useState<FallbackData>(() => buildStaticData());
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [apiAvailable, setApiAvailable] = useState(false);

  const fetchDashboard = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const res = await fetch(`${API_BASE}/dashboard`, {
        signal: AbortSignal.timeout(3000),
        headers: token ? { Authorization: `Bearer ${token}` } : {},
      });
      if (!res.ok) throw new Error(`API returned ${res.status}`);
      const apiData = await res.json();
      deduplicatePipelineStatus(apiData);
      setData({ ...apiData, source: "api" });
      setApiAvailable(true);
    } catch (err) {
      console.info("SVAP API unavailable, using fallback data.", (err as Error).message);
      setData(buildStaticData());
      setApiAvailable(false);
    } finally {
      setLoading(false);
    }
  }, [token]);

  useEffect(() => {
    fetchDashboard();
  }, [fetchDashboard]);

  const qualityMap = useMemo(() => {
    const map: Record<string, Quality> = {};
    (data.taxonomy || []).forEach((q) => {
      map[q.quality_id] = q;
    });
    return map;
  }, [data.taxonomy]);

  const runStage = useCallback(
    async (stage: number) => {
      if (!apiAvailable) throw new Error("API not available");
      return apiPost("/pipeline/run", { stage }, token);
    },
    [apiAvailable, token]
  );

  const approveStage = useCallback(
    async (stage: number) => {
      if (!apiAvailable) throw new Error("API not available");
      const result = await apiPost("/pipeline/approve", { stage }, token);
      await fetchDashboard();
      return result;
    },
    [apiAvailable, fetchDashboard, token]
  );

  const seedPipeline = useCallback(async () => {
    if (!apiAvailable) throw new Error("API not available");
    const result = await apiPost("/pipeline/seed", undefined, token);
    await fetchDashboard();
    return result;
  }, [apiAvailable, fetchDashboard, token]);

  return {
    ...data,
    threshold: data.calibration?.threshold ?? 3,
    qualityMap,
    loading,
    error,
    apiAvailable,
    refresh: fetchDashboard,
    runStage,
    approveStage,
    seedPipeline,
  };
}

// ── Context provider (optional — use if you want to avoid prop drilling) ──

const PipelineContext = createContext<PipelineData | null>(null);

export function PipelineProvider({ children, token }: { children: React.ReactNode; token: string }) {
  const pipeline = usePipelineData(token);
  return <PipelineContext.Provider value={pipeline}>{children}</PipelineContext.Provider>;
}

export function usePipeline(): PipelineData {
  const ctx = useContext(PipelineContext);
  if (!ctx) throw new Error("usePipeline must be used within PipelineProvider");
  return ctx;
}
