// Pipeline stage status
export type StageStatus = "idle" | "pending" | "running" | "completed" | "pending_review";
export type RiskLevel = "critical" | "high" | "medium" | "low";
export type ViewId =
  | "dashboard"
  | "sources"
  | "cases"
  | "policies"
  | "taxonomy"
  | "matrix"
  | "predictions"
  | "detection";

export interface PipelineStageStatus {
  stage: number;
  status: StageStatus;
}

export interface Counts {
  cases: number;
  taxonomy_qualities: number;
  policies: number;
  predictions: number;
  detection_patterns: number;
}

export interface Case {
  case_id: string;
  case_name: string;
  scale_dollars?: number;
  detection_method: string;
  qualities: string[];
  scheme_mechanics: string;
  exploited_policy: string;
  enabling_condition: string;
}

export interface Quality {
  quality_id: string;
  name: string;
  definition: string;
  color: string;
  case_count: number;
  recognition_test: string;
  exploitation_logic: string;
}

export interface Policy {
  policy_id: string;
  name: string;
  convergence_score: number;
  risk_level: RiskLevel;
  qualities: string[];
}

export interface Prediction {
  prediction_id: string;
  policy_id: string;
  policy_name: string;
  convergence_score: number;
  lifecycle_stage: string;
  detection_difficulty: string;
  mechanics: string;
  enabling_qualities: string[];
  actor_profile: string;
}

export interface DetectionPattern {
  pattern_id: string;
  priority: RiskLevel;
  policy_name: string;
  anomaly_signal: string;
  detection_latency: string;
  data_source: string;
  baseline: string;
  false_positive_risk: string;
}

export type ValidationStatus = "pending" | "valid" | "invalid" | "error";

export interface EnforcementSource {
  source_id: string;
  name: string;
  description: string;
  url: string | null;
  source_type: string;
  has_document: boolean;
  s3_key: string | null;
  doc_id: string | null;
  summary: string | null;
  validation_status: ValidationStatus;
  created_at: string;
  updated_at: string;
}

// Raw data from API/fallback (before computed fields)
export interface FallbackData {
  run_id: string;
  source: "api" | "static";
  pipeline_status: PipelineStageStatus[];
  counts: Counts;
  calibration: { threshold: number };
  cases: Case[];
  taxonomy: Quality[];
  policies: Policy[];
  predictions: Prediction[];
  detection_patterns: DetectionPattern[];
  case_convergence: unknown[];
  policy_convergence: unknown[];
  policy_catalog: Record<string, unknown>;
  enforcement_sources: EnforcementSource[];
  data_sources: Record<string, unknown>;
  scanned_programs: string[];
}

// Full pipeline data with computed fields and actions
export interface PipelineData extends FallbackData {
  threshold: number;
  qualityMap: Record<string, Quality>;
  loading: boolean;
  error: string | null;
  apiAvailable: boolean;
  refresh: () => Promise<void>;
  runPipeline: () => Promise<unknown>;
  approveStage: (stage: number) => Promise<unknown>;
  seedPipeline: () => Promise<unknown>;
  uploadSourceDocument: (sourceId: string, file: File) => Promise<unknown>;
  createSource: (source: { name: string; url?: string; description?: string }) => Promise<unknown>;
  deleteSource: (sourceId: string) => Promise<unknown>;
}

// Common view props
export interface ViewProps {
  onNavigate: (view: ViewId) => void;
}
