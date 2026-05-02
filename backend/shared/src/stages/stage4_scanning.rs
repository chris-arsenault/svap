//! Stage 4: Policy Corpus Scanning

use serde_json::json;
use sha2::{Digest, Sha256};
use tokio_postgres::Client;
use tracing::info;

use crate::bedrock::BedrockClient;
use crate::db;
use crate::rag::ContextAssembler;
use crate::types::{Config, Policy, TaxonomyQuality};

const SYSTEM_CHARACTERIZE: &str = "You are a structural analyst characterizing how a government policy or program works. Focus on mechanical structure: money flows, verification, barriers. Do not evaluate whether the policy is good or bad.";
const SYSTEM_SCORE: &str = "You are scoring a policy against a structural vulnerability taxonomy. Apply each recognition test. A quality is PRESENT only if the structural characterization clearly shows the property. Be conservative.";

const STAGE4_CHARACTERIZE_PROMPT: &str = include_str!("../../prompts/stage4_characterize.txt");
const STAGE4_SCORE_PROMPT: &str = include_str!("../../prompts/stage4_score.txt");
type StageResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

struct PolicyScanInputs {
    context: ContextAssembler,
    taxonomy: Vec<TaxonomyQuality>,
    taxonomy_context: String,
    threshold: i32,
    policies: Vec<Policy>,
}

struct PolicyScoreRun {
    delta_policies: Vec<(Policy, String)>,
    skipped: usize,
}

enum PolicyScorePreparation {
    Unchanged { skipped: usize },
    Run(PolicyScoreRun),
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
) -> StageResult<serde_json::Value> {
    let Some(inputs) = policy_scan_inputs(db_client, config).await? else {
        return Ok(json!({"policies_scored": 0}));
    };
    characterize_and_score_policies(db_client, bedrock, run_id, inputs).await
}

async fn policy_scan_inputs(
    db_client: &Client,
    config: &Config,
) -> StageResult<Option<PolicyScanInputs>> {
    let taxonomy = db::get_approved_taxonomy(db_client).await?;
    let calibration = db::get_calibration(db_client).await?;
    if taxonomy.is_empty() {
        return Err("No taxonomy found. Run Stages 1-3 first.".into());
    }

    let ctx = ContextAssembler::new(config);
    let taxonomy_context = ContextAssembler::format_taxonomy_context(&taxonomy);
    let threshold = calibration.as_ref().map(|c| c.threshold).unwrap_or(3);

    let policies = db::get_policies(db_client).await?;
    if policies.is_empty() {
        info!("No policies found.");
        return Ok(None);
    }
    Ok(Some(PolicyScanInputs {
        context: ctx,
        taxonomy,
        taxonomy_context,
        threshold,
        policies,
    }))
}

async fn characterize_and_score_policies(
    db_client: &Client,
    bedrock: &BedrockClient,
    run_id: &str,
    inputs: PolicyScanInputs,
) -> StageResult<serde_json::Value> {
    characterize_missing_policies(db_client, bedrock, &inputs.context, &inputs.policies).await?;

    match prepare_policy_scoring(db_client, &inputs.taxonomy).await? {
        PolicyScorePreparation::Unchanged { skipped } => {
            info!("All policies unchanged. Skipping scoring.");
            Ok(json!({"policies_scored": 0, "skipped_unchanged": skipped}))
        }
        PolicyScorePreparation::Run(score_run) => {
            score_policy_run(db_client, bedrock, run_id, &inputs, score_run).await
        }
    }
}

async fn prepare_policy_scoring(
    db_client: &Client,
    taxonomy: &[TaxonomyQuality],
) -> StageResult<PolicyScorePreparation> {
    let policies = db::get_policies(db_client).await?;
    let policies_to_score = policies_without_assessments(db_client, &policies).await?;
    let tax_fp = taxonomy_fingerprint(taxonomy);
    let stored_hashes = db::get_processing_hashes(db_client, 4).await?;
    let (delta_policies, skipped) = changed_policies(&policies_to_score, &stored_hashes, &tax_fp);
    if delta_policies.is_empty() {
        Ok(PolicyScorePreparation::Unchanged { skipped })
    } else {
        Ok(PolicyScorePreparation::Run(PolicyScoreRun {
            delta_policies,
            skipped,
        }))
    }
}

async fn score_policy_run(
    db_client: &Client,
    bedrock: &BedrockClient,
    run_id: &str,
    inputs: &PolicyScanInputs,
    score_run: PolicyScoreRun,
) -> StageResult<serde_json::Value> {
    // 4b: Vulnerability Scoring
    info!(
        "Scoring {} policies ({} unchanged)...",
        score_run.delta_policies.len(),
        score_run.skipped
    );
    let results = score_policies(
        db_client,
        bedrock,
        run_id,
        &inputs.taxonomy_context,
        &score_run.delta_policies,
    )
    .await?;

    let above_threshold = count_above_threshold(&results, inputs.threshold);
    info!("Stage 4 complete: {} policies scored.", results.len());
    Ok(json!({"policies_scored": results.len(), "above_threshold": above_threshold}))
}

fn count_above_threshold(results: &[serde_json::Value], threshold: i32) -> usize {
    results
        .iter()
        .filter(|r| {
            r.get("convergence_score")
                .and_then(|c| c.as_i64())
                .unwrap_or(0)
                >= threshold as i64
        })
        .count()
}

async fn characterize_missing_policies(
    db_client: &Client,
    bedrock: &BedrockClient,
    ctx: &ContextAssembler,
    policies: &[Policy],
) -> StageResult<()> {
    info!("Characterizing {} policies...", policies.len());
    for policy in policies
        .iter()
        .filter(|policy| policy.structural_characterization.is_none())
    {
        let characterization = characterize_policy(db_client, bedrock, ctx, policy).await?;
        let mut updated = policy.clone();
        updated.structural_characterization = Some(characterization);
        db::insert_policy(db_client, &updated).await?;
    }
    Ok(())
}

async fn characterize_policy(
    db_client: &Client,
    bedrock: &BedrockClient,
    ctx: &ContextAssembler,
    policy: &Policy,
) -> StageResult<String> {
    info!("Characterizing: {}", policy.name);
    let rag_context = ctx
        .retrieve(db_client, &policy_query(policy), Some("policy"), None)
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
            ("rag_context", rag_context_or_empty(&rag_context)),
        ],
    );
    bedrock
        .invoke(&prompt, SYSTEM_CHARACTERIZE, None, Some(2048))
        .await
}

fn policy_query(policy: &Policy) -> String {
    format!(
        "{} {}",
        policy.name,
        policy.description.as_deref().unwrap_or("")
    )
}

fn rag_context_or_empty(rag_context: &str) -> &str {
    if rag_context.is_empty() {
        "No additional source documents available."
    } else {
        rag_context
    }
}

async fn policies_without_assessments(
    db_client: &Client,
    policies: &[Policy],
) -> StageResult<Vec<Policy>> {
    let mut policies_to_score = Vec::new();
    for policy in policies {
        let existing = db::get_quality_assessments(db_client, Some(&policy.policy_id)).await?;
        if existing.is_empty() {
            policies_to_score.push(policy.clone());
        } else {
            info!(
                "Skipping {} -- already assessed via deep research",
                policy.name
            );
        }
    }
    Ok(policies_to_score)
}

fn changed_policies(
    policies: &[Policy],
    stored_hashes: &std::collections::HashMap<String, String>,
    tax_fp: &str,
) -> (Vec<(Policy, String)>, usize) {
    let mut changed = Vec::new();
    let mut skipped = 0;
    for policy in policies {
        let hash = compute_hash(&[
            policy.structural_characterization.as_deref().unwrap_or(""),
            tax_fp,
        ]);
        if stored_hashes.get(&policy.policy_id).map(|s| s.as_str()) == Some(&hash) {
            skipped += 1;
        } else {
            changed.push((policy.clone(), hash));
        }
    }
    (changed, skipped)
}

async fn score_policies(
    db_client: &Client,
    bedrock: &BedrockClient,
    run_id: &str,
    taxonomy_context: &str,
    policies: &[(Policy, String)],
) -> StageResult<Vec<serde_json::Value>> {
    let mut results = Vec::new();
    for (policy, hash) in policies {
        let convergence_count =
            score_policy(db_client, bedrock, run_id, taxonomy_context, policy).await?;
        db::record_processing(db_client, 4, &policy.policy_id, hash, run_id).await?;
        results.push(json!({"policy": policy.name, "convergence_score": convergence_count}));
    }
    Ok(results)
}

async fn score_policy(
    db_client: &Client,
    bedrock: &BedrockClient,
    run_id: &str,
    taxonomy_context: &str,
    policy: &Policy,
) -> StageResult<i32> {
    info!("Scoring: {}", policy.name);
    let scores = policy_scores(bedrock, taxonomy_context, policy).await?;
    insert_policy_scores(db_client, run_id, policy, &scores).await
}

async fn policy_scores(
    bedrock: &BedrockClient,
    taxonomy_context: &str,
    policy: &Policy,
) -> StageResult<serde_json::Value> {
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
            ("taxonomy", taxonomy_context),
        ],
    );
    bedrock
        .invoke_json(&prompt, SYSTEM_SCORE, None, Some(2048))
        .await
}

async fn insert_policy_scores(
    db_client: &Client,
    run_id: &str,
    policy: &Policy,
    scores: &serde_json::Value,
) -> StageResult<i32> {
    let mut convergence_count = 0;
    let Some(obj) = scores.get("scores").unwrap_or(scores).as_object() else {
        return Ok(convergence_count);
    };

    for (quality_id, score_data) in obj {
        let (present, evidence) = parse_score(score_data);
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
    Ok(convergence_count)
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
