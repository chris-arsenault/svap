/**
 * Convenience selector hooks for the pipeline store.
 *
 * Action selectors return stable references and never cause re-renders.
 * Data selectors subscribe to a single value — the component only
 * re-renders when that specific slice changes.
 */

import { usePipelineStore } from "./pipelineStore";

// ── Action selectors (stable refs, never trigger re-renders) ─────────

export const useRefresh = () => usePipelineStore((s) => s.refresh);
export const useRunPipeline = () => usePipelineStore((s) => s.runPipeline);
export const useApproveStage = () => usePipelineStore((s) => s.approveStage);
export const useSeedPipeline = () => usePipelineStore((s) => s.seedPipeline);
export const useUploadSourceDocument = () => usePipelineStore((s) => s.uploadSourceDocument);
export const useCreateSource = () => usePipelineStore((s) => s.createSource);
export const useDeleteSource = () => usePipelineStore((s) => s.deleteSource);
export const useFetchDiscovery = () => usePipelineStore((s) => s.fetchDiscovery);
export const useRunDiscoveryFeeds = () => usePipelineStore((s) => s.runDiscoveryFeeds);
export const useReviewCandidate = () => usePipelineStore((s) => s.reviewCandidate);
export const useFetchDimensions = () => usePipelineStore((s) => s.fetchDimensions);
export const useFetchTriageResults = () => usePipelineStore((s) => s.fetchTriageResults);
export const useRunTriage = () => usePipelineStore((s) => s.runTriage);
export const useRunDeepResearch = () => usePipelineStore((s) => s.runDeepResearch);
export const useFetchResearchSessions = () => usePipelineStore((s) => s.fetchResearchSessions);
export const useFetchFindings = () => usePipelineStore((s) => s.fetchFindings);
export const useFetchAssessments = () => usePipelineStore((s) => s.fetchAssessments);
export const useUpdatePipelineStatus = () => usePipelineStore((s) => s.updatePipelineStatus);

// ── Data selectors (re-render only when the selected slice changes) ──

export const useQuality = (id: string) => usePipelineStore((s) => s.qualityMap[id]);
export const usePipelineStatus = () => usePipelineStore((s) => s.pipeline_status);
export const useCounts = () => usePipelineStore((s) => s.counts);
export const useLoading = () => usePipelineStore((s) => s.loading);
export const useError = () => usePipelineStore((s) => s.error);
export const useApiAvailable = () => usePipelineStore((s) => s.apiAvailable);
export const useRunId = () => usePipelineStore((s) => s.run_id);
export const useCases = () => usePipelineStore((s) => s.cases);
export const useTaxonomy = () => usePipelineStore((s) => s.taxonomy);
export const usePolicies = () => usePipelineStore((s) => s.policies);
export const useThreshold = () => usePipelineStore((s) => s.threshold);
export const useExploitationTrees = () => usePipelineStore((s) => s.exploitation_trees);
export const useDetectionPatterns = () => usePipelineStore((s) => s.detection_patterns);
export const useEnforcementSources = () => usePipelineStore((s) => s.enforcement_sources);
export const useSourceFeeds = () => usePipelineStore((s) => s.source_feeds);
export const useSourceCandidates = () => usePipelineStore((s) => s.source_candidates);
export const useDimensions = () => usePipelineStore((s) => s.dimensions);
export const useTriageResults = () => usePipelineStore((s) => s.triage_results);
export const useResearchSessions = () => usePipelineStore((s) => s.research_sessions);
export const usePolicyCatalog = () => usePipelineStore((s) => s.policy_catalog);
export const useScannedPrograms = () => usePipelineStore((s) => s.scanned_programs);
export const useDataSources = () => usePipelineStore((s) => s.data_sources);
