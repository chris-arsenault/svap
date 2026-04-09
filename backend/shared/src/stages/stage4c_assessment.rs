//! Stage 4C: Quality Assessment from Structural Findings

use serde_json::json;
use sha2::{Digest, Sha256};
use tokio_postgres::Client;
use tracing::info;

use crate::bedrock::BedrockClient;
use crate::db;
use crate::types::{Config, QualityAssessment};

const ASSESSMENT_SYSTEM: &str = "You are assessing whether a structural vulnerability quality is present in a policy based on specific, cited structural findings. Be conservative -- a quality is present only if findings directly support it.";

const STAGE4C_ASSESS_PROMPT: &str = include_str!("../../prompts/stage4c_assess_quality.txt");

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
    info!("Stage 4C: Quality Assessment from Findings");
    db::log_stage_start(db_client, run_id, 42).await?;

    match run_inner(db_client, bedrock, run_id, config).await {
        Ok(result) => {
            db::log_stage_complete(db_client, run_id, 42, Some(&result)).await?;
            Ok(result)
        }
        Err(e) => {
            db::log_stage_failed(db_client, run_id, 42, &e.to_string()).await?;
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
    let taxonomy = db::get_approved_taxonomy(db_client).await?;
    if taxonomy.is_empty() {
        return Ok(json!({"policies_assessed": 0}));
    }

    // Collect assessable sessions
    let sessions_new = db::get_research_sessions(db_client, Some("findings_complete")).await?;
    let sessions_done = db::get_research_sessions(db_client, Some("assessment_complete")).await?;
    let mut seen = std::collections::HashSet::new();
    let mut sessions = Vec::new();
    for s in sessions_new.iter().chain(sessions_done.iter()) {
        if seen.insert(s.policy_id.clone()) {
            sessions.push(s.clone());
        }
    }

    if sessions.is_empty() {
        return Ok(json!({"policies_assessed": 0}));
    }

    // Delta detection
    let tax_fp = taxonomy_fingerprint(&taxonomy);
    let stored_hashes = db::get_processing_hashes(db_client, 42).await?;
    let mut sessions_to_assess = Vec::new();
    let mut skipped = 0;

    for session in &sessions {
        let findings = db::get_structural_findings(db_client, &session.policy_id).await?;
        if findings.is_empty() {
            continue;
        }
        let mut finding_ids: Vec<&str> = findings.iter().map(|f| f.finding_id.as_str()).collect();
        finding_ids.sort();
        let h = compute_hash(&[&finding_ids.join(":"), &tax_fp]);
        if stored_hashes.get(&session.policy_id).map(|s| s.as_str()) == Some(&h) {
            skipped += 1;
        } else {
            sessions_to_assess.push((session.clone(), findings, h));
        }
    }

    if sessions_to_assess.is_empty() {
        return Ok(json!({"policies_assessed": 0, "skipped_unchanged": skipped}));
    }

    let all_policies = db::get_policies(db_client).await?;
    let mut assessed = 0;

    for (session, findings, h) in &sessions_to_assess {
        let policy = all_policies
            .iter()
            .find(|p| p.policy_id == session.policy_id);
        let policy_name = policy
            .map(|p| p.name.as_str())
            .unwrap_or(&session.policy_id);

        info!("Assessing: {} ({} findings)", policy_name, findings.len());

        let findings_text: String = findings
            .iter()
            .map(|f| {
                let dim_name = f
                    .dimension_name
                    .as_deref()
                    .unwrap_or(f.dimension_id.as_deref().unwrap_or("Unknown"));
                format!(
                    "[{}] ({}, {} confidence)\n  {}\n  Source: {}",
                    f.finding_id,
                    dim_name,
                    f.confidence,
                    f.observation,
                    f.source_citation.as_deref().unwrap_or("N/A")
                )
            })
            .collect::<Vec<_>>()
            .join("\n\n");

        for quality in &taxonomy {
            let prompt = BedrockClient::render_prompt(
                STAGE4C_ASSESS_PROMPT,
                &[
                    ("quality_id", &quality.quality_id),
                    ("quality_name", &quality.name),
                    ("quality_definition", &quality.definition),
                    ("quality_recognition_test", &quality.recognition_test),
                    ("policy_name", policy_name),
                    ("findings_text", &findings_text),
                ],
            );
            let result = bedrock
                .invoke_json(&prompt, ASSESSMENT_SYSTEM, Some(0.1), Some(1000))
                .await?;

            let assessment_id = {
                let mut hasher = Sha256::new();
                hasher.update(format!("{}:{}", session.policy_id, quality.quality_id));
                format!("{:x}", hasher.finalize())[..12].to_string()
            };

            let cited_ids: Vec<String> = result
                .get("finding_ids")
                .and_then(|f| f.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();
            let valid_ids: std::collections::HashSet<&str> =
                findings.iter().map(|f| f.finding_id.as_str()).collect();
            let validated: Vec<String> = cited_ids
                .into_iter()
                .filter(|id| valid_ids.contains(id.as_str()))
                .collect();

            let present = result
                .get("present")
                .and_then(|p| p.as_str())
                .unwrap_or("uncertain")
                .to_string();

            let assessment = QualityAssessment {
                assessment_id,
                run_id: run_id.to_string(),
                policy_id: session.policy_id.clone(),
                quality_id: quality.quality_id.clone(),
                taxonomy_version: Some(run_id.to_string()),
                present: present.clone(),
                evidence_finding_ids: Some(serde_json::to_string(&validated)?),
                confidence: result
                    .get("confidence")
                    .and_then(|c| c.as_str())
                    .unwrap_or("medium")
                    .to_string(),
                rationale: result
                    .get("reasoning")
                    .and_then(|r| r.as_str())
                    .map(String::from),
                created_at: String::new(),
            };
            db::upsert_quality_assessment(db_client, run_id, &assessment).await?;

            // Backward compat: sync to policy_scores
            let present_bool = present == "yes";
            db::insert_policy_score(
                db_client,
                run_id,
                &session.policy_id,
                &quality.quality_id,
                present_bool,
                assessment.rationale.as_deref().unwrap_or(""),
            )
            .await?;
        }

        db::update_research_session(
            db_client,
            &session.session_id,
            "assessment_complete",
            None,
            None,
        )
        .await?;
        db::update_policy_lifecycle(db_client, &session.policy_id, "fully_assessed").await?;
        db::record_processing(db_client, 42, &session.policy_id, h, run_id).await?;
        assessed += 1;
    }

    info!(
        "Assessment complete: {} policies ({} unchanged).",
        assessed, skipped
    );
    Ok(
        json!({"policies_assessed": assessed, "skipped": skipped, "qualities_per_policy": taxonomy.len()}),
    )
}
