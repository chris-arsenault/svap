//! Stage 5: Exploitation Tree Generation

use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use tokio_postgres::Client;
use tracing::info;

use crate::bedrock::BedrockClient;
use crate::db;
use crate::types::{
    Config, ExploitationStep, ExploitationTree, Policy, PolicyScore, TaxonomyQuality,
};

const SYSTEM_PROMPT: &str = "You are a structural analyst building exploitation decision trees. Every step must be CAUSED by a specific vulnerability quality. If you cannot cite which structural quality enables a step, do not include it.";

const STAGE5_PREDICT_PROMPT: &str = include_str!("../../prompts/stage5_predict.txt");
type StageResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;
type PolicyProfile = (String, Vec<String>, i32);

struct PredictionContext<'a> {
    db_client: &'a Client,
    bedrock: &'a BedrockClient,
    run_id: &'a str,
}

struct PredictionTarget {
    policy_id: String,
    name: String,
    qualities: Vec<String>,
    count: i32,
    hash: String,
}

struct PredictionRun {
    taxonomy: Vec<TaxonomyQuality>,
    policies: Vec<Policy>,
    policy_scores: Vec<PolicyScore>,
    targets: Vec<PredictionTarget>,
    high_risk_count: usize,
}

enum PredictionPreparation {
    NoHighRisk { threshold: i32 },
    Unchanged { high_risk_count: usize },
    Run(PredictionRun),
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
) -> StageResult<serde_json::Value> {
    match prepare_predictions(db_client).await? {
        PredictionPreparation::NoHighRisk { threshold } => {
            info!("No policies scored at or above threshold ({}).", threshold);
            db::log_stage_complete(db_client, run_id, 5, Some(&json!({"trees_generated": 0})))
                .await?;
            Ok(json!({"trees_generated": 0}))
        }
        PredictionPreparation::Unchanged { high_risk_count } => {
            info!("All high-risk policies unchanged -- skipping.");
            db::log_stage_complete(
                db_client,
                run_id,
                5,
                Some(&json!({"trees_generated": 0, "skipped_unchanged": high_risk_count})),
            )
            .await?;
            Ok(json!({"trees_generated": 0}))
        }
        PredictionPreparation::Run(prediction_run) => {
            execute_predictions(db_client, bedrock, run_id, prediction_run).await
        }
    }
}

async fn prepare_predictions(db_client: &Client) -> StageResult<PredictionPreparation> {
    let taxonomy = db::get_approved_taxonomy(db_client).await?;
    let calibration = db::get_calibration(db_client).await?;
    let threshold = calibration.as_ref().map(|c| c.threshold).unwrap_or(3);

    let policies = db::get_policies(db_client).await?;
    let policy_scores = db::get_policy_scores(db_client).await?;
    let profiles = build_profiles(&policy_scores);
    let high_risk_count = profiles
        .values()
        .filter(|(_, _, count)| *count >= threshold)
        .count();

    if high_risk_count == 0 {
        return Ok(PredictionPreparation::NoHighRisk { threshold });
    }

    // Delta detection
    let cal_fp = calibration
        .as_ref()
        .map(|c| c.threshold.to_string())
        .unwrap_or_else(|| "3".to_string());
    let stored = db::get_processing_hashes(db_client, 5).await?;
    let to_predict = changed_prediction_targets(&profiles, threshold, &stored, &cal_fp);

    if to_predict.is_empty() {
        return Ok(PredictionPreparation::Unchanged { high_risk_count });
    }

    Ok(PredictionPreparation::Run(PredictionRun {
        taxonomy,
        policies,
        policy_scores,
        targets: to_predict,
        high_risk_count,
    }))
}

async fn execute_predictions(
    db_client: &Client,
    bedrock: &BedrockClient,
    run_id: &str,
    prediction_run: PredictionRun,
) -> StageResult<serde_json::Value> {
    info!(
        "{}/{} policies changed, generating exploitation trees...",
        prediction_run.targets.len(),
        prediction_run.high_risk_count
    );

    delete_stale_trees(db_client, &prediction_run.targets).await?;

    let quality_lookup: HashMap<&str, &TaxonomyQuality> = prediction_run
        .taxonomy
        .iter()
        .map(|q| (q.quality_id.as_str(), q))
        .collect();
    let context = PredictionContext {
        db_client,
        bedrock,
        run_id,
    };
    let total_steps = generate_trees(
        &context,
        &prediction_run.targets,
        &prediction_run.policies,
        &prediction_run.policy_scores,
        &quality_lookup,
    )
    .await?;

    complete_prediction_stage(db_client, run_id, total_steps, prediction_run.targets.len()).await?;
    Ok(json!({"trees_generated": prediction_run.targets.len(), "steps_generated": total_steps}))
}

async fn complete_prediction_stage(
    db_client: &Client,
    run_id: &str,
    total_steps: usize,
    tree_count: usize,
) -> StageResult<()> {
    if total_steps > 0 {
        db::log_stage_pending_review(db_client, run_id, 5).await?;
        info!(
            "Stage 5 complete: {} steps across {} trees. HUMAN REVIEW REQUIRED.",
            total_steps, tree_count
        );
        return Ok(());
    }

    db::log_stage_complete(
        db_client,
        run_id,
        5,
        Some(&json!({"trees_generated": 0, "steps_generated": 0})),
    )
    .await
}

fn build_profiles(policy_scores: &[PolicyScore]) -> HashMap<String, PolicyProfile> {
    let mut profiles = HashMap::new();
    for score in policy_scores {
        let entry =
            profiles
                .entry(score.policy_id.clone())
                .or_insert((score.name.clone(), Vec::new(), 0));
        if score.present {
            entry.1.push(score.quality_id.clone());
            entry.2 += 1;
        }
    }
    profiles
}

fn changed_prediction_targets(
    profiles: &HashMap<String, PolicyProfile>,
    threshold: i32,
    stored: &HashMap<String, String>,
    cal_fp: &str,
) -> Vec<PredictionTarget> {
    profiles
        .iter()
        .filter(|(_, (_, _, count))| *count >= threshold)
        .filter_map(|(policy_id, (name, qualities, count))| {
            let hash = prediction_hash(qualities, cal_fp);
            (stored.get(policy_id).map(|s| s.as_str()) != Some(&hash)).then(|| PredictionTarget {
                policy_id: policy_id.clone(),
                name: name.clone(),
                qualities: qualities.clone(),
                count: *count,
                hash,
            })
        })
        .collect()
}

fn prediction_hash(qualities: &[String], cal_fp: &str) -> String {
    let mut sorted_quals = qualities.to_vec();
    sorted_quals.sort();
    compute_hash(&[&sorted_quals.join(":"), cal_fp])
}

async fn delete_stale_trees(db_client: &Client, targets: &[PredictionTarget]) -> StageResult<()> {
    for target in targets {
        db::delete_tree_for_policy(db_client, &target.policy_id).await?;
    }
    Ok(())
}

async fn generate_trees(
    context: &PredictionContext<'_>,
    targets: &[PredictionTarget],
    policies: &[Policy],
    policy_scores: &[PolicyScore],
    quality_lookup: &HashMap<&str, &TaxonomyQuality>,
) -> StageResult<usize> {
    let mut total_steps = 0;
    for target in targets {
        let Some(policy) = policies.iter().find(|p| p.policy_id == target.policy_id) else {
            continue;
        };
        total_steps +=
            generate_tree(context, target, policy, policy_scores, quality_lookup).await?;
    }
    Ok(total_steps)
}

async fn generate_tree(
    context: &PredictionContext<'_>,
    target: &PredictionTarget,
    policy: &Policy,
    policy_scores: &[PolicyScore],
    quality_lookup: &HashMap<&str, &TaxonomyQuality>,
) -> StageResult<usize> {
    let result = invoke_prediction(
        context.bedrock,
        target,
        policy,
        policy_scores,
        quality_lookup,
    )
    .await?;
    let tree = exploitation_tree(context.run_id, target, &result);
    db::insert_exploitation_tree(context.db_client, context.run_id, &tree).await?;
    let step_count = insert_steps(context.db_client, &tree.tree_id, target, &result).await?;
    db::record_processing(
        context.db_client,
        5,
        &target.policy_id,
        &target.hash,
        context.run_id,
    )
    .await?;
    Ok(step_count)
}

async fn invoke_prediction(
    bedrock: &BedrockClient,
    target: &PredictionTarget,
    policy: &Policy,
    policy_scores: &[PolicyScore],
    quality_lookup: &HashMap<&str, &TaxonomyQuality>,
) -> StageResult<serde_json::Value> {
    let quality_descriptions = quality_descriptions(target, policy_scores, quality_lookup);
    let prompt = BedrockClient::render_prompt(
        STAGE5_PREDICT_PROMPT,
        &[
            ("policy_name", &target.name),
            ("policy_description", policy_description(policy)),
            ("convergence_score", &target.count.to_string()),
            ("quality_profile", &quality_descriptions),
        ],
    );
    bedrock
        .invoke_json(&prompt, SYSTEM_PROMPT, Some(0.3), Some(4096))
        .await
}

fn quality_descriptions(
    target: &PredictionTarget,
    policy_scores: &[PolicyScore],
    quality_lookup: &HashMap<&str, &TaxonomyQuality>,
) -> String {
    target
        .qualities
        .iter()
        .filter_map(|quality_id| {
            quality_lookup.get(quality_id.as_str()).map(|quality| {
                let evidence = score_evidence(policy_scores, &target.policy_id, quality_id);
                format!(
                    "- {} ({}): {}\n  How it manifests here: {}",
                    quality_id, quality.name, quality.definition, evidence
                )
            })
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn score_evidence<'a>(
    policy_scores: &'a [PolicyScore],
    policy_id: &str,
    quality_id: &str,
) -> &'a str {
    policy_scores
        .iter()
        .find(|score| {
            score.policy_id == policy_id && score.quality_id == quality_id && score.present
        })
        .and_then(|score| score.evidence.as_deref())
        .unwrap_or("")
}

fn policy_description(policy: &Policy) -> &str {
    policy
        .structural_characterization
        .as_deref()
        .unwrap_or(policy.description.as_deref().unwrap_or(""))
}

fn exploitation_tree(
    run_id: &str,
    target: &PredictionTarget,
    result: &serde_json::Value,
) -> ExploitationTree {
    ExploitationTree {
        tree_id: tree_id(&target.policy_id),
        policy_id: target.policy_id.clone(),
        convergence_score: target.count,
        actor_profile: json_opt_string(result, "actor_profile"),
        lifecycle_stage: json_opt_string(result, "lifecycle_stage"),
        detection_difficulty: json_opt_string(result, "detection_difficulty"),
        review_status: Some("draft".to_string()),
        reviewer_notes: None,
        run_id: Some(run_id.to_string()),
        created_at: String::new(),
        policy_name: Some(target.name.clone()),
        step_count: None,
        steps: Vec::new(),
    }
}

fn tree_id(policy_id: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(policy_id.as_bytes());
    format!("{:x}", hasher.finalize())[..12].to_string()
}

async fn insert_steps(
    db_client: &Client,
    tree_id: &str,
    target: &PredictionTarget,
    result: &serde_json::Value,
) -> StageResult<usize> {
    let steps = result
        .get("steps")
        .and_then(|s| s.as_array())
        .cloned()
        .unwrap_or_default();
    let mut order_to_id = HashMap::new();

    for step_data in &steps {
        let (step, quality_ids) = exploitation_step(tree_id, target, step_data, &mut order_to_id);
        db::insert_exploitation_step(db_client, &step, &quality_ids).await?;
    }

    Ok(steps.len())
}

fn exploitation_step(
    tree_id: &str,
    target: &PredictionTarget,
    step_data: &serde_json::Value,
    order_to_id: &mut HashMap<i64, String>,
) -> (ExploitationStep, Vec<String>) {
    let order = step_data
        .get("step_order")
        .and_then(|o| o.as_i64())
        .unwrap_or(0);
    let title = step_data
        .get("title")
        .and_then(|t| t.as_str())
        .unwrap_or("");
    let step_id = step_id(&target.policy_id, order, title);
    order_to_id.insert(order, step_id.clone());
    let parent_id = step_data
        .get("parent_step_order")
        .and_then(|p| p.as_i64())
        .and_then(|parent_order| order_to_id.get(&parent_order).cloned());

    (
        ExploitationStep {
            step_id,
            tree_id: tree_id.to_string(),
            parent_step_id: parent_id,
            step_order: order as i32,
            title: title.to_string(),
            description: json_string(step_data, "description"),
            actor_action: json_opt_string(step_data, "actor_action"),
            is_branch_point: step_data.get("is_branch_point").and_then(|b| b.as_bool()),
            branch_label: json_opt_string(step_data, "branch_label"),
            created_at: String::new(),
            policy_id: None,
            policy_name: None,
            enabling_qualities: Vec::new(),
        },
        json_string_array(step_data, "enabling_qualities"),
    )
}

fn step_id(policy_id: &str, order: i64, title: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(format!(
        "{}:step:{}:{}",
        policy_id,
        order,
        &title[..title.len().min(50)]
    ));
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

fn json_string_array(value: &serde_json::Value, key: &str) -> Vec<String> {
    value
        .get(key)
        .and_then(|q| q.as_array())
        .into_iter()
        .flatten()
        .filter_map(|v| v.as_str().map(String::from))
        .collect()
}
