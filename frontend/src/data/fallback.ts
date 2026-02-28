import type { FallbackData } from "../types";

export const FALLBACK_DATA: FallbackData = {
  run_id: "fallback",
  source: "static",
  pipeline_status: [
    { stage: 0, status: "idle" },
    { stage: 1, status: "idle" },
    { stage: 2, status: "idle" },
    { stage: 3, status: "idle" },
    { stage: 4, status: "idle" },
    { stage: 5, status: "idle" },
    { stage: 6, status: "idle" },
  ],
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
};
