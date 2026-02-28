// Pipeline stage status
export type StageStatus = "idle" | "pending" | "running" | "completed" | "failed" | "pending_review";
export type RiskLevel = "critical" | "high" | "medium" | "low";
export interface PipelineStageStatus {
  stage: number;
  status: StageStatus;
  error_message?: string | null;
}

export interface Counts {
  cases: number;
  taxonomy_qualities: number;
  policies: number;
  exploitation_trees: number;
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

export interface ExploitationStep {
  step_id: string;
  tree_id: string;
  step_order: number;
  title: string;
  description: string;
  actor_action: string;
  is_branch_point: boolean;
  branch_label: string | null;
  parent_step_id: string | null;
  enabling_qualities: string[];
  policy_id?: string;
  policy_name?: string;
}

export interface ExploitationTree {
  tree_id: string;
  policy_id: string;
  policy_name: string;
  convergence_score: number;
  actor_profile: string;
  lifecycle_stage: string;
  detection_difficulty: string;
  review_status: string;
  step_count: number;
  steps: ExploitationStep[];
}

export interface DetectionPattern {
  pattern_id: string;
  step_id: string;
  step_title: string;
  tree_id: string;
  priority: RiskLevel;
  policy_name: string;
  anomaly_signal: string;
  detection_latency: string;
  data_source: string;
  baseline: string;
  false_positive_risk: string;
  implementation_notes: string;
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
  exploitation_trees: ExploitationTree[];
  detection_patterns: DetectionPattern[];
  case_convergence: unknown[];
  policy_convergence: unknown[];
  policy_catalog: Record<string, unknown>;
  enforcement_sources: EnforcementSource[];
  data_sources: Record<string, unknown>;
  scanned_programs: string[];
}

// ── Discovery & Research types ───────────────────────────────────────────

export interface SourceFeed {
  feed_id: string;
  name: string;
  listing_url: string;
  content_type: string;
  last_checked_at: string | null;
  enabled: boolean;
}

export interface SourceCandidate {
  candidate_id: string;
  feed_id: string;
  title: string;
  url: string;
  status: string;
  richness_score: number | null;
  richness_rationale: string | null;
  estimated_cases: number | null;
  discovered_at: string;
}

export interface Dimension {
  dimension_id: string;
  name: string;
  definition: string;
  probing_questions: string[];
  origin: string;
  related_quality_ids: string[];
}

export interface TriageResult {
  policy_id: string;
  triage_score: number;
  rationale: string;
  uncertainty: string;
  priority_rank: number;
}

export interface StructuralFinding {
  finding_id: string;
  policy_id: string;
  dimension_id: string;
  observation: string;
  source_type: string;
  source_citation: string;
  confidence: string;
  status: string;
}

export interface QualityAssessment {
  assessment_id: string;
  policy_id: string;
  quality_id: string;
  present: "yes" | "no" | "uncertain";
  evidence_finding_ids: string[];
  confidence: string;
  rationale: string;
}

export interface ResearchSession {
  session_id: string;
  run_id: string;
  policy_id: string;
  status: string;
  sources_queried: unknown[];
  started_at: string | null;
  completed_at: string | null;
}

