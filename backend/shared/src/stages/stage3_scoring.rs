//! Stage 3: Convergence Scoring & Calibration

use serde_json::json;
use sha2::{Digest, Sha256};
use std::cmp::Reverse;
use std::collections::HashMap;
use tokio_postgres::Client;
use tracing::info;

use crate::bedrock::BedrockClient;
use crate::db;
use crate::rag::ContextAssembler;
use crate::types::{Case, Config, ConvergenceRow, TaxonomyQuality};

const SYSTEM_PROMPT: &str = "You are scoring a policy against a structural vulnerability taxonomy. Apply each recognition test precisely. A quality is PRESENT only if the policy clearly exhibits the structural property described. If ambiguous, mark ABSENT.";

const STAGE3_SCORE_PROMPT: &str = include_str!("../../prompts/stage3_score.txt");
type StageResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

struct ScoringInputs {
    cases: Vec<Case>,
    taxonomy_context: String,
    tax_fp: String,
}

fn compute_hash(parts: &[&str]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(parts.join("|"));
    format!("{:x}", hasher.finalize())[..12].to_string()
}

fn taxonomy_fingerprint(taxonomy: &[TaxonomyQuality]) -> String {
    let mut ids: Vec<&str> = taxonomy.iter().map(|q| q.quality_id.as_str()).collect();
    ids.sort();
    let mut hasher = Sha256::new();
    hasher.update(ids.join(":"));
    format!("{:x}", hasher.finalize())[..12].to_string()
}

pub async fn run(
    db_client: &Client,
    bedrock: &BedrockClient,
    run_id: &str,
    config: &Config,
) -> StageResult<serde_json::Value> {
    info!("Stage 3: Convergence Scoring & Calibration");
    db::log_stage_start(db_client, run_id, 3).await?;

    match run_inner(db_client, bedrock, run_id, config).await {
        Ok(result) => {
            db::log_stage_complete(db_client, run_id, 3, Some(&result)).await?;
            Ok(result)
        }
        Err(e) => {
            db::log_stage_failed(db_client, run_id, 3, &e.to_string()).await?;
            Err(e)
        }
    }
}

async fn run_inner(
    db_client: &Client,
    bedrock: &BedrockClient,
    run_id: &str,
    _config: &Config,
) -> StageResult<serde_json::Value> {
    ensure_taxonomy_ready(db_client, run_id).await?;
    let inputs = scoring_inputs(db_client).await?;
    score_and_calibrate(db_client, bedrock, run_id, inputs).await
}

async fn scoring_inputs(db_client: &Client) -> StageResult<ScoringInputs> {
    let cases = db::get_cases(db_client).await?;
    let taxonomy = db::get_approved_taxonomy(db_client).await?;
    if cases.is_empty() || taxonomy.is_empty() {
        return Err("Need both cases and taxonomy.".into());
    }

    Ok(ScoringInputs {
        cases,
        taxonomy_context: ContextAssembler::format_taxonomy_context(&taxonomy),
        tax_fp: taxonomy_fingerprint(&taxonomy),
    })
}

async fn score_and_calibrate(
    db_client: &Client,
    bedrock: &BedrockClient,
    run_id: &str,
    inputs: ScoringInputs,
) -> StageResult<serde_json::Value> {
    let stored = db::get_processing_hashes(db_client, 3).await?;
    let (cases_to_score, skipped) = changed_cases(&inputs.cases, &stored, &inputs.tax_fp);

    if cases_to_score.is_empty() {
        info!(
            "All {} cases unchanged. Skipping scoring.",
            inputs.cases.len()
        );
        return Ok(json!({"cases_scored": 0, "skipped": skipped}));
    }

    info!(
        "Scoring {} cases ({} unchanged)...",
        cases_to_score.len(),
        skipped
    );
    score_changed_cases(
        db_client,
        bedrock,
        run_id,
        &inputs.taxonomy_context,
        &cases_to_score,
    )
    .await?;
    let threshold = run_calibration(db_client, bedrock, run_id).await?;

    info!("Stage 3 complete. Threshold: {}", threshold);
    Ok(json!({"cases_scored": cases_to_score.len(), "threshold": threshold}))
}

async fn ensure_taxonomy_ready(db_client: &Client, run_id: &str) -> StageResult<()> {
    let stage2_status = db::get_stage_status(db_client, run_id, 2).await?;
    if matches!(
        stage2_status.as_deref(),
        Some("approved") | Some("completed")
    ) {
        return Ok(());
    }
    Err(format!(
        "Stage 2 status is '{:?}'. Taxonomy must be approved before scoring.",
        stage2_status
    )
    .into())
}

fn changed_cases<'a>(
    cases: &'a [Case],
    stored: &HashMap<String, String>,
    tax_fp: &str,
) -> (Vec<(&'a Case, String)>, usize) {
    let mut changed = Vec::new();
    let mut skipped = 0;

    for case in cases {
        let hash = compute_hash(&[&case.enabling_condition, tax_fp]);
        if stored.get(&case.case_id).map(|s| s.as_str()) == Some(&hash) {
            skipped += 1;
        } else {
            changed.push((case, hash));
        }
    }

    (changed, skipped)
}

async fn score_changed_cases(
    db_client: &Client,
    bedrock: &BedrockClient,
    run_id: &str,
    taxonomy_context: &str,
    cases_to_score: &[(&Case, String)],
) -> StageResult<()> {
    for (case, hash) in cases_to_score {
        let scores = score_case(bedrock, case, taxonomy_context).await?;
        insert_case_scores(db_client, run_id, case, &scores).await?;
        db::record_processing(db_client, 3, &case.case_id, hash, run_id).await?;
    }
    Ok(())
}

async fn score_case(
    bedrock: &BedrockClient,
    case: &Case,
    taxonomy_context: &str,
) -> StageResult<serde_json::Value> {
    info!("Scoring: {}", case.case_name);
    let prompt = BedrockClient::render_prompt(
        STAGE3_SCORE_PROMPT,
        &[
            ("case_name", &case.case_name),
            ("exploited_policy", &case.exploited_policy),
            ("scheme_mechanics", &case.scheme_mechanics),
            ("enabling_condition", &case.enabling_condition),
            ("taxonomy", taxonomy_context),
        ],
    );
    bedrock
        .invoke_json(&prompt, SYSTEM_PROMPT, None, Some(2048))
        .await
}

async fn insert_case_scores(
    db_client: &Client,
    run_id: &str,
    case: &Case,
    scores: &serde_json::Value,
) -> StageResult<()> {
    let score_map = scores.get("scores").unwrap_or(scores);
    let Some(obj) = score_map.as_object() else {
        return Ok(());
    };

    for (quality_id, score_data) in obj {
        let (present, evidence) = parse_score(score_data);
        db::insert_convergence_score(
            db_client,
            run_id,
            &case.case_id,
            quality_id,
            present,
            &evidence,
        )
        .await?;
    }
    Ok(())
}

fn parse_score(score_data: &serde_json::Value) -> (bool, String) {
    if let Some(sd) = score_data.as_object() {
        return (
            sd.get("present").and_then(|p| p.as_bool()).unwrap_or(false),
            sd.get("evidence")
                .and_then(|e| e.as_str())
                .unwrap_or("")
                .to_string(),
        );
    }
    (score_data.as_bool().unwrap_or(false), String::new())
}

async fn run_calibration(
    db_client: &Client,
    bedrock: &BedrockClient,
    run_id: &str,
) -> StageResult<i32> {
    info!("Running calibration analysis...");
    let matrix = db::get_convergence_matrix(db_client).await?;
    let (cal_data, quality_freq) = calibration_inputs(&matrix);
    let cal_prompt = format!(
        "Analyze this convergence score data to determine the calibration threshold.\n\n\
         {}\n\n\
         Determine:\n1. THRESHOLD: minimum convergence score for large-scale exploitation\n\
         2. CORRELATION_NOTES: relationship description\n\n\
         Return JSON: {{\"threshold\": N, \"correlation_notes\": \"...\"}}",
        serde_json::to_string_pretty(&cal_data)?
    );
    let cal_result = bedrock
        .invoke_json(&cal_prompt, "", None, Some(1024))
        .await?;
    let threshold = cal_result
        .get("threshold")
        .and_then(|t| t.as_i64())
        .unwrap_or(3) as i32;

    db::insert_calibration(
        db_client,
        run_id,
        threshold,
        cal_result
            .get("correlation_notes")
            .and_then(|n| n.as_str())
            .unwrap_or(""),
        &serde_json::to_value(&quality_freq)?,
        &json!({}),
    )
    .await?;
    Ok(threshold)
}

fn calibration_inputs(matrix: &[ConvergenceRow]) -> (Vec<serde_json::Value>, HashMap<String, i32>) {
    let mut case_scores: HashMap<String, (String, i32, f64)> = HashMap::new();
    let mut quality_freq: HashMap<String, i32> = HashMap::new();

    for row in matrix {
        let entry = case_scores.entry(row.case_id.clone()).or_insert((
            row.case_name.clone(),
            0,
            row.scale_dollars.unwrap_or(0.0),
        ));
        if row.present {
            entry.1 += 1;
            *quality_freq.entry(row.quality_id.clone()).or_insert(0) += 1;
        }
    }

    let mut sorted_cases: Vec<_> = case_scores.values().collect();
    sorted_cases.sort_by_key(|case| Reverse(case.1));
    let cal_data = sorted_cases
        .iter()
        .map(|(name, score, scale)| json!({"case": name, "score": score, "scale_dollars": scale}))
        .collect();
    (cal_data, quality_freq)
}
