/**
 * pipelineStore — Zustand store for all SVAP pipeline data
 *
 * Replaces the old PipelineContext. Each component subscribes to only the
 * slices it needs via selectors, so unrelated state changes don't trigger
 * re-renders.
 */

import { create } from "zustand";
import { config } from "../config";
import type { FallbackData, PipelineStageStatus, Quality, Counts } from "../types";

const API_BASE = config.apiBaseUrl || "/api";

// ── Helpers ──────────────────────────────────────────────────────────────

function deduplicatePipelineStatus(statuses: PipelineStageStatus[]): PipelineStageStatus[] {
  const latest: Record<number, PipelineStageStatus> = {};
  statuses.forEach((s) => {
    latest[s.stage] = s;
  });
  return Object.values(latest).sort((a, b) => a.stage - b.stage);
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

function buildQualityMap(taxonomy: Quality[]): Record<string, Quality> {
  const map: Record<string, Quality> = {};
  taxonomy.forEach((q) => {
    map[q.quality_id] = q;
  });
  return map;
}

// ── Store shape ──────────────────────────────────────────────────────────

export interface PipelineStore {
  // Data slices
  run_id: string;
  source: "api" | "static";
  pipeline_status: PipelineStageStatus[];
  counts: Counts;
  calibration: { threshold: number };
  cases: FallbackData["cases"];
  taxonomy: FallbackData["taxonomy"];
  policies: FallbackData["policies"];
  predictions: FallbackData["predictions"];
  detection_patterns: FallbackData["detection_patterns"];
  case_convergence: unknown[];
  policy_convergence: unknown[];
  policy_catalog: Record<string, unknown>;
  enforcement_sources: FallbackData["enforcement_sources"];
  data_sources: Record<string, unknown>;
  scanned_programs: string[];

  // Derived
  threshold: number;
  qualityMap: Record<string, Quality>;

  // Status
  loading: boolean;
  error: string | null;
  apiAvailable: boolean;

  // Internal
  _token: string;

  // Actions
  _setToken: (token: string) => void;
  refresh: () => Promise<void>;
  runPipeline: () => Promise<unknown>;
  approveStage: (stage: number) => Promise<unknown>;
  seedPipeline: () => Promise<unknown>;
  uploadSourceDocument: (sourceId: string, file: File) => Promise<unknown>;
  createSource: (source: { name: string; url?: string; description?: string }) => Promise<unknown>;
  deleteSource: (sourceId: string) => Promise<unknown>;
}

// ── Store ────────────────────────────────────────────────────────────────

export const usePipelineStore = create<PipelineStore>((set, get) => {
  // ── Internal fetch functions ─────────────────────────────────────────

  const fetchDashboard = async () => {
    const token = get()._token;
    set({ loading: true, error: null });
    try {
      const res = await fetch(`${API_BASE}/dashboard`, {
        signal: AbortSignal.timeout(10000),
        headers: token ? { Authorization: `Bearer ${token}` } : {},
      });
      if (!res.ok) throw new Error(`API returned ${res.status}`);
      const apiData: FallbackData = await res.json();
      const pipeline_status = deduplicatePipelineStatus(apiData.pipeline_status || []);
      const taxonomy = apiData.taxonomy || [];
      set({
        run_id: apiData.run_id,
        source: "api",
        pipeline_status,
        counts: apiData.counts,
        calibration: apiData.calibration,
        cases: apiData.cases || [],
        taxonomy,
        policies: apiData.policies || [],
        predictions: apiData.predictions || [],
        detection_patterns: apiData.detection_patterns || [],
        case_convergence: apiData.case_convergence || [],
        policy_convergence: apiData.policy_convergence || [],
        policy_catalog: apiData.policy_catalog || {},
        enforcement_sources: apiData.enforcement_sources || [],
        data_sources: apiData.data_sources || {},
        scanned_programs: apiData.scanned_programs || [],
        threshold: apiData.calibration?.threshold ?? 3,
        qualityMap: buildQualityMap(taxonomy),
        apiAvailable: true,
        loading: false,
      });
    } catch (err) {
      const message = (err as Error).message || "Unknown error";
      console.error("SVAP API unreachable:", message);
      set({
        error: `Unable to reach the SVAP API. ${message}`,
        apiAvailable: false,
        loading: false,
      });
    }
  };

  const refreshSources = async () => {
    const token = get()._token;
    const headers: Record<string, string> = {};
    if (token) headers["Authorization"] = `Bearer ${token}`;
    const res = await fetch(`${API_BASE}/enforcement-sources`, { headers });
    if (!res.ok) return;
    const sources = await res.json();
    set({ enforcement_sources: sources });
  };

  // ── Public store ─────────────────────────────────────────────────────

  return {
    // Data — empty defaults
    run_id: "",
    source: "api",
    pipeline_status: [],
    counts: { cases: 0, taxonomy_qualities: 0, policies: 0, predictions: 0, detection_patterns: 0 },
    calibration: { threshold: 3 },
    cases: [],
    taxonomy: [],
    policies: [],
    predictions: [],
    detection_patterns: [],
    case_convergence: [],
    policy_convergence: [],
    policy_catalog: {},
    enforcement_sources: [],
    data_sources: {},
    scanned_programs: [],

    // Derived
    threshold: 3,
    qualityMap: {},

    // Status
    loading: true,
    error: null,
    apiAvailable: false,

    // Internal
    _token: "",

    // Actions
    _setToken: (token: string) => {
      set({ _token: token });
      fetchDashboard();
    },

    refresh: fetchDashboard,

    runPipeline: async () => {
      if (!get().apiAvailable) throw new Error("API not available");
      const result = await apiPost("/pipeline/run", {}, get()._token);
      await fetchDashboard();
      return result;
    },

    approveStage: async (stage: number) => {
      if (!get().apiAvailable) throw new Error("API not available");
      const result = await apiPost("/pipeline/approve", { stage }, get()._token);
      await fetchDashboard();
      return result;
    },

    seedPipeline: async () => {
      if (!get().apiAvailable) throw new Error("API not available");
      const result = await apiPost("/pipeline/seed", undefined, get()._token);
      await fetchDashboard();
      return result;
    },

    uploadSourceDocument: async (sourceId: string, file: File) => {
      if (!get().apiAvailable) throw new Error("API not available");
      const buffer = await file.arrayBuffer();
      const bytes = new Uint8Array(buffer);
      let binary = "";
      for (let i = 0; i < bytes.byteLength; i++) {
        binary += String.fromCharCode(bytes[i]);
      }
      const result = await apiPost(
        "/enforcement-sources/upload",
        { source_id: sourceId, filename: file.name, content: btoa(binary) },
        get()._token,
      );
      await refreshSources();
      return result;
    },

    createSource: async (source: { name: string; url?: string; description?: string }) => {
      if (!get().apiAvailable) throw new Error("API not available");
      const result = await apiPost("/enforcement-sources", source, get()._token);
      await refreshSources();
      return result;
    },

    deleteSource: async (sourceId: string) => {
      if (!get().apiAvailable) throw new Error("API not available");
      const result = await apiPost("/enforcement-sources/delete", { source_id: sourceId }, get()._token);
      await refreshSources();
      return result;
    },
  };
});
