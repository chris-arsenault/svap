//! Stage 4B: Deep Structural Research
//!
//! Per-policy investigation using regulatory sources (eCFR, Federal Register).

use serde_json::json;
use sha2::{Digest, Sha256};
use tokio_postgres::Client;
use tracing::{error, info, warn};

use crate::bedrock::BedrockClient;
use crate::db;
use crate::types::{Config, Dimension, Policy, RegulatorySource, StructuralFinding, TriageResult};

const RESEARCH_PLAN_SYSTEM: &str = "You are a regulatory research planner. You identify which specific sections of the Code of Federal Regulations should be consulted.";
const FINDING_EXTRACTION_SYSTEM: &str = "You are extracting factual structural observations from regulatory text. Each finding must be a single atomic observation about how a policy works mechanically.";

const STAGE4B_PLAN_PROMPT: &str = include_str!("../../prompts/stage4b_plan_research.txt");
const STAGE4B_EXTRACT_PROMPT: &str = include_str!("../../prompts/stage4b_extract_findings.txt");
type StageResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

struct ResearchContext<'a> {
    db_client: &'a Client,
    bedrock: &'a BedrockClient,
    run_id: &'a str,
    dimensions_text: &'a str,
    strip_re: &'a regex::Regex,
}

struct ResearchRun {
    dimensions_text: String,
    all_policies: Vec<Policy>,
    delta_entries: Vec<(TriageResult, String)>,
    skipped: usize,
}

enum ResearchPreparation {
    NoTriage,
    Unchanged { skipped: usize },
    Run(ResearchRun),
}

fn compute_hash(parts: &[&str]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(parts.join("|"));
    format!("{:x}", hasher.finalize())[..12].to_string()
}

pub async fn run(
    db_client: &Client,
    bedrock: &BedrockClient,
    run_id: &str,
    config: &Config,
) -> StageResult<serde_json::Value> {
    info!("Stage 4B: Deep Structural Research");
    db::log_stage_start(db_client, run_id, 41).await?;

    match run_inner(db_client, bedrock, run_id, config).await {
        Ok(result) => {
            db::log_stage_complete(db_client, run_id, 41, Some(&result)).await?;
            Ok(result)
        }
        Err(e) => {
            db::log_stage_failed(db_client, run_id, 41, &e.to_string()).await?;
            Err(e)
        }
    }
}

async fn run_inner(
    db_client: &Client,
    bedrock: &BedrockClient,
    run_id: &str,
    config: &Config,
) -> StageResult<serde_json::Value> {
    match prepare_research(db_client, config).await? {
        ResearchPreparation::NoTriage => {
            info!("No policies to research. Run triage first.");
            Ok(json!({"policies_researched": 0}))
        }
        ResearchPreparation::Unchanged { skipped } => {
            info!("All policies unchanged. Skipping research.");
            Ok(json!({"policies_researched": 0, "skipped": skipped}))
        }
        ResearchPreparation::Run(research_run) => {
            execute_research(db_client, bedrock, run_id, research_run).await
        }
    }
}

async fn prepare_research(db_client: &Client, config: &Config) -> StageResult<ResearchPreparation> {
    let dimensions = db::get_dimensions(db_client).await?;
    let top_n = config.research.as_ref().and_then(|r| r.top_n).unwrap_or(10);

    let triage = db::get_triage_results(db_client).await?;
    let policies_to_research: Vec<_> = triage.iter().take(top_n).cloned().collect();

    if policies_to_research.is_empty() {
        return Ok(ResearchPreparation::NoTriage);
    }

    let dim_fp = dimension_fingerprint(&dimensions);
    let stored_hashes = db::get_processing_hashes(db_client, 41).await?;
    let all_policies = db::get_policies(db_client).await?;
    let (delta_entries, skipped) = changed_research_entries(
        &policies_to_research,
        &all_policies,
        &stored_hashes,
        &dim_fp,
    );

    if delta_entries.is_empty() {
        return Ok(ResearchPreparation::Unchanged { skipped });
    }

    Ok(ResearchPreparation::Run(ResearchRun {
        dimensions_text: dimensions_text(&dimensions),
        all_policies,
        delta_entries,
        skipped,
    }))
}

async fn execute_research(
    db_client: &Client,
    bedrock: &BedrockClient,
    run_id: &str,
    research_run: ResearchRun,
) -> StageResult<serde_json::Value> {
    info!(
        "Researching {} policies ({} unchanged)...",
        research_run.delta_entries.len(),
        research_run.skipped
    );
    let mut researched = 0;
    let strip_re = regex::Regex::new(r"<[^>]+>").unwrap();
    let context = ResearchContext {
        db_client,
        bedrock,
        run_id,
        dimensions_text: &research_run.dimensions_text,
        strip_re: &strip_re,
    };

    for (entry, h) in &research_run.delta_entries {
        if research_policy(&context, entry, h, &research_run.all_policies).await? {
            researched += 1;
        }
    }

    info!("Research complete: {} policies.", researched);
    Ok(json!({"policies_researched": researched, "skipped": research_run.skipped}))
}

fn dimension_fingerprint(dimensions: &[Dimension]) -> String {
    let mut ids: Vec<&str> = dimensions.iter().map(|d| d.dimension_id.as_str()).collect();
    ids.sort();
    compute_hash(&[&ids.join(":")])
}

fn changed_research_entries(
    entries: &[TriageResult],
    policies: &[Policy],
    stored_hashes: &std::collections::HashMap<String, String>,
    dim_fp: &str,
) -> (Vec<(TriageResult, String)>, usize) {
    let mut changed = Vec::new();
    let mut skipped = 0;
    for entry in entries {
        let hash = research_hash(entry, policies, dim_fp);
        if stored_hashes.get(&entry.policy_id).map(|s| s.as_str()) == Some(&hash) {
            skipped += 1;
        } else {
            changed.push((entry.clone(), hash));
        }
    }
    (changed, skipped)
}

fn research_hash(entry: &TriageResult, policies: &[Policy], dim_fp: &str) -> String {
    let description = policies
        .iter()
        .find(|p| p.policy_id == entry.policy_id)
        .and_then(|p| p.description.as_deref())
        .unwrap_or("");
    compute_hash(&[description, &entry.triage_score.to_string(), dim_fp])
}

fn dimensions_text(dimensions: &[Dimension]) -> String {
    dimensions
        .iter()
        .map(|d| {
            format!(
                "- {}: {} -- {}",
                d.dimension_id,
                d.name,
                &d.definition[..d.definition.len().min(100)]
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

async fn research_policy(
    context: &ResearchContext<'_>,
    entry: &TriageResult,
    hash: &str,
    all_policies: &[Policy],
) -> StageResult<bool> {
    let Some(policy) = all_policies.iter().find(|p| p.policy_id == entry.policy_id) else {
        warn!("Policy {} not found, skipping", entry.policy_id);
        return Ok(false);
    };

    info!("Researching: {}", policy.name);
    let session_id = create_session_id(context.run_id, &entry.policy_id);
    start_research_session(context.db_client, context.run_id, entry, &session_id).await?;
    let plan = plan_research(context.bedrock, policy, context.dimensions_text).await?;
    process_ecfr_queries(context, entry, policy, &plan).await?;
    finish_research_session(context.db_client, context.run_id, entry, hash, &session_id).await?;
    Ok(true)
}

fn create_session_id(run_id: &str, policy_id: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(format!("{}:{}", run_id, policy_id));
    format!("{:x}", hasher.finalize())[..12].to_string()
}

async fn start_research_session(
    db_client: &Client,
    run_id: &str,
    entry: &TriageResult,
    session_id: &str,
) -> StageResult<()> {
    db::create_research_session(db_client, run_id, &entry.policy_id, session_id).await?;
    db::update_research_session(db_client, session_id, "researching", None, None).await?;
    db::update_policy_lifecycle(db_client, &entry.policy_id, "research_in_progress").await
}

async fn plan_research(
    bedrock: &BedrockClient,
    policy: &Policy,
    dimensions_text: &str,
) -> StageResult<serde_json::Value> {
    let prompt = BedrockClient::render_prompt(
        STAGE4B_PLAN_PROMPT,
        &[
            ("policy_name", &policy.name),
            (
                "policy_description",
                policy.description.as_deref().unwrap_or(""),
            ),
            ("dimensions", dimensions_text),
            (
                "known_cfr_references",
                "None pre-mapped. Use your knowledge of federal healthcare regulations.",
            ),
        ],
    );
    bedrock
        .invoke_json(&prompt, RESEARCH_PLAN_SYSTEM, None, Some(2000))
        .await
}

async fn process_ecfr_queries(
    context: &ResearchContext<'_>,
    entry: &TriageResult,
    policy: &Policy,
    plan: &serde_json::Value,
) -> StageResult<()> {
    let Some(ecfr_queries) = plan.get("ecfr_queries").and_then(|e| e.as_array()) else {
        return Ok(());
    };
    for ecfr_ref in ecfr_queries {
        process_ecfr_query(context, entry, policy, ecfr_ref).await?;
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    }
    Ok(())
}

async fn process_ecfr_query(
    context: &ResearchContext<'_>,
    entry: &TriageResult,
    policy: &Policy,
    ecfr_ref: &serde_json::Value,
) -> StageResult<()> {
    let Some((title, part)) = ecfr_reference(ecfr_ref) else {
        return Ok(());
    };
    let Some(reg_text) = regulatory_text(context.db_client, title, &part).await? else {
        return Ok(());
    };
    let clean_text = context.strip_re.replace_all(&reg_text, " ").to_string();
    if clean_text.len() < 100 {
        return Ok(());
    }
    extract_and_store_findings(context, entry, policy, title, &part, &clean_text).await
}

fn ecfr_reference(ecfr_ref: &serde_json::Value) -> Option<(i64, String)> {
    let title = ecfr_ref.get("title").and_then(|t| t.as_i64()).unwrap_or(42);
    let part = ecfr_ref.get("part").and_then(|p| p.as_str())?;
    Some((title, part.to_string()))
}

async fn regulatory_text(
    db_client: &Client,
    title: i64,
    part: &str,
) -> StageResult<Option<String>> {
    let source_id = format!("ecfr_t{}_p{}", title, part);
    if let Some(cached) = db::get_regulatory_source(db_client, &source_id).await? {
        return Ok(Some(cached.full_text));
    }
    fetch_regulatory_text(db_client, title, part, &source_id).await
}

async fn fetch_regulatory_text(
    db_client: &Client,
    title: i64,
    part: &str,
    source_id: &str,
) -> StageResult<Option<String>> {
    let url = format!(
        "https://www.ecfr.gov/api/versioner/v1/full/{}/title-{}.xml?part={}",
        chrono::Utc::now().format("%Y-%m-%d"),
        title,
        part
    );
    let resp = match reqwest::get(&url).await {
        Ok(resp) => resp,
        Err(e) => {
            error!("Failed to fetch eCFR: {}", e);
            return Ok(None);
        }
    };
    let text = resp.text().await.unwrap_or_default();
    let source = RegulatorySource {
        source_id: source_id.to_string(),
        source_type: "ecfr".to_string(),
        url: format!("https://www.ecfr.gov/current/title-{}/part-{}", title, part),
        title: Some(format!("Title {} Part {}", title, part)),
        cfr_reference: Some(format!("{} CFR Part {}", title, part)),
        full_text: text.clone(),
        fetched_at: String::new(),
        metadata: None,
    };
    db::insert_regulatory_source(db_client, &source).await?;
    Ok(Some(text))
}

async fn extract_and_store_findings(
    context: &ResearchContext<'_>,
    entry: &TriageResult,
    policy: &Policy,
    title: i64,
    part: &str,
    clean_text: &str,
) -> StageResult<()> {
    let source_citation = format!("{} CFR Part {}", title, part);
    let prompt = BedrockClient::render_prompt(
        STAGE4B_EXTRACT_PROMPT,
        &[
            ("policy_name", &policy.name),
            ("source_citation", &source_citation),
            ("dimensions_text", context.dimensions_text),
            ("source_text", &clean_text[..clean_text.len().min(4000)]),
        ],
    );
    let result = context
        .bedrock
        .invoke_json(&prompt, FINDING_EXTRACTION_SYSTEM, Some(0.1), Some(2000))
        .await?;
    store_findings(context, entry, &result).await
}

async fn store_findings(
    context: &ResearchContext<'_>,
    entry: &TriageResult,
    result: &serde_json::Value,
) -> StageResult<()> {
    let Some(findings) = result.get("findings").and_then(|f| f.as_array()) else {
        return Ok(());
    };
    for finding_data in findings {
        let finding = structural_finding(context.run_id, &entry.policy_id, finding_data);
        db::insert_structural_finding(context.db_client, context.run_id, &finding).await?;
    }
    Ok(())
}

fn structural_finding(
    run_id: &str,
    policy_id: &str,
    finding_data: &serde_json::Value,
) -> StructuralFinding {
    StructuralFinding {
        finding_id: finding_id(policy_id, finding_data),
        run_id: run_id.to_string(),
        policy_id: policy_id.to_string(),
        dimension_id: json_opt_string(finding_data, "dimension_id"),
        observation: json_string(finding_data, "observation"),
        source_type: "ecfr".to_string(),
        source_citation: json_opt_string(finding_data, "source_citation"),
        source_text: json_opt_string(finding_data, "source_text_excerpt"),
        confidence: finding_data
            .get("confidence")
            .and_then(|c| c.as_str())
            .unwrap_or("medium")
            .to_string(),
        status: "active".to_string(),
        stale_reason: None,
        created_at: String::new(),
        created_by: Some("stage4b_research".to_string()),
        dimension_name: None,
    }
}

fn finding_id(policy_id: &str, finding_data: &serde_json::Value) -> String {
    let mut hasher = Sha256::new();
    hasher.update(format!(
        "{}:{}:{}",
        policy_id,
        finding_data
            .get("dimension_id")
            .and_then(|d| d.as_str())
            .unwrap_or(""),
        finding_data
            .get("source_citation")
            .and_then(|s| s.as_str())
            .unwrap_or("")
    ));
    format!("{:x}", hasher.finalize())[..12].to_string()
}

async fn finish_research_session(
    db_client: &Client,
    run_id: &str,
    entry: &TriageResult,
    hash: &str,
    session_id: &str,
) -> StageResult<()> {
    db::update_research_session(db_client, session_id, "findings_complete", None, None).await?;
    db::update_policy_lifecycle(db_client, &entry.policy_id, "structurally_characterized").await?;
    db::record_processing(db_client, 41, &entry.policy_id, hash, run_id).await
}

fn json_string(value: &serde_json::Value, key: &str) -> String {
    value
        .get(key)
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

fn json_opt_string(value: &serde_json::Value, key: &str) -> Option<String> {
    value.get(key).and_then(|v| v.as_str()).map(String::from)
}
