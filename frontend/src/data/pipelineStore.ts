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

// ── Internal fetch functions ──────────────────────────────────────────

const EMPTY_ARRAY: never[] = [];
const EMPTY_OBJECT = {};

function orArray<T>(v: T[] | undefined | null): T[] { return v ?? EMPTY_ARRAY as T[]; }
function orObj<T extends object>(v: T | undefined | null): T { return v ?? EMPTY_OBJECT as T; }

function buildIncomingData(apiData: FallbackData) {
  const pipeline_status = deduplicatePipelineStatus(orArray(apiData.pipeline_status));
  const taxonomy = orArray(apiData.taxonomy);
  return {
    run_id: apiData.run_id,
    source: "api" as const,
    pipeline_status,
    counts: apiData.counts,
    calibration: apiData.calibration,
    cases: orArray(apiData.cases),
    taxonomy,
    policies: orArray(apiData.policies),
    exploitation_trees: orArray(apiData.exploitation_trees),
    detection_patterns: orArray(apiData.detection_patterns),
    case_convergence: orArray(apiData.case_convergence),
    policy_convergence: orArray(apiData.policy_convergence),
    policy_catalog: orObj(apiData.policy_catalog),
    enforcement_sources: orArray(apiData.enforcement_sources),
    data_sources: orObj(apiData.data_sources),
    scanned_programs: orArray(apiData.scanned_programs),
    threshold: apiData.calibration?.threshold ?? 3,
    qualityMap: buildQualityMap(taxonomy),
  };
}

// ── Discovery & Research action factory ──────────────────────────────────

type SetFn = (partial: Partial<PipelineStore>) => void;
type GetFn = () => PipelineStore;

function createDiscoveryActions(set: SetFn, get: GetFn) {
  const fetchDiscovery = async () => {
    const [feedsRes, candidatesRes] = await Promise.all([
      apiGet("/discovery/feeds"),
      apiGet("/discovery/candidates"),
    ]);
    if (feedsRes.ok && candidatesRes.ok) {
      set({ source_feeds: await feedsRes.json(), source_candidates: await candidatesRes.json() });
    }
  };

  return {
    fetchDiscovery,
    runDiscoveryFeeds: async () => {
      const result = await apiPost("/discovery/run-feeds", {});
      await fetchDiscovery();
      return result;
    },
    reviewCandidate: async (candidateId: string, action: "accept" | "reject") => {
      const result = await apiPost("/discovery/candidates/review", { candidate_id: candidateId, action });
      await fetchDiscovery();
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
      return (await res.json()).findings || [];
    },
    fetchAssessments: async (policyId: string) => {
      const res = await apiGet(`/research/assessments/${policyId}`);
      if (!res.ok) return [];
      return (await res.json()).assessments || [];
    },
  };
}

// ── Initial state ────────────────────────────────────────────────────────

const INITIAL_STATE = {
  run_id: "",
  source: "api" as const,
  pipeline_status: [] as PipelineStageStatus[],
  counts: { cases: 0, taxonomy_qualities: 0, policies: 0, exploitation_trees: 0, detection_patterns: 0 },
  calibration: { threshold: 3 },
  cases: [] as FallbackData["cases"],
  taxonomy: [] as FallbackData["taxonomy"],
  policies: [] as FallbackData["policies"],
  exploitation_trees: [] as FallbackData["exploitation_trees"],
  detection_patterns: [] as FallbackData["detection_patterns"],
  case_convergence: [] as unknown[],
  policy_convergence: [] as unknown[],
  policy_catalog: {} as Record<string, unknown>,
  enforcement_sources: [] as FallbackData["enforcement_sources"],
  data_sources: {} as Record<string, unknown>,
  scanned_programs: [] as string[],
  source_feeds: [] as SourceFeed[],
  source_candidates: [] as SourceCandidate[],
  dimensions: [] as Dimension[],
  triage_results: [] as TriageResult[],
  research_sessions: [] as ResearchSession[],
  threshold: 3,
  qualityMap: {} as Record<string, Quality>,
  loading: true,
  error: null as string | null,
  apiAvailable: false,
  _token: "",
};

// ── Source CRUD action factory ───────────────────────────────────────────

function createSourceActions(set: SetFn, get: GetFn, refreshSources: () => Promise<void>) {
  return {
    uploadSourceDocument: async (sourceId: string, file: File) => {
      if (!get().apiAvailable) throw new Error("API not available");
      const buffer = await file.arrayBuffer();
      const bytes = new Uint8Array(buffer);
      let binary = "";
      for (let i = 0; i < bytes.byteLength; i++) binary += String.fromCharCode(bytes[i]);
      const result = await apiPost("/enforcement-sources/upload", { source_id: sourceId, filename: file.name, content: btoa(binary) });
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
  };
}

// ── Store ────────────────────────────────────────────────────────────────

export const usePipelineStore = create<PipelineStore>((set, get) => {
  const fetchDashboard = async () => {
    set({ loading: true, error: null });
    try {
      const res = await apiGet("/dashboard", { signal: AbortSignal.timeout(10000) });
      if (!res.ok) throw new Error(`API returned ${res.status}`);
      const incoming = buildIncomingData(await res.json());
      const diff = changedSlices(get(), incoming);
      set({ apiAvailable: true, loading: false, ...(diff || {}) });
    } catch (err) {
      const message = (err as Error).message || "Unknown error";
      console.error("SVAP API unreachable:", message);
      set({ error: `Unable to reach the SVAP API. ${message}`, apiAvailable: false, loading: false });
    }
  };

  const refreshSources = async () => {
    const res = await apiGet("/enforcement-sources");
    if (res.ok) set({ enforcement_sources: await res.json() });
  };

  return {
    ...INITIAL_STATE,

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

    ...createSourceActions(set, get, refreshSources),
    ...createDiscoveryActions(set, get),
  };
});
