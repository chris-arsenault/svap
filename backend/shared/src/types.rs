//! Shared types for the SVAP pipeline.
//!
//! Every struct here maps 1:1 to a database table row or an API response shape.
//! LLM response structs are kept separate from DB row structs where the shapes differ.

use serde::{Deserialize, Serialize};

// ── Pipeline Runs ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineRun {
    pub run_id: String,
    pub created_at: String,
    pub config_snapshot: String,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunSummary {
    pub run_id: String,
    pub created_at: String,
    pub notes: Option<String>,
    pub stages: Vec<StageStatusEntry>,
}

// ── Stage Log ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageStatusEntry {
    pub stage: i32,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
}

// ── Cases ─────���────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Case {
    pub case_id: String,
    pub source_doc_id: Option<String>,
    pub case_name: String,
    pub scheme_mechanics: String,
    pub exploited_policy: String,
    pub enabling_condition: String,
    pub scale_dollars: Option<f64>,
    pub scale_defendants: Option<i32>,
    pub scale_duration: Option<String>,
    pub detection_method: Option<String>,
    pub raw_extraction: Option<serde_json::Value>,
    pub created_at: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub qualities: Vec<String>,
}

// ── Taxonomy ─────────���─────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaxonomyQuality {
    pub quality_id: String,
    pub name: String,
    pub definition: String,
    pub recognition_test: String,
    pub exploitation_logic: String,
    pub canonical_examples: Option<serde_json::Value>,
    pub review_status: Option<String>,
    pub reviewer_notes: Option<String>,
    pub created_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub case_count: Option<i64>,
}

// ── Convergence Scores ��────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConvergenceRow {
    pub case_name: String,
    pub case_id: String,
    pub scale_dollars: Option<f64>,
    pub quality_id: String,
    pub present: bool,
    pub evidence: Option<String>,
}

// ── Calibration ─────────���──────────────────────────────���───────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Calibration {
    pub run_id: Option<String>,
    pub threshold: i32,
    pub correlation_notes: Option<String>,
    pub quality_frequency: Option<String>,
    pub quality_combinations: Option<String>,
    pub created_at: String,
}

// ── Policies ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Policy {
    pub policy_id: String,
    pub name: String,
    pub description: Option<String>,
    pub source_document: Option<String>,
    pub structural_characterization: Option<String>,
    pub created_at: String,
    pub lifecycle_status: Option<String>,
    pub lifecycle_updated_at: Option<String>,
    // Enriched fields (not in DB row)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub qualities: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub convergence_score: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub risk_level: Option<String>,
}

// ── Policy Scores ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyScore {
    pub name: String,
    pub policy_id: String,
    pub quality_id: String,
    pub present: bool,
    pub evidence: Option<String>,
}

// ── Exploitation Trees ─────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExploitationTree {
    pub tree_id: String,
    pub policy_id: String,
    pub convergence_score: i32,
    pub actor_profile: Option<String>,
    pub lifecycle_stage: Option<String>,
    pub detection_difficulty: Option<String>,
    pub review_status: Option<String>,
    pub reviewer_notes: Option<String>,
    pub run_id: Option<String>,
    pub created_at: String,
    pub policy_name: Option<String>,
    pub step_count: Option<i64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub steps: Vec<ExploitationStep>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExploitationStep {
    pub step_id: String,
    pub tree_id: String,
    pub parent_step_id: Option<String>,
    pub step_order: i32,
    pub title: String,
    pub description: String,
    pub actor_action: Option<String>,
    pub is_branch_point: Option<bool>,
    pub branch_label: Option<String>,
    pub created_at: String,
    pub policy_id: Option<String>,
    pub policy_name: Option<String>,
    #[serde(default)]
    pub enabling_qualities: Vec<String>,
}

// ── Detection Patterns ───────────────────��─────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectionPattern {
    pub pattern_id: String,
    pub run_id: String,
    pub step_id: Option<String>,
    pub prediction_id: Option<String>,
    pub data_source: String,
    pub anomaly_signal: String,
    pub baseline: Option<String>,
    pub false_positive_risk: Option<String>,
    pub detection_latency: Option<String>,
    pub priority: Option<String>,
    pub implementation_notes: Option<String>,
    pub created_at: String,
    pub step_title: Option<String>,
    pub tree_id: Option<String>,
    pub policy_name: Option<String>,
}

// ── Documents (RAG) ──────���─────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    pub doc_id: String,
    pub filename: Option<String>,
    pub doc_type: Option<String>,
    pub full_text: String,
    pub metadata: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chunk {
    pub chunk_id: String,
    pub doc_id: String,
    pub chunk_index: i32,
    pub text: String,
    pub token_count: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doc_type: Option<String>,
}

// ── Enforcement Sources ─────���──────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnforcementSource {
    pub source_id: String,
    pub name: String,
    pub url: Option<String>,
    pub source_type: String,
    pub description: Option<String>,
    pub has_document: bool,
    pub s3_key: Option<String>,
    pub doc_id: Option<String>,
    pub summary: Option<String>,
    pub validation_status: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub candidate_id: Option<String>,
    pub feed_id: Option<String>,
}

// ── Dimensions ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dimension {
    pub dimension_id: String,
    pub name: String,
    pub definition: String,
    pub probing_questions: Option<String>,
    pub origin: String,
    pub related_quality_ids: Option<String>,
    pub created_at: String,
    pub created_by: Option<String>,
}

// ── Structural Findings ────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructuralFinding {
    pub finding_id: String,
    pub run_id: String,
    pub policy_id: String,
    pub dimension_id: Option<String>,
    pub observation: String,
    pub source_type: String,
    pub source_citation: Option<String>,
    pub source_text: Option<String>,
    pub confidence: String,
    pub status: String,
    pub stale_reason: Option<String>,
    pub created_at: String,
    pub created_by: Option<String>,
    pub dimension_name: Option<String>,
}

// ── Quality Assessments ────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityAssessment {
    pub assessment_id: String,
    pub run_id: String,
    pub policy_id: String,
    pub quality_id: String,
    pub taxonomy_version: Option<String>,
    pub present: String,
    pub evidence_finding_ids: Option<String>,
    pub confidence: String,
    pub rationale: Option<String>,
    pub created_at: String,
}

// ── Source Feeds ────────────────────────────────────��───────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceFeed {
    pub feed_id: String,
    pub name: String,
    pub listing_url: String,
    pub content_type: String,
    pub link_selector: Option<String>,
    pub last_checked_at: Option<String>,
    pub last_entry_url: Option<String>,
    pub enabled: Option<bool>,
    pub created_at: String,
    pub updated_at: String,
}

// ── Source Candidates ────���─────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceCandidate {
    pub candidate_id: String,
    pub feed_id: Option<String>,
    pub title: String,
    pub url: String,
    pub discovered_at: String,
    pub published_date: Option<String>,
    pub status: String,
    pub richness_score: Option<f64>,
    pub richness_rationale: Option<String>,
    pub estimated_cases: Option<i32>,
    pub source_id: Option<String>,
    pub doc_id: Option<String>,
    pub reviewed_by: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

// ── Triage Results ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriageResult {
    pub policy_id: String,
    pub triage_score: f64,
    pub rationale: String,
    pub uncertainty: Option<String>,
    pub priority_rank: i32,
    pub policy_name: Option<String>,
    pub run_id: Option<String>,
}

// ── Research Sessions ──────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchSession {
    pub session_id: String,
    pub run_id: String,
    pub policy_id: String,
    pub status: String,
    pub sources_queried: Option<String>,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub error_message: Option<String>,
    pub trigger: Option<String>,
}

// ── Regulatory Sources ─────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegulatorySource {
    pub source_id: String,
    pub source_type: String,
    pub url: String,
    pub title: Option<String>,
    pub cfr_reference: Option<String>,
    pub full_text: String,
    pub fetched_at: String,
    pub metadata: Option<String>,
}

// ── LLM Response Types ─────────────────────────────────────────────────

/// Stage 1 LLM response: extracted case
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmCaseExtraction {
    pub case_name: Option<String>,
    pub scheme_mechanics: Option<String>,
    pub exploited_policy: Option<String>,
    pub enabling_condition: Option<String>,
    pub scale_dollars: Option<serde_json::Value>,
    pub scale_defendants: Option<i32>,
    pub scale_duration: Option<String>,
    pub detection_method: Option<String>,
}

/// Stage 2 LLM response: clustered quality draft
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmQualityDraft {
    pub name: Option<String>,
    pub definition: Option<String>,
    pub enabling_conditions: Option<Vec<String>>,
}

/// Stage 2 LLM response: refined quality
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmQualityRefined {
    pub name: Option<String>,
    pub definition: Option<String>,
    pub recognition_test: Option<String>,
    pub exploitation_logic: Option<String>,
    pub canonical_examples: Option<Vec<String>>,
}

/// Stage 2 LLM response: dedup check
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmDedupResult {
    #[serde(rename = "match")]
    pub is_match: Option<bool>,
    pub existing_quality_id: Option<String>,
}

/// Stage 3 LLM response: case scoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmScoreEntry {
    pub present: Option<bool>,
    pub evidence: Option<String>,
}

/// Stage 3 LLM response: calibration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmCalibration {
    pub threshold: Option<i32>,
    pub correlation_notes: Option<String>,
}

/// Stage 4a LLM response: triage ranking entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmTriageEntry {
    pub policy_name: Option<String>,
    pub score: Option<f64>,
    pub rationale: Option<String>,
    pub uncertainty: Option<String>,
}

/// Stage 4b LLM response: research plan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmResearchPlan {
    pub ecfr_queries: Option<Vec<LlmEcfrQuery>>,
    pub fr_searches: Option<Vec<LlmFrSearch>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmEcfrQuery {
    pub title: Option<i32>,
    pub part: Option<String>,
    pub subpart: Option<String>,
    pub rationale: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmFrSearch {
    pub term: Option<String>,
}

/// Stage 4b LLM response: extracted finding
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmFinding {
    pub dimension_id: Option<String>,
    pub observation: Option<String>,
    pub source_citation: Option<String>,
    pub source_text_excerpt: Option<String>,
    pub confidence: Option<String>,
}

/// Stage 4c LLM response: quality assessment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmAssessment {
    pub present: Option<String>,
    pub finding_ids: Option<Vec<String>>,
    pub confidence: Option<String>,
    pub reasoning: Option<String>,
}

/// Stage 5 LLM response: exploitation tree
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmExploitationTree {
    pub actor_profile: Option<String>,
    pub lifecycle_stage: Option<String>,
    pub detection_difficulty: Option<String>,
    pub steps: Option<Vec<LlmExploitationStep>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmExploitationStep {
    pub step_order: i32,
    pub title: String,
    pub description: Option<String>,
    pub actor_action: Option<String>,
    pub parent_step_order: Option<i32>,
    pub is_branch_point: Option<bool>,
    pub branch_label: Option<String>,
    pub enabling_qualities: Option<Vec<String>>,
}

/// Stage 6 LLM response: detection pattern
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmDetectionPattern {
    pub data_source: Option<String>,
    pub anomaly_signal: Option<String>,
    pub baseline: Option<String>,
    pub false_positive_risk: Option<String>,
    pub detection_latency: Option<String>,
    pub priority: Option<String>,
    pub implementation_notes: Option<String>,
}

/// Stage 0a LLM response: link extraction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmExtractedLink {
    pub url: Option<String>,
    pub title: Option<String>,
    pub text: Option<String>,
    pub published_date: Option<String>,
}

/// Stage 0a LLM response: richness evaluation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmRichnessEval {
    pub richness_score: Option<f64>,
    pub rationale: Option<String>,
    pub estimated_cases: Option<i32>,
    pub scheme_types: Option<Vec<String>>,
}

/// Stage 0 LLM response: document validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmValidation {
    pub summary: Option<String>,
    pub is_valid: Option<bool>,
}

// ── Config ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub bedrock: BedrockConfig,
    #[serde(default)]
    pub rag: RagConfig,
    #[serde(default)]
    pub pipeline: PipelineConfig,
    #[serde(default)]
    pub storage: Option<StorageConfig>,
    #[serde(default)]
    pub discovery: Option<DiscoveryConfig>,
    #[serde(default)]
    pub research: Option<ResearchConfig>,
    /// Internal deadline for stage execution (epoch seconds).
    #[serde(skip)]
    pub deadline: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BedrockConfig {
    #[serde(default = "default_region")]
    pub region: String,
    #[serde(default = "default_model_id")]
    pub model_id: String,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: i32,
    #[serde(default = "default_temperature")]
    pub temperature: f64,
    #[serde(default = "default_retry_attempts")]
    pub retry_attempts: u32,
    #[serde(default = "default_retry_delay")]
    pub retry_delay_seconds: u64,
}

fn default_region() -> String {
    "us-east-1".to_string()
}
fn default_model_id() -> String {
    "us.anthropic.claude-sonnet-4-6".to_string()
}
fn default_max_tokens() -> i32 {
    4096
}
fn default_temperature() -> f64 {
    0.2
}
fn default_retry_attempts() -> u32 {
    3
}
fn default_retry_delay() -> u64 {
    5
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagConfig {
    #[serde(default = "default_chunk_size")]
    pub chunk_size: usize,
    #[serde(default = "default_chunk_overlap")]
    pub chunk_overlap: usize,
    #[serde(default = "default_max_context_chunks")]
    pub max_context_chunks: usize,
    pub embedding_model: Option<String>,
}

impl Default for RagConfig {
    fn default() -> Self {
        Self {
            chunk_size: 1500,
            chunk_overlap: 200,
            max_context_chunks: 10,
            embedding_model: None,
        }
    }
}

fn default_chunk_size() -> usize {
    1500
}
fn default_chunk_overlap() -> usize {
    200
}
fn default_max_context_chunks() -> usize {
    10
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineConfig {
    #[serde(default = "default_human_gates")]
    pub human_gates: Vec<i32>,
    #[serde(default = "default_max_concurrency")]
    pub max_concurrency: usize,
    #[serde(default = "default_export_dir")]
    pub export_dir: String,
}

impl Default for PipelineConfig {
    fn default() -> Self {
        Self {
            human_gates: vec![2, 5],
            max_concurrency: 5,
            export_dir: "/tmp/results".to_string(),
        }
    }
}

fn default_human_gates() -> Vec<i32> {
    vec![2, 5]
}
fn default_max_concurrency() -> usize {
    5
}
fn default_export_dir() -> String {
    "/tmp/results".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    pub database_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryConfig {
    pub max_candidates_per_feed: Option<usize>,
    pub richness_accept_threshold: Option<f64>,
    pub richness_review_threshold: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchConfig {
    pub top_n: Option<usize>,
}

// ── API Request/Response Types ─────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusResponse {
    pub run_id: String,
    pub stages: Vec<StageStatusEntry>,
    pub counts: CorpusCounts,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorpusCounts {
    pub cases: i64,
    pub taxonomy_qualities: i64,
    pub policies: i64,
    pub exploitation_trees: i64,
    pub detection_patterns: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunPipelineRequest {
    pub config_overrides: Option<serde_json::Value>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApproveStageRequest {
    pub stage: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateEnforcementSourceRequest {
    pub source_id: Option<String>,
    pub name: String,
    pub url: Option<String>,
    pub source_type: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadDocumentRequest {
    pub source_id: String,
    pub filename: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteSourceRequest {
    pub source_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateFeedRequest {
    pub name: String,
    pub listing_url: String,
    pub content_type: Option<String>,
    pub link_selector: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewCandidateRequest {
    pub candidate_id: String,
    pub action: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeepResearchRequest {
    pub policy_ids: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StopExecutionRequest {
    pub execution_arn: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteRunRequest {
    pub run_id: String,
}

/// Stage runner event from Step Functions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageRunnerEvent {
    pub run_id: String,
    pub stage: i32,
    #[serde(default)]
    pub config_overrides: Option<serde_json::Value>,
    #[serde(default)]
    pub gate: Option<bool>,
    #[serde(default)]
    pub task_token: Option<String>,
    /// Step Functions wraps under Payload key
    #[serde(rename = "Payload")]
    pub payload: Option<Box<StageRunnerEvent>>,
}
