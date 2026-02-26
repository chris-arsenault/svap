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

// ── Data selectors (re-render only when the selected slice changes) ──

export const useQuality = (id: string) => usePipelineStore((s) => s.qualityMap[id]);
export const usePipelineStatus = () => usePipelineStore((s) => s.pipeline_status);
export const useCounts = () => usePipelineStore((s) => s.counts);
export const useLoading = () => usePipelineStore((s) => s.loading);
export const useError = () => usePipelineStore((s) => s.error);
export const useApiAvailable = () => usePipelineStore((s) => s.apiAvailable);
