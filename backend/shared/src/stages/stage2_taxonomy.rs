//! Stage 2: Vulnerability Taxonomy Extraction
//!
//! Three-pass iterative: cluster, refine, semantic dedup.
//! Human gate if novel draft qualities are added.

use serde_json::json;
use sha2::{Digest, Sha256};
use tokio_postgres::Client;
use tracing::info;

use crate::bedrock::BedrockClient;
use crate::db;
use crate::types::{Config, TaxonomyQuality};

const SYSTEM_CLUSTER: &str = "You are a structural analyst. Your task is to find the abstract patterns that make policies exploitable. You think in terms of system design properties -- payment timing, verification architecture, information asymmetry, barrier structures -- not in terms of specific domains or actors.";
const SYSTEM_REFINE: &str = "You are refining a taxonomy of structural vulnerability qualities. Each quality must be precise enough that two independent analysts would agree on whether a given policy has it.";
const SYSTEM_DEDUP: &str = "You are a taxonomy curator comparing a newly extracted vulnerability quality against an existing approved taxonomy. Determine whether the new quality is semantically equivalent to any existing quality.";

const STAGE2_CLUSTER_PROMPT: &str = include_str!("../../prompts/stage2_cluster.txt");
const STAGE2_REFINE_PROMPT: &str = include_str!("../../prompts/stage2_refine.txt");
const STAGE2_DEDUP_PROMPT: &str = include_str!("../../prompts/stage2_dedup.txt");

pub async fn run(
    db_client: &Client,
    bedrock: &BedrockClient,
    run_id: &str,
    config: &Config,
) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
    info!("Stage 2: Vulnerability Taxonomy Extraction");
    db::log_stage_start(db_client, run_id, 2).await?;

    match run_inner(db_client, bedrock, run_id, config).await {
        Ok(result) => Ok(result),
        Err(e) => {
            db::log_stage_failed(db_client, run_id, 2, &e.to_string()).await?;
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
    let cases = db::get_cases(db_client).await?;
    if cases.is_empty() {
        return Err("No cases found. Run Stage 1 first.".into());
    }

    let processed_ids = db::get_taxonomy_processed_case_ids(db_client).await?;
    let new_cases: Vec<_> = cases
        .iter()
        .filter(|c| !processed_ids.contains(&c.case_id))
        .collect();

    if new_cases.is_empty() {
        let taxonomy = db::get_taxonomy(db_client).await?;
        info!("All cases already processed. Nothing to extract.");
        db::log_stage_complete(
            db_client,
            run_id,
            2,
            Some(&json!({"qualities_total": taxonomy.len(), "cases_processed": 0, "note": "no new cases"})),
        )
        .await?;
        return Ok(json!({"qualities_total": taxonomy.len(), "cases_processed": 0}));
    }

    info!(
        "{} new cases to process ({} already processed)",
        new_cases.len(),
        cases.len() - new_cases.len()
    );

    // Pass 1: Cluster
    info!("Pass 1: Clustering enabling conditions...");
    let enabling_conditions: String = new_cases
        .iter()
        .map(|c| {
            format!(
                "CASE: {}\nENABLING CONDITION: {}",
                c.case_name, c.enabling_condition
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n");

    let cluster_prompt = BedrockClient::render_prompt(
        STAGE2_CLUSTER_PROMPT,
        &[
            ("enabling_conditions", &enabling_conditions),
            ("num_cases", &new_cases.len().to_string()),
        ],
    );
    let clusters = bedrock
        .invoke_json(&cluster_prompt, SYSTEM_CLUSTER, None, Some(4096))
        .await?;
    let qualities_draft: Vec<serde_json::Value> = if clusters.is_array() {
        clusters.as_array().cloned().unwrap_or_default()
    } else {
        clusters
            .get("qualities")
            .and_then(|q| q.as_array())
            .cloned()
            .unwrap_or_default()
    };
    info!("Identified {} draft qualities.", qualities_draft.len());

    // Pass 2: Refine
    info!("Pass 2: Refining each quality...");
    let all_names: Vec<String> = qualities_draft
        .iter()
        .filter_map(|q| q.get("name").and_then(|n| n.as_str()).map(String::from))
        .collect();
    let mut refined_qualities = Vec::new();

    for draft in &qualities_draft {
        let name = draft
            .get("name")
            .and_then(|n| n.as_str())
            .unwrap_or("")
            .to_string();
        info!("Refining: {}", name);

        let other_names: String = all_names
            .iter()
            .filter(|n| n.as_str() != name)
            .cloned()
            .collect::<Vec<_>>()
            .join(", ");

        let examples = draft
            .get("enabling_conditions")
            .map(|e| serde_json::to_string_pretty(e).unwrap_or_default())
            .unwrap_or_default();

        let refine_prompt = BedrockClient::render_prompt(
            STAGE2_REFINE_PROMPT,
            &[
                ("quality_name", &name),
                (
                    "quality_definition",
                    draft
                        .get("definition")
                        .and_then(|d| d.as_str())
                        .unwrap_or(""),
                ),
                ("example_conditions", &examples),
                ("other_quality_names", &other_names),
            ],
        );
        let refined = bedrock
            .invoke_json(&refine_prompt, SYSTEM_REFINE, None, Some(2048))
            .await?;
        let final_name = refined
            .get("name")
            .and_then(|n| n.as_str())
            .unwrap_or(&name)
            .to_string();

        let mut hasher = Sha256::new();
        hasher.update(final_name.as_bytes());
        let quality_id = format!("{:x}", hasher.finalize())[..8].to_string();

        refined_qualities.push(TaxonomyQuality {
            quality_id,
            name: final_name,
            definition: refined
                .get("definition")
                .and_then(|d| d.as_str())
                .unwrap_or("")
                .to_string(),
            recognition_test: refined
                .get("recognition_test")
                .and_then(|r| r.as_str())
                .unwrap_or("")
                .to_string(),
            exploitation_logic: refined
                .get("exploitation_logic")
                .and_then(|e| e.as_str())
                .unwrap_or("")
                .to_string(),
            canonical_examples: refined.get("canonical_examples").cloned(),
            review_status: Some("draft".to_string()),
            reviewer_notes: None,
            created_at: String::new(),
            color: None,
            case_count: None,
        });
    }

    // Pass 3: Semantic dedup
    info!("Pass 3: Semantic deduplication...");
    let mut existing = db::get_taxonomy(db_client).await?;
    let mut novel_qualities = Vec::new();
    let mut merged_count = 0;

    for draft in &refined_qualities {
        if existing.is_empty() {
            info!("NOVEL: '{}' -- adding as draft", draft.name);
            db::insert_quality(db_client, draft).await?;
            novel_qualities.push(draft.clone());
            existing.push(draft.clone());
            continue;
        }

        let existing_text: String = existing
            .iter()
            .map(|q| {
                format!(
                    "ID: {}\nName: {}\nDefinition: {}\nExploitation Logic: {}",
                    q.quality_id, q.name, q.definition, q.exploitation_logic
                )
            })
            .collect::<Vec<_>>()
            .join("\n\n");

        let dedup_prompt = BedrockClient::render_prompt(
            STAGE2_DEDUP_PROMPT,
            &[
                ("new_name", &draft.name),
                ("new_definition", &draft.definition),
                ("new_exploitation_logic", &draft.exploitation_logic),
                ("existing_taxonomy", &existing_text),
            ],
        );
        let result = bedrock
            .invoke_json(&dedup_prompt, SYSTEM_DEDUP, None, Some(1024))
            .await?;

        let is_match = result
            .get("match")
            .and_then(|m| m.as_bool())
            .unwrap_or(false);
        let matched_id = result.get("existing_quality_id").and_then(|id| id.as_str());

        if is_match {
            if let Some(mid) = matched_id {
                if existing.iter().any(|q| q.quality_id == mid) {
                    info!("MERGED: '{}' -> existing '{}'", draft.name, mid);
                    if let Some(examples) = &draft.canonical_examples {
                        if let Some(arr) = examples.as_array() {
                            let strs: Vec<String> = arr
                                .iter()
                                .filter_map(|v| v.as_str().map(String::from))
                                .collect();
                            db::merge_quality_examples(db_client, mid, &strs).await?;
                        }
                    }
                    merged_count += 1;
                    continue;
                }
            }
        }

        info!("NOVEL: '{}' -- adding as draft", draft.name);
        db::insert_quality(db_client, draft).await?;
        novel_qualities.push(draft.clone());
        existing.push(draft.clone());
    }

    // Record all new cases as processed
    for case in &new_cases {
        db::record_taxonomy_case_processed(db_client, &case.case_id).await?;
    }

    let taxonomy = db::get_taxonomy(db_client).await?;

    // Human gate if novel qualities need review
    if !novel_qualities.is_empty() {
        db::log_stage_pending_review(db_client, run_id, 2).await?;
        info!(
            "HUMAN REVIEW REQUIRED -- {} new draft qualities need approval.",
            novel_qualities.len()
        );
    } else {
        db::log_stage_complete(
            db_client,
            run_id,
            2,
            Some(&json!({
                "qualities_total": taxonomy.len(),
                "cases_processed": new_cases.len(),
                "merged": merged_count,
                "novel": 0,
            })),
        )
        .await?;
    }

    Ok(json!({
        "qualities_total": taxonomy.len(),
        "cases_processed": new_cases.len(),
        "merged": merged_count,
        "novel": novel_qualities.len(),
    }))
}
