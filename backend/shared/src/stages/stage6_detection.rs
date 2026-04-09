//! Stage 6: Detection Pattern Generation

use serde_json::json;
use sha2::{Digest, Sha256};
use tokio_postgres::Client;
use tracing::info;

use crate::bedrock::BedrockClient;
use crate::db;
use crate::types::{Config, DetectionPattern};

const SYSTEM_PROMPT: &str = "You are a fraud detection analyst designing monitoring rules. You translate predicted exploitation steps into specific, queryable anomaly signals. Be concrete.";

const STAGE6_DETECT_PROMPT: &str = include_str!("../../prompts/stage6_detect.txt");

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
    info!("Stage 6: Detection Pattern Generation");
    db::log_stage_start(db_client, run_id, 6).await?;

    match run_inner(db_client, bedrock, run_id, config).await {
        Ok(result) => {
            db::log_stage_complete(db_client, run_id, 6, Some(&result)).await?;
            Ok(result)
        }
        Err(e) => {
            db::log_stage_failed(db_client, run_id, 6, &e.to_string()).await?;
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
    let stage5_status = db::get_stage_status(db_client, run_id, 5).await?;
    if !matches!(
        stage5_status.as_deref(),
        Some("approved") | Some("completed")
    ) {
        return Err(format!(
            "Stage 5 status is '{:?}'. Trees must be approved first.",
            stage5_status
        )
        .into());
    }

    let trees = db::get_exploitation_trees(db_client, true).await?;
    if trees.is_empty() {
        return Err("No approved exploitation trees found.".into());
    }

    // Load all steps with context
    let mut all_steps = Vec::new();
    for tree in &trees {
        let steps = db::get_exploitation_steps(db_client, &tree.tree_id).await?;
        let tree_summary = build_tree_summary(tree, &steps);
        for mut step in steps {
            step.policy_name = tree.policy_name.clone();
            all_steps.push((step, tree.clone(), tree_summary.clone()));
        }
    }

    if all_steps.is_empty() {
        return Err("No exploitation steps found.".into());
    }

    // Delta detection
    let stored = db::get_processing_hashes(db_client, 6).await?;
    let mut to_detect = Vec::new();
    for (step, tree, summary) in &all_steps {
        let quals_str: String = {
            let mut sorted = step.enabling_qualities.clone();
            sorted.sort();
            sorted.join(":")
        };
        let h = compute_hash(&[&step.description, &quals_str]);
        if stored.get(&step.step_id).map(|s| s.as_str()) != Some(&h) {
            to_detect.push((step, tree, summary, h));
        }
    }

    if to_detect.is_empty() {
        info!("All {} steps unchanged -- skipping.", all_steps.len());
        return Ok(json!({"patterns_generated": 0, "skipped_unchanged": all_steps.len()}));
    }

    info!(
        "{}/{} steps changed, generating patterns...",
        to_detect.len(),
        all_steps.len()
    );

    // Delete stale patterns
    for (step, _, _, _) in &to_detect {
        db::delete_patterns_for_step(db_client, &step.step_id).await?;
    }

    let data_sources_context = default_data_sources();
    let mut total_patterns = 0;

    for (step, _tree, summary, h) in &to_detect {
        let qualities_str = step.enabling_qualities.join(", ");
        let prompt = BedrockClient::render_prompt(
            STAGE6_DETECT_PROMPT,
            &[
                ("policy_name", step.policy_name.as_deref().unwrap_or("")),
                ("step_title", &step.title),
                ("step_description", &step.description),
                (
                    "step_actor_action",
                    step.actor_action.as_deref().unwrap_or(""),
                ),
                ("step_qualities", &qualities_str),
                ("tree_summary", summary),
                ("data_sources", &data_sources_context),
            ],
        );

        let result = bedrock
            .invoke_json(&prompt, SYSTEM_PROMPT, None, Some(8192))
            .await?;
        let patterns: Vec<serde_json::Value> = if result.is_array() {
            result.as_array().cloned().unwrap_or_default()
        } else {
            result
                .get("patterns")
                .and_then(|p| p.as_array())
                .cloned()
                .unwrap_or_else(|| vec![result.clone()])
        };

        for (i, pat_data) in patterns.iter().enumerate() {
            let pat_id = {
                let mut hasher = Sha256::new();
                hasher.update(format!("{}:pat:{}", step.step_id, i));
                format!("{:x}", hasher.finalize())[..12].to_string()
            };

            let pattern = DetectionPattern {
                pattern_id: pat_id,
                run_id: run_id.to_string(),
                step_id: Some(step.step_id.clone()),
                prediction_id: None,
                data_source: pat_data
                    .get("data_source")
                    .and_then(|d| d.as_str())
                    .unwrap_or("")
                    .to_string(),
                anomaly_signal: pat_data
                    .get("anomaly_signal")
                    .and_then(|a| a.as_str())
                    .unwrap_or("")
                    .to_string(),
                baseline: pat_data
                    .get("baseline")
                    .and_then(|b| b.as_str())
                    .map(String::from),
                false_positive_risk: pat_data
                    .get("false_positive_risk")
                    .and_then(|f| f.as_str())
                    .map(String::from),
                detection_latency: pat_data
                    .get("detection_latency")
                    .and_then(|d| d.as_str())
                    .map(String::from),
                priority: pat_data
                    .get("priority")
                    .and_then(|p| p.as_str())
                    .map(String::from),
                implementation_notes: pat_data
                    .get("implementation_notes")
                    .and_then(|n| n.as_str())
                    .map(String::from),
                created_at: String::new(),
                step_title: None,
                tree_id: None,
                policy_name: None,
            };
            db::insert_detection_pattern(db_client, run_id, &pattern).await?;
            total_patterns += 1;
        }
        db::record_processing(db_client, 6, &step.step_id, h, run_id).await?;
    }

    info!(
        "Stage 6 complete: {} detection patterns generated.",
        total_patterns
    );
    Ok(json!({"patterns_generated": total_patterns}))
}

fn build_tree_summary(
    tree: &crate::types::ExploitationTree,
    steps: &[crate::types::ExploitationStep],
) -> String {
    let mut lines = vec![format!(
        "Exploitation tree for {}:",
        tree.policy_name.as_deref().unwrap_or("Unknown")
    )];
    lines.push(format!(
        "  Actor: {}",
        tree.actor_profile.as_deref().unwrap_or("Unknown")
    ));
    lines.push(format!(
        "  Lifecycle: {}",
        tree.lifecycle_stage.as_deref().unwrap_or("Unknown")
    ));
    lines.push(format!("  Steps ({} total):", steps.len()));
    for s in steps {
        let branch = if s.is_branch_point.unwrap_or(false) {
            " [BRANCH POINT]"
        } else {
            ""
        };
        let label = s
            .branch_label
            .as_ref()
            .map(|l| format!(" ({})", l))
            .unwrap_or_default();
        lines.push(format!(
            "    {}. {}{}{}",
            s.step_order, s.title, branch, label
        ));
    }
    lines.join("\n")
}

fn default_data_sources() -> String {
    "Available data sources:\n\
     - Claims Database: Medicare FFS claims (Part A, B, D)\n\
     - Enrollment Database: Medicare/Medicaid beneficiary enrollment\n\
     - Provider Enrollment: NPI registry, provider enrollment dates\n\
     - MA Encounter Data: Medicare Advantage plan encounters\n\
     - EVV Data: Electronic Visit Verification records\n\
     - Marketplace Enrollment: ACA marketplace applications\n\
     - Exclusions Database: OIG exclusion list\n\
     - Financial Data: Provider payment amounts"
        .to_string()
}
