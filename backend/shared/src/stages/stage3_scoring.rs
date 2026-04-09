//! Stage 3: Convergence Scoring & Calibration

use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use tokio_postgres::Client;
use tracing::info;

use crate::bedrock::BedrockClient;
use crate::db;
use crate::rag::ContextAssembler;
use crate::types::Config;

const SYSTEM_PROMPT: &str = "You are scoring a policy against a structural vulnerability taxonomy. Apply each recognition test precisely. A quality is PRESENT only if the policy clearly exhibits the structural property described. If ambiguous, mark ABSENT.";

const STAGE3_SCORE_PROMPT: &str = include_str!("../../prompts/stage3_score.txt");

fn compute_hash(parts: &[&str]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(parts.join("|"));
    format!("{:x}", hasher.finalize())[..12].to_string()
}

fn taxonomy_fingerprint(taxonomy: &[crate::types::TaxonomyQuality]) -> String {
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
) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
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
) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
    let stage2_status = db::get_stage_status(db_client, run_id, 2).await?;
    if !matches!(
        stage2_status.as_deref(),
        Some("approved") | Some("completed")
    ) {
        return Err(format!(
            "Stage 2 status is '{:?}'. Taxonomy must be approved before scoring.",
            stage2_status
        )
        .into());
    }

    let cases = db::get_cases(db_client).await?;
    let taxonomy = db::get_approved_taxonomy(db_client).await?;
    if cases.is_empty() || taxonomy.is_empty() {
        return Err("Need both cases and taxonomy.".into());
    }

    let taxonomy_context = ContextAssembler::format_taxonomy_context(&taxonomy);
    let tax_fp = taxonomy_fingerprint(&taxonomy);
    let stored = db::get_processing_hashes(db_client, 3).await?;

    let mut cases_to_score = Vec::new();
    let mut skipped = 0;
    for case in &cases {
        let h = compute_hash(&[&case.enabling_condition, &tax_fp]);
        if stored.get(&case.case_id).map(|s| s.as_str()) == Some(&h) {
            skipped += 1;
        } else {
            cases_to_score.push((case, h));
        }
    }

    if cases_to_score.is_empty() {
        info!("All {} cases unchanged. Skipping scoring.", cases.len());
        return Ok(json!({"cases_scored": 0, "skipped": skipped}));
    }

    info!(
        "Scoring {} cases ({} unchanged)...",
        cases_to_score.len(),
        skipped
    );
    for (case, h) in &cases_to_score {
        info!("Scoring: {}", case.case_name);
        let prompt = BedrockClient::render_prompt(
            STAGE3_SCORE_PROMPT,
            &[
                ("case_name", &case.case_name),
                ("exploited_policy", &case.exploited_policy),
                ("scheme_mechanics", &case.scheme_mechanics),
                ("enabling_condition", &case.enabling_condition),
                ("taxonomy", &taxonomy_context),
            ],
        );
        let scores = bedrock
            .invoke_json(&prompt, SYSTEM_PROMPT, None, Some(2048))
            .await?;
        let score_map = scores.get("scores").unwrap_or(&scores);

        if let Some(obj) = score_map.as_object() {
            for (quality_id, score_data) in obj {
                let (present, evidence) = if let Some(sd) = score_data.as_object() {
                    (
                        sd.get("present").and_then(|p| p.as_bool()).unwrap_or(false),
                        sd.get("evidence")
                            .and_then(|e| e.as_str())
                            .unwrap_or("")
                            .to_string(),
                    )
                } else {
                    (score_data.as_bool().unwrap_or(false), String::new())
                };
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
        }
        db::record_processing(db_client, 3, &case.case_id, h, run_id).await?;
    }

    // Calibration
    info!("Running calibration analysis...");
    let matrix = db::get_convergence_matrix(db_client).await?;
    let mut case_scores: HashMap<String, (String, i32, f64)> = HashMap::new();
    let mut quality_freq: HashMap<String, i32> = HashMap::new();

    for row in &matrix {
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
    sorted_cases.sort_by(|a, b| b.1.cmp(&a.1));

    let cal_data: Vec<serde_json::Value> = sorted_cases
        .iter()
        .map(|(name, score, scale)| json!({"case": name, "score": score, "scale_dollars": scale}))
        .collect();

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

    let freq_val = serde_json::to_value(&quality_freq)?;
    let combos_val = json!({});
    db::insert_calibration(
        db_client,
        run_id,
        threshold,
        cal_result
            .get("correlation_notes")
            .and_then(|n| n.as_str())
            .unwrap_or(""),
        &freq_val,
        &combos_val,
    )
    .await?;

    info!("Stage 3 complete. Threshold: {}", threshold);
    Ok(json!({"cases_scored": cases_to_score.len(), "threshold": threshold}))
}
