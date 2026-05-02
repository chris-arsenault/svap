//! Stage 4A: Policy Triage -- Shallow Vulnerability Ranking

use serde_json::json;
use sha2::{Digest, Sha256};
use tokio_postgres::Client;
use tracing::{info, warn};

use crate::bedrock::BedrockClient;
use crate::db;
use crate::types::{Case, Config, Policy, TaxonomyQuality, TriageResult};

const TRIAGE_SYSTEM: &str = "You are an expert healthcare policy analyst assessing structural vulnerability to fraud. You rank policies by how many vulnerability qualities are likely present based on how the program actually operates.";

const STAGE4A_TRIAGE_PROMPT: &str = include_str!("../../prompts/stage4a_triage.txt");
type StageResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

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
    info!("Stage 4A: Policy Triage");
    db::log_stage_start(db_client, run_id, 40).await?;

    match run_inner(db_client, bedrock, run_id, config).await {
        Ok(result) => {
            db::log_stage_complete(db_client, run_id, 40, Some(&result)).await?;
            Ok(result)
        }
        Err(e) => {
            db::log_stage_failed(db_client, run_id, 40, &e.to_string()).await?;
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
    let policies = db::get_policies(db_client).await?;
    let taxonomy = db::get_approved_taxonomy(db_client).await?;
    let cases = db::get_cases(db_client).await?;

    if policies.is_empty() || taxonomy.is_empty() {
        return Ok(json!({"policies_triaged": 0}));
    }

    let h = triage_inputs_hash(&policies, &taxonomy, &cases);
    let stored = db::get_processing_hashes(db_client, 40).await?;
    if stored.get("triage_batch").map(|s| s.as_str()) == Some(&h) {
        info!("Triage inputs unchanged. Skipping.");
        return Ok(json!({"policies_triaged": 0, "skipped": true}));
    }

    let prompt = build_triage_prompt(&policies, &taxonomy, &cases);
    let result = bedrock
        .invoke_json(&prompt, TRIAGE_SYSTEM, None, Some(8192))
        .await?;
    let rankings = result
        .get("rankings")
        .and_then(|r| r.as_array())
        .cloned()
        .unwrap_or_default();

    let stored_count = store_rankings(db_client, run_id, &policies, &rankings).await?;

    db::record_processing(db_client, 40, "triage_batch", &h, run_id).await?;
    info!("Triage complete: {} policies ranked.", stored_count);
    Ok(json!({"policies_triaged": stored_count, "total_rankings": rankings.len()}))
}

fn triage_inputs_hash(policies: &[Policy], taxonomy: &[TaxonomyQuality], cases: &[Case]) -> String {
    let mut policy_ids: Vec<&str> = policies.iter().map(|p| p.policy_id.as_str()).collect();
    let mut tax_ids: Vec<&str> = taxonomy.iter().map(|q| q.quality_id.as_str()).collect();
    let mut case_ids: Vec<&str> = cases.iter().map(|c| c.case_id.as_str()).collect();
    policy_ids.sort();
    tax_ids.sort();
    case_ids.sort();
    compute_hash(&[
        &policy_ids.join(":"),
        &tax_ids.join(":"),
        &case_ids.join(":"),
    ])
}

fn build_triage_prompt(
    policies: &[Policy],
    taxonomy: &[TaxonomyQuality],
    cases: &[Case],
) -> String {
    let taxonomy_summary = taxonomy_summary(taxonomy);
    let case_summary = case_summary(cases);
    let policy_list = policy_list(policies);
    BedrockClient::render_prompt(
        STAGE4A_TRIAGE_PROMPT,
        &[
            ("n_policies", &policies.len().to_string()),
            ("taxonomy_summary", &taxonomy_summary),
            ("case_summary", &case_summary),
            ("policy_list", &policy_list),
        ],
    )
}

fn taxonomy_summary(taxonomy: &[TaxonomyQuality]) -> String {
    taxonomy
        .iter()
        .map(|q| {
            format!(
                "{}: {}\n  Definition: {}\n  Recognition test: {}",
                q.quality_id, q.name, q.definition, q.recognition_test
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn case_summary(cases: &[Case]) -> String {
    cases
        .iter()
        .take(20)
        .map(|c| {
            format!(
                "- {}: {}\n  Enabling condition: {}",
                c.case_name,
                &c.exploited_policy[..c.exploited_policy.len().min(100)],
                &c.enabling_condition[..c.enabling_condition.len().min(150)]
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn policy_list(policies: &[Policy]) -> String {
    policies
        .iter()
        .map(|p| {
            let desc = p.description.as_deref().unwrap_or("No description");
            let truncated: String = desc.chars().take(200).collect();
            format!("- {}: {}", p.name, truncated)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

async fn store_rankings(
    db_client: &Client,
    run_id: &str,
    policies: &[Policy],
    rankings: &[serde_json::Value],
) -> StageResult<usize> {
    let mut stored_count = 0;
    for (i, entry) in rankings.iter().enumerate() {
        let policy_name = entry
            .get("policy_name")
            .and_then(|n| n.as_str())
            .unwrap_or("");
        let Some(policy_id) = resolve_policy_id(policy_name, policies) else {
            warn!("Could not match policy '{}'", policy_name);
            continue;
        };

        let triage = triage_result(entry, &policy_id, i);
        db::insert_triage_result(db_client, run_id, &triage).await?;
        db::update_policy_lifecycle(db_client, &policy_id, "triaged").await?;
        stored_count += 1;
    }
    Ok(stored_count)
}

fn triage_result(entry: &serde_json::Value, policy_id: &str, index: usize) -> TriageResult {
    TriageResult {
        policy_id: policy_id.to_string(),
        triage_score: entry.get("score").and_then(|s| s.as_f64()).unwrap_or(0.0),
        rationale: entry
            .get("rationale")
            .and_then(|r| r.as_str())
            .unwrap_or("")
            .to_string(),
        uncertainty: entry
            .get("uncertainty")
            .and_then(|u| u.as_str())
            .map(String::from),
        priority_rank: (index + 1) as i32,
        policy_name: None,
        run_id: None,
    }
}

fn resolve_policy_id(name: &str, policies: &[Policy]) -> Option<String> {
    let name_lower = name.to_lowercase();
    // Exact match
    for p in policies {
        if p.name.to_lowercase() == name_lower {
            return Some(p.policy_id.clone());
        }
    }
    // Fuzzy match
    for p in policies {
        let pname = p.name.to_lowercase();
        if name_lower.contains(&pname) || pname.contains(&name_lower) {
            return Some(p.policy_id.clone());
        }
    }
    None
}
