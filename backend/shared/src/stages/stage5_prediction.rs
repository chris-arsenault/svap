//! Stage 5: Exploitation Tree Generation

use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use tokio_postgres::Client;
use tracing::info;

use crate::bedrock::BedrockClient;
use crate::db;
use crate::types::{Config, ExploitationStep, ExploitationTree};

const SYSTEM_PROMPT: &str = "You are a structural analyst building exploitation decision trees. Every step must be CAUSED by a specific vulnerability quality. If you cannot cite which structural quality enables a step, do not include it.";

const STAGE5_PREDICT_PROMPT: &str = include_str!("../../prompts/stage5_predict.txt");

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
) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
    info!("Stage 5: Exploitation Tree Generation");
    db::log_stage_start(db_client, run_id, 5).await?;

    match run_inner(db_client, bedrock, run_id, config).await {
        Ok(result) => Ok(result),
        Err(e) => {
            db::log_stage_failed(db_client, run_id, 5, &e.to_string()).await?;
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
    let calibration = db::get_calibration(db_client).await?;
    let threshold = calibration.as_ref().map(|c| c.threshold).unwrap_or(3);

    let policies = db::get_policies(db_client).await?;
    let policy_scores = db::get_policy_scores(db_client).await?;

    // Build per-policy profiles
    let mut profiles: HashMap<String, (String, Vec<String>, i32)> = HashMap::new();
    for ps in &policy_scores {
        let entry =
            profiles
                .entry(ps.policy_id.clone())
                .or_insert((ps.name.clone(), Vec::new(), 0));
        if ps.present {
            entry.1.push(ps.quality_id.clone());
            entry.2 += 1;
        }
    }

    let high_risk: HashMap<&str, &(String, Vec<String>, i32)> = profiles
        .iter()
        .filter(|(_, v)| v.2 >= threshold)
        .map(|(k, v)| (k.as_str(), v))
        .collect();

    if high_risk.is_empty() {
        info!("No policies scored at or above threshold ({}).", threshold);
        db::log_stage_complete(db_client, run_id, 5, Some(&json!({"trees_generated": 0}))).await?;
        return Ok(json!({"trees_generated": 0}));
    }

    // Delta detection
    let cal_fp = calibration
        .as_ref()
        .map(|c| c.threshold.to_string())
        .unwrap_or_else(|| "3".to_string());
    let stored = db::get_processing_hashes(db_client, 5).await?;

    let mut to_predict = Vec::new();
    for (policy_id, (name, qualities, count)) in &profiles {
        if *count < threshold {
            continue;
        }
        let mut sorted_quals = qualities.clone();
        sorted_quals.sort();
        let quality_profile = sorted_quals.join(":");
        let h = compute_hash(&[&quality_profile, &cal_fp]);
        if stored.get(policy_id).map(|s| s.as_str()) != Some(&h) {
            to_predict.push((
                policy_id.as_str(),
                name.as_str(),
                qualities.clone(),
                *count,
                h,
            ));
        }
    }

    if to_predict.is_empty() {
        info!("All high-risk policies unchanged -- skipping.");
        db::log_stage_complete(
            db_client,
            run_id,
            5,
            Some(&json!({"trees_generated": 0, "skipped_unchanged": high_risk.len()})),
        )
        .await?;
        return Ok(json!({"trees_generated": 0}));
    }

    info!(
        "{}/{} policies changed, generating exploitation trees...",
        to_predict.len(),
        high_risk.len()
    );

    // Delete stale data
    for (policy_id, _, _, _, _) in &to_predict {
        db::delete_tree_for_policy(db_client, policy_id).await?;
    }

    let quality_lookup: HashMap<&str, &crate::types::TaxonomyQuality> = taxonomy
        .iter()
        .map(|q| (q.quality_id.as_str(), q))
        .collect();

    let mut total_steps = 0;

    for (policy_id, name, qualities, count, h) in &to_predict {
        let policy = match policies.iter().find(|p| p.policy_id == *policy_id) {
            Some(p) => p,
            None => continue,
        };

        // Build quality descriptions
        let quality_descriptions: String = qualities
            .iter()
            .filter_map(|qid| {
                quality_lookup.get(qid.as_str()).map(|q| {
                    let evidence_row = policy_scores.iter().find(|ps| {
                        ps.policy_id == *policy_id && ps.quality_id == *qid && ps.present
                    });
                    let evidence = evidence_row
                        .and_then(|e| e.evidence.as_deref())
                        .unwrap_or("");
                    format!(
                        "- {} ({}): {}\n  How it manifests here: {}",
                        qid, q.name, q.definition, evidence
                    )
                })
            })
            .collect::<Vec<_>>()
            .join("\n");

        let prompt = BedrockClient::render_prompt(
            STAGE5_PREDICT_PROMPT,
            &[
                ("policy_name", name),
                (
                    "policy_description",
                    policy
                        .structural_characterization
                        .as_deref()
                        .unwrap_or(policy.description.as_deref().unwrap_or("")),
                ),
                ("convergence_score", &count.to_string()),
                ("quality_profile", &quality_descriptions),
            ],
        );

        let result = bedrock
            .invoke_json(&prompt, SYSTEM_PROMPT, Some(0.3), Some(4096))
            .await?;

        // Store tree
        let tree_id = {
            let mut hasher = Sha256::new();
            hasher.update(policy_id.as_bytes());
            format!("{:x}", hasher.finalize())[..12].to_string()
        };

        let tree = ExploitationTree {
            tree_id: tree_id.clone(),
            policy_id: policy_id.to_string(),
            convergence_score: *count,
            actor_profile: result
                .get("actor_profile")
                .and_then(|a| a.as_str())
                .map(String::from),
            lifecycle_stage: result
                .get("lifecycle_stage")
                .and_then(|l| l.as_str())
                .map(String::from),
            detection_difficulty: result
                .get("detection_difficulty")
                .and_then(|d| d.as_str())
                .map(String::from),
            review_status: Some("draft".to_string()),
            reviewer_notes: None,
            run_id: Some(run_id.to_string()),
            created_at: String::new(),
            policy_name: Some(name.to_string()),
            step_count: None,
            steps: Vec::new(),
        };
        db::insert_exploitation_tree(db_client, run_id, &tree).await?;

        // Store steps
        let steps = result
            .get("steps")
            .and_then(|s| s.as_array())
            .cloned()
            .unwrap_or_default();
        let mut order_to_id: HashMap<i64, String> = HashMap::new();

        for step_data in &steps {
            let order = step_data
                .get("step_order")
                .and_then(|o| o.as_i64())
                .unwrap_or(0);
            let title = step_data
                .get("title")
                .and_then(|t| t.as_str())
                .unwrap_or("");
            let step_id = {
                let mut hasher = Sha256::new();
                hasher.update(format!(
                    "{}:step:{}:{}",
                    policy_id,
                    order,
                    &title[..title.len().min(50)]
                ));
                format!("{:x}", hasher.finalize())[..12].to_string()
            };
            order_to_id.insert(order, step_id.clone());

            let parent_order = step_data.get("parent_step_order").and_then(|p| p.as_i64());
            let parent_id = parent_order.and_then(|po| order_to_id.get(&po).cloned());

            let step = ExploitationStep {
                step_id: step_id.clone(),
                tree_id: tree_id.clone(),
                parent_step_id: parent_id,
                step_order: order as i32,
                title: title.to_string(),
                description: step_data
                    .get("description")
                    .and_then(|d| d.as_str())
                    .unwrap_or("")
                    .to_string(),
                actor_action: step_data
                    .get("actor_action")
                    .and_then(|a| a.as_str())
                    .map(String::from),
                is_branch_point: step_data.get("is_branch_point").and_then(|b| b.as_bool()),
                branch_label: step_data
                    .get("branch_label")
                    .and_then(|b| b.as_str())
                    .map(String::from),
                created_at: String::new(),
                policy_id: None,
                policy_name: None,
                enabling_qualities: Vec::new(),
            };
            let quality_ids: Vec<String> = step_data
                .get("enabling_qualities")
                .and_then(|q| q.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();
            db::insert_exploitation_step(db_client, &step, &quality_ids).await?;
            total_steps += 1;
        }

        db::record_processing(db_client, 5, policy_id, h, run_id).await?;
    }

    if total_steps > 0 {
        db::log_stage_pending_review(db_client, run_id, 5).await?;
        info!(
            "Stage 5 complete: {} steps across {} trees. HUMAN REVIEW REQUIRED.",
            total_steps,
            to_predict.len()
        );
    } else {
        db::log_stage_complete(
            db_client,
            run_id,
            5,
            Some(&json!({"trees_generated": 0, "steps_generated": 0})),
        )
        .await?;
    }

    Ok(json!({"trees_generated": to_predict.len(), "steps_generated": total_steps}))
}
