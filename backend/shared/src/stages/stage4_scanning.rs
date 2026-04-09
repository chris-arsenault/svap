//! Stage 4: Policy Corpus Scanning

use serde_json::json;
use sha2::{Digest, Sha256};
use tokio_postgres::Client;
use tracing::info;

use crate::bedrock::BedrockClient;
use crate::db;
use crate::rag::ContextAssembler;
use crate::types::Config;

const SYSTEM_CHARACTERIZE: &str = "You are a structural analyst characterizing how a government policy or program works. Focus on mechanical structure: money flows, verification, barriers. Do not evaluate whether the policy is good or bad.";
const SYSTEM_SCORE: &str = "You are scoring a policy against a structural vulnerability taxonomy. Apply each recognition test. A quality is PRESENT only if the structural characterization clearly shows the property. Be conservative.";

const STAGE4_CHARACTERIZE_PROMPT: &str = include_str!("../../prompts/stage4_characterize.txt");
const STAGE4_SCORE_PROMPT: &str = include_str!("../../prompts/stage4_score.txt");

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
    info!("Stage 4: Policy Corpus Scanning");
    db::log_stage_start(db_client, run_id, 4).await?;

    match run_inner(db_client, bedrock, run_id, config).await {
        Ok(result) => {
            db::log_stage_complete(db_client, run_id, 4, Some(&result)).await?;
            Ok(result)
        }
        Err(e) => {
            db::log_stage_failed(db_client, run_id, 4, &e.to_string()).await?;
            Err(e)
        }
    }
}

async fn run_inner(
    db_client: &Client,
    bedrock: &BedrockClient,
    run_id: &str,
    config: &Config,
) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
    let taxonomy = db::get_approved_taxonomy(db_client).await?;
    let calibration = db::get_calibration(db_client).await?;
    if taxonomy.is_empty() {
        return Err("No taxonomy found. Run Stages 1-3 first.".into());
    }

    let ctx = ContextAssembler::new(config);
    let taxonomy_context = ContextAssembler::format_taxonomy_context(&taxonomy);
    let threshold = calibration.as_ref().map(|c| c.threshold).unwrap_or(3);

    let mut policies = db::get_policies(db_client).await?;
    if policies.is_empty() {
        info!("No policies found.");
        return Ok(json!({"policies_scored": 0}));
    }

    // 4a: Structural Characterization
    info!("Characterizing {} policies...", policies.len());
    for policy in &policies {
        if policy.structural_characterization.is_some() {
            continue;
        }
        info!("Characterizing: {}", policy.name);
        let rag_context = ctx
            .retrieve(
                db_client,
                &format!(
                    "{} {}",
                    policy.name,
                    policy.description.as_deref().unwrap_or("")
                ),
                Some("policy"),
                None,
            )
            .await?;

        let prompt = BedrockClient::render_prompt(
            STAGE4_CHARACTERIZE_PROMPT,
            &[
                ("policy_name", &policy.name),
                (
                    "policy_description",
                    policy
                        .description
                        .as_deref()
                        .unwrap_or("No description provided."),
                ),
                (
                    "rag_context",
                    if rag_context.is_empty() {
                        "No additional source documents available."
                    } else {
                        &rag_context
                    },
                ),
            ],
        );
        let characterization = bedrock
            .invoke(&prompt, SYSTEM_CHARACTERIZE, None, Some(2048))
            .await?;
        let mut updated = policy.clone();
        updated.structural_characterization = Some(characterization);
        db::insert_policy(db_client, &updated).await?;
    }

    policies = db::get_policies(db_client).await?;

    // Filter out policies already assessed by deep research
    let mut policies_to_score = Vec::new();
    for policy in &policies {
        let existing = db::get_quality_assessments(db_client, Some(&policy.policy_id)).await?;
        if existing.is_empty() {
            policies_to_score.push(policy);
        } else {
            info!(
                "Skipping {} -- already assessed via deep research",
                policy.name
            );
        }
    }

    // Delta detection
    let tax_fp = taxonomy_fingerprint(&taxonomy);
    let stored_hashes = db::get_processing_hashes(db_client, 4).await?;
    let mut delta_policies = Vec::new();
    let mut skipped = 0;

    for policy in &policies_to_score {
        let h = compute_hash(&[
            policy.structural_characterization.as_deref().unwrap_or(""),
            &tax_fp,
        ]);
        if stored_hashes.get(&policy.policy_id).map(|s| s.as_str()) == Some(&h) {
            skipped += 1;
        } else {
            delta_policies.push((policy, h));
        }
    }

    if delta_policies.is_empty() {
        info!("All policies unchanged. Skipping scoring.");
        return Ok(json!({"policies_scored": 0, "skipped_unchanged": skipped}));
    }

    // 4b: Vulnerability Scoring
    info!(
        "Scoring {} policies ({} unchanged)...",
        delta_policies.len(),
        skipped
    );
    let mut results = Vec::new();

    for (policy, h) in &delta_policies {
        info!("Scoring: {}", policy.name);
        let prompt = BedrockClient::render_prompt(
            STAGE4_SCORE_PROMPT,
            &[
                ("policy_name", &policy.name),
                (
                    "structural_characterization",
                    policy
                        .structural_characterization
                        .as_deref()
                        .unwrap_or(policy.description.as_deref().unwrap_or("")),
                ),
                ("taxonomy", &taxonomy_context),
            ],
        );
        let scores = bedrock
            .invoke_json(&prompt, SYSTEM_SCORE, None, Some(2048))
            .await?;
        let score_map = scores.get("scores").unwrap_or(&scores);

        let mut convergence_count = 0;
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
                db::insert_policy_score(
                    db_client,
                    run_id,
                    &policy.policy_id,
                    quality_id,
                    present,
                    &evidence,
                )
                .await?;
                if present {
                    convergence_count += 1;
                }
            }
        }
        db::record_processing(db_client, 4, &policy.policy_id, h, run_id).await?;
        results.push(json!({"policy": policy.name, "convergence_score": convergence_count}));
    }

    let above_threshold = results
        .iter()
        .filter(|r| {
            r.get("convergence_score")
                .and_then(|c| c.as_i64())
                .unwrap_or(0)
                >= threshold as i64
        })
        .count();
    info!("Stage 4 complete: {} policies scored.", results.len());
    Ok(json!({"policies_scored": results.len(), "above_threshold": above_threshold}))
}
