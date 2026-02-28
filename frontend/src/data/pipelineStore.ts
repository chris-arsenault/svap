/**
 * pipelineStore — Zustand store for all SVAP pipeline data
 *
 * Replaces the old PipelineContext. Each component subscribes to only the
 * slices it needs via selectors, so unrelated state changes don't trigger
 * re-renders.
 */

import { create } from "zustand";
import { apiGet, apiPost } from "./api";
import type {
  FallbackData,
  PipelineStageStatus,
  Quality,
  Counts,
  SourceFeed,
  SourceCandidate,
  Dimension,
  TriageResult,
  ResearchSession,
  StructuralFinding,
  QualityAssessment,
} from "../types";

// ── Helpers ──────────────────────────────────────────────────────────────

function deduplicatePipelineStatus(statuses: PipelineStageStatus[]): PipelineStageStatus[] {
  const latest: Record<number, PipelineStageStatus> = {};
  statuses.forEach((s) => {
    latest[s.stage] = s;
  });
  return Object.values(latest).sort((a, b) => a.stage - b.stage);
}

function buildQualityMap(taxonomy: Quality[]): Record<string, Quality> {
  const map: Record<string, Quality> = {};
  taxonomy.forEach((q) => {
    map[q.quality_id] = q;
  });
  return map;
}

/** Compare slices by JSON.stringify — returns only changed keys, or null if nothing changed. */
function changedSlices(
  current: PipelineStore,
  incoming: Partial<PipelineStore>,
): Partial<PipelineStore> | null {
  const diff: Partial<PipelineStore> = {};
  let changed = false;
  for (const key of Object.keys(incoming)) {
    const k = key as keyof PipelineStore;
    if (JSON.stringify(current[k]) !== JSON.stringify(incoming[k])) {
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      (diff as any)[k] = incoming[k];
      changed = true;
    }
  }
  return changed ? diff : null;
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
  exploitation_trees: FallbackData["exploitation_trees"];
  detection_patterns: FallbackData["detection_patterns"];
  case_convergence: unknown[];
  policy_convergence: unknown[];
  policy_catalog: Record<string, unknown>;
  enforcement_sources: FallbackData["enforcement_sources"];
  data_sources: Record<string, unknown>;
  scanned_programs: string[];

  // Discovery & research slices (loaded on-demand)
  source_feeds: SourceFeed[];
  source_candidates: SourceCandidate[];
  dimensions: Dimension[];
  triage_results: TriageResult[];
  research_sessions: ResearchSession[];

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
  updatePipelineStatus: (stages: PipelineStageStatus[]) => void;
  runPipeline: () => Promise<unknown>;
  approveStage: (stage: number) => Promise<unknown>;
  seedPipeline: () => Promise<unknown>;
  uploadSourceDocument: (sourceId: string, file: File) => Promise<unknown>;
  createSource: (source: { name: string; url?: string; description?: string }) => Promise<unknown>;
  deleteSource: (sourceId: string) => Promise<unknown>;

  // Discovery & research actions
  fetchDiscovery: () => Promise<void>;
  runDiscoveryFeeds: () => Promise<unknown>;
  reviewCandidate: (candidateId: string, action: "accept" | "reject") => Promise<unknown>;
  fetchDimensions: () => Promise<void>;
  fetchTriageResults: () => Promise<void>;
  runTriage: () => Promise<unknown>;
  runDeepResearch: (policyIds?: string[]) => Promise<unknown>;
  fetchResearchSessions: () => Promise<void>;
  fetchFindings: (policyId: string) => Promise<StructuralFinding[]>;
  fetchAssessments: (policyId: string) => Promise<QualityAssessment[]>;
}

// ── Store ────────────────────────────────────────────────────────────────

export const usePipelineStore = create<PipelineStore>((set, get) => {
  // ── Internal fetch functions ─────────────────────────────────────────

  const fetchDashboard = async () => {
    set({ loading: true, error: null });
    try {
      const res = await apiGet("/dashboard", { signal: AbortSignal.timeout(10000) });
      if (!res.ok) throw new Error(`API returned ${res.status}`);
      const apiData: FallbackData = await res.json();
      const pipeline_status = deduplicatePipelineStatus(apiData.pipeline_status || []);
      const taxonomy = apiData.taxonomy || [];

      // Build incoming data, then only set slices that actually changed
      const incoming = {
        run_id: apiData.run_id,
        source: "api" as const,
        pipeline_status,
        counts: apiData.counts,
        calibration: apiData.calibration,
        cases: apiData.cases || [],
        taxonomy,
        policies: apiData.policies || [],
        exploitation_trees: apiData.exploitation_trees || [],
        detection_patterns: apiData.detection_patterns || [],
        case_convergence: apiData.case_convergence || [],
        policy_convergence: apiData.policy_convergence || [],
        policy_catalog: apiData.policy_catalog || {},
        enforcement_sources: apiData.enforcement_sources || [],
        data_sources: apiData.data_sources || {},
        scanned_programs: apiData.scanned_programs || [],
        threshold: apiData.calibration?.threshold ?? 3,
        qualityMap: buildQualityMap(taxonomy),
      };
      const diff = changedSlices(get(), incoming);
      // Always set status flags; only set data slices if something changed
      set({ apiAvailable: true, loading: false, ...(diff || {}) });
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
    const res = await apiGet("/enforcement-sources");
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
    counts: { cases: 0, taxonomy_qualities: 0, policies: 0, exploitation_trees: 0, detection_patterns: 0 },
    calibration: { threshold: 3 },
    cases: [],
    taxonomy: [],
    policies: [],
    exploitation_trees: [],
    detection_patterns: [],
    case_convergence: [],
    policy_convergence: [],
    policy_catalog: {},
    enforcement_sources: [],
    data_sources: {},
    scanned_programs: [],

    // Discovery & research — empty defaults
    source_feeds: [],
    source_candidates: [],
    dimensions: [],
    triage_results: [],
    research_sessions: [],

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

    updatePipelineStatus: (stages: PipelineStageStatus[]) => {
      const incoming = deduplicatePipelineStatus(stages);
      if (JSON.stringify(get().pipeline_status) !== JSON.stringify(incoming)) {
        set({ pipeline_status: incoming });
      }
    },

    runPipeline: async () => {
      if (!get().apiAvailable) throw new Error("API not available");
      const result = (await apiPost("/pipeline/run", {})) as { run_id?: string };
      if (result.run_id) {
        set({ run_id: result.run_id, pipeline_status: [] });
      }
      return result;
    },

    approveStage: async (stage: number) => {
      if (!get().apiAvailable) throw new Error("API not available");
      const result = await apiPost("/pipeline/approve", { stage });
      await fetchDashboard();
      return result;
    },

    seedPipeline: async () => {
      if (!get().apiAvailable) throw new Error("API not available");
      const result = await apiPost("/pipeline/seed");
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
      );
      await refreshSources();
      return result;
    },

    createSource: async (source: { name: string; url?: string; description?: string }) => {
      if (!get().apiAvailable) throw new Error("API not available");
      const result = await apiPost("/enforcement-sources", source);
      await refreshSources();
      return result;
    },

    deleteSource: async (sourceId: string) => {
      if (!get().apiAvailable) throw new Error("API not available");
      const result = await apiPost("/enforcement-sources/delete", { source_id: sourceId });
      await refreshSources();
      return result;
    },

    // ── Discovery & Research actions ────────────────────────────────────

    fetchDiscovery: async () => {
      const [feedsRes, candidatesRes] = await Promise.all([
        apiGet("/discovery/feeds"),
        apiGet("/discovery/candidates"),
      ]);
      if (feedsRes.ok && candidatesRes.ok) {
        set({
          source_feeds: await feedsRes.json(),
          source_candidates: await candidatesRes.json(),
        });
      }
    },

    runDiscoveryFeeds: async () => {
      const result = await apiPost("/discovery/run-feeds", {});
      await get().fetchDiscovery();
      return result;
    },

    reviewCandidate: async (candidateId: string, action: "accept" | "reject") => {
      const result = await apiPost(
        "/discovery/candidates/review",
        { candidate_id: candidateId, action },
      );
      await get().fetchDiscovery();
      return result;
    },

    fetchDimensions: async () => {
      const res = await apiGet("/dimensions");
      if (res.ok) set({ dimensions: await res.json() });
    },

    fetchTriageResults: async () => {
      const res = await apiGet("/research/triage");
      if (res.ok) set({ triage_results: await res.json() });
    },

    runTriage: async () => {
      const result = await apiPost("/research/triage", {});
      await get().fetchTriageResults();
      return result;
    },

    runDeepResearch: async (policyIds?: string[]) => {
      const body = policyIds ? { policy_ids: policyIds } : {};
      const result = await apiPost("/research/deep", body);
      await get().fetchResearchSessions();
      return result;
    },

    fetchResearchSessions: async () => {
      const res = await apiGet("/research/sessions");
      if (res.ok) set({ research_sessions: await res.json() });
    },

    fetchFindings: async (policyId: string) => {
      const res = await apiGet(`/research/findings/${policyId}`);
      if (!res.ok) return [];
      const data = await res.json();
      return data.findings || [];
    },

    fetchAssessments: async (policyId: string) => {
      const res = await apiGet(`/research/assessments/${policyId}`);
      if (!res.ok) return [];
      const data = await res.json();
      return data.assessments || [];
    },
  };
});
