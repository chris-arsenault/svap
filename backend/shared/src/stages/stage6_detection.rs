//! Stage 6: Detection Pattern Generation

use serde_json::json;
use sha2::{Digest, Sha256};
use tokio_postgres::Client;
use tracing::info;

use crate::bedrock::BedrockClient;
use crate::db;
use crate::types::{Config, DetectionPattern, ExploitationStep, ExploitationTree};

const SYSTEM_PROMPT: &str = "You are a fraud detection analyst designing monitoring rules. You translate predicted exploitation steps into specific, queryable anomaly signals. Be concrete.";

const STAGE6_DETECT_PROMPT: &str = include_str!("../../prompts/stage6_detect.txt");
type StageResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

struct DetectionContext<'a> {
    db_client: &'a Client,
    bedrock: &'a BedrockClient,
    run_id: &'a str,
}

struct StepContext {
    step: ExploitationStep,
    summary: String,
}

struct DetectionTarget {
    step: ExploitationStep,
    summary: String,
    hash: String,
}

struct DetectionRun {
    targets: Vec<DetectionTarget>,
    total_steps: usize,
}

enum DetectionPreparation {
    Unchanged { total_steps: usize },
    Run(DetectionRun),
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
) -> StageResult<serde_json::Value> {
    ensure_trees_ready(db_client, run_id).await?;
    match prepare_detection(db_client).await? {
        DetectionPreparation::Unchanged { total_steps } => {
            info!("All {} steps unchanged -- skipping.", total_steps);
            Ok(json!({"patterns_generated": 0, "skipped_unchanged": total_steps}))
        }
        DetectionPreparation::Run(detection_run) => {
            execute_detection(db_client, bedrock, run_id, detection_run).await
        }
    }
}

async fn prepare_detection(db_client: &Client) -> StageResult<DetectionPreparation> {
    let trees = db::get_exploitation_trees(db_client, true).await?;
    if trees.is_empty() {
        return Err("No approved exploitation trees found.".into());
    }

    let all_steps = load_step_contexts(db_client, &trees).await?;

    if all_steps.is_empty() {
        return Err("No exploitation steps found.".into());
    }

    let stored = db::get_processing_hashes(db_client, 6).await?;
    let to_detect = changed_steps(&all_steps, &stored);

    if to_detect.is_empty() {
        return Ok(DetectionPreparation::Unchanged {
            total_steps: all_steps.len(),
        });
    }

    Ok(DetectionPreparation::Run(DetectionRun {
        targets: to_detect,
        total_steps: all_steps.len(),
    }))
}

async fn execute_detection(
    db_client: &Client,
    bedrock: &BedrockClient,
    run_id: &str,
    detection_run: DetectionRun,
) -> StageResult<serde_json::Value> {
    info!(
        "{}/{} steps changed, generating patterns...",
        detection_run.targets.len(),
        detection_run.total_steps
    );

    delete_stale_patterns(db_client, &detection_run.targets).await?;

    let data_sources_context = default_data_sources();
    let context = DetectionContext {
        db_client,
        bedrock,
        run_id,
    };
    let total_patterns =
        generate_patterns(&context, &detection_run.targets, &data_sources_context).await?;

    info!(
        "Stage 6 complete: {} detection patterns generated.",
        total_patterns
    );
    Ok(json!({"patterns_generated": total_patterns}))
}

async fn ensure_trees_ready(db_client: &Client, run_id: &str) -> StageResult<()> {
    let stage5_status = db::get_stage_status(db_client, run_id, 5).await?;
    if matches!(
        stage5_status.as_deref(),
        Some("approved") | Some("completed")
    ) {
        return Ok(());
    }
    Err(format!(
        "Stage 5 status is '{:?}'. Trees must be approved first.",
        stage5_status
    )
    .into())
}

async fn load_step_contexts(
    db_client: &Client,
    trees: &[ExploitationTree],
) -> StageResult<Vec<StepContext>> {
    let mut contexts = Vec::new();
    for tree in trees {
        let steps = db::get_exploitation_steps(db_client, &tree.tree_id).await?;
        let summary = build_tree_summary(tree, &steps);
        for mut step in steps {
            step.policy_name = tree.policy_name.clone();
            contexts.push(StepContext {
                step,
                summary: summary.clone(),
            });
        }
    }
    Ok(contexts)
}

fn changed_steps(
    step_contexts: &[StepContext],
    stored: &std::collections::HashMap<String, String>,
) -> Vec<DetectionTarget> {
    step_contexts
        .iter()
        .filter_map(|context| {
            let hash = step_hash(&context.step);
            (stored.get(&context.step.step_id).map(|s| s.as_str()) != Some(&hash)).then(|| {
                DetectionTarget {
                    step: context.step.clone(),
                    summary: context.summary.clone(),
                    hash,
                }
            })
        })
        .collect()
}

fn step_hash(step: &ExploitationStep) -> String {
    let mut sorted = step.enabling_qualities.clone();
    sorted.sort();
    compute_hash(&[&step.description, &sorted.join(":")])
}

async fn delete_stale_patterns(db_client: &Client, targets: &[DetectionTarget]) -> StageResult<()> {
    for target in targets {
        db::delete_patterns_for_step(db_client, &target.step.step_id).await?;
    }
    Ok(())
}

async fn generate_patterns(
    context: &DetectionContext<'_>,
    targets: &[DetectionTarget],
    data_sources_context: &str,
) -> StageResult<usize> {
    let mut total_patterns = 0;
    for target in targets {
        total_patterns += generate_patterns_for_step(context, target, data_sources_context).await?;
        db::record_processing(
            context.db_client,
            6,
            &target.step.step_id,
            &target.hash,
            context.run_id,
        )
        .await?;
    }
    Ok(total_patterns)
}

async fn generate_patterns_for_step(
    context: &DetectionContext<'_>,
    target: &DetectionTarget,
    data_sources_context: &str,
) -> StageResult<usize> {
    let result = invoke_detection(context.bedrock, target, data_sources_context).await?;
    let patterns = response_patterns(result);
    for (index, pattern_data) in patterns.iter().enumerate() {
        let pattern = detection_pattern(context.run_id, &target.step, pattern_data, index);
        db::insert_detection_pattern(context.db_client, context.run_id, &pattern).await?;
    }
    Ok(patterns.len())
}

async fn invoke_detection(
    bedrock: &BedrockClient,
    target: &DetectionTarget,
    data_sources_context: &str,
) -> StageResult<serde_json::Value> {
    let qualities_str = target.step.enabling_qualities.join(", ");
    let prompt = BedrockClient::render_prompt(
        STAGE6_DETECT_PROMPT,
        &[
            (
                "policy_name",
                target.step.policy_name.as_deref().unwrap_or(""),
            ),
            ("step_title", &target.step.title),
            ("step_description", &target.step.description),
            (
                "step_actor_action",
                target.step.actor_action.as_deref().unwrap_or(""),
            ),
            ("step_qualities", &qualities_str),
            ("tree_summary", &target.summary),
            ("data_sources", data_sources_context),
        ],
    );
    bedrock
        .invoke_json(&prompt, SYSTEM_PROMPT, None, Some(8192))
        .await
}

fn response_patterns(response: serde_json::Value) -> Vec<serde_json::Value> {
    if response.is_array() {
        return response.as_array().cloned().unwrap_or_default();
    }
    if let Some(patterns) = response.get("patterns").and_then(|p| p.as_array()) {
        return patterns.clone();
    }
    vec![response]
}

fn detection_pattern(
    run_id: &str,
    step: &ExploitationStep,
    pattern_data: &serde_json::Value,
    index: usize,
) -> DetectionPattern {
    DetectionPattern {
        pattern_id: pattern_id(&step.step_id, index),
        run_id: run_id.to_string(),
        step_id: Some(step.step_id.clone()),
        prediction_id: None,
        data_source: json_string(pattern_data, "data_source"),
        anomaly_signal: json_string(pattern_data, "anomaly_signal"),
        baseline: json_opt_string(pattern_data, "baseline"),
        false_positive_risk: json_opt_string(pattern_data, "false_positive_risk"),
        detection_latency: json_opt_string(pattern_data, "detection_latency"),
        priority: json_opt_string(pattern_data, "priority"),
        implementation_notes: json_opt_string(pattern_data, "implementation_notes"),
        created_at: String::new(),
        step_title: None,
        tree_id: None,
        policy_name: None,
    }
}

fn pattern_id(step_id: &str, index: usize) -> String {
    let mut hasher = Sha256::new();
    hasher.update(format!("{}:pat:{}", step_id, index));
    format!("{:x}", hasher.finalize())[..12].to_string()
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

fn build_tree_summary(tree: &ExploitationTree, steps: &[ExploitationStep]) -> String {
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
