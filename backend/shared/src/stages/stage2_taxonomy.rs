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
use crate::types::{Case, Config, TaxonomyQuality};

const SYSTEM_CLUSTER: &str = "You are a structural analyst. Your task is to find the abstract patterns that make policies exploitable. You think in terms of system design properties -- payment timing, verification architecture, information asymmetry, barrier structures -- not in terms of specific domains or actors.";
const SYSTEM_REFINE: &str = "You are refining a taxonomy of structural vulnerability qualities. Each quality must be precise enough that two independent analysts would agree on whether a given policy has it.";
const SYSTEM_DEDUP: &str = "You are a taxonomy curator comparing a newly extracted vulnerability quality against an existing approved taxonomy. Determine whether the new quality is semantically equivalent to any existing quality.";

const STAGE2_CLUSTER_PROMPT: &str = include_str!("../../prompts/stage2_cluster.txt");
const STAGE2_REFINE_PROMPT: &str = include_str!("../../prompts/stage2_refine.txt");
const STAGE2_DEDUP_PROMPT: &str = include_str!("../../prompts/stage2_dedup.txt");
type StageResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

struct DedupOutcome {
    novel: Vec<TaxonomyQuality>,
    merged_count: usize,
}

enum DedupAction {
    Merged,
    Novel(Box<TaxonomyQuality>),
}

pub async fn run(
    db_client: &Client,
    bedrock: &BedrockClient,
    run_id: &str,
    config: &Config,
) -> StageResult<serde_json::Value> {
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
) -> StageResult<serde_json::Value> {
    let cases = db::get_cases(db_client).await?;
    if cases.is_empty() {
        return Err("No cases found. Run Stage 1 first.".into());
    }

    let new_cases = new_taxonomy_cases(db_client, &cases).await?;

    if new_cases.is_empty() {
        return complete_no_new_cases(db_client, run_id).await;
    }

    info!(
        "{} new cases to process ({} already processed)",
        new_cases.len(),
        cases.len() - new_cases.len()
    );

    let qualities_draft = cluster_qualities(bedrock, &new_cases).await?;
    let refined_qualities = refine_qualities(bedrock, &qualities_draft).await?;
    let dedup = deduplicate_qualities(db_client, bedrock, &refined_qualities).await?;

    for case in &new_cases {
        db::record_taxonomy_case_processed(db_client, &case.case_id).await?;
    }

    let taxonomy = db::get_taxonomy(db_client).await?;
    complete_or_request_review(db_client, run_id, &taxonomy, new_cases.len(), &dedup).await?;

    Ok(json!({
        "qualities_total": taxonomy.len(),
        "cases_processed": new_cases.len(),
        "merged": dedup.merged_count,
        "novel": dedup.novel.len(),
    }))
}

async fn new_taxonomy_cases<'a>(
    db_client: &Client,
    cases: &'a [Case],
) -> StageResult<Vec<&'a Case>> {
    let processed_ids = db::get_taxonomy_processed_case_ids(db_client).await?;
    Ok(cases
        .iter()
        .filter(|case| !processed_ids.contains(&case.case_id))
        .collect())
}

async fn complete_no_new_cases(db_client: &Client, run_id: &str) -> StageResult<serde_json::Value> {
    let taxonomy = db::get_taxonomy(db_client).await?;
    info!("All cases already processed. Nothing to extract.");
    db::log_stage_complete(
        db_client,
        run_id,
        2,
        Some(&json!({
            "qualities_total": taxonomy.len(),
            "cases_processed": 0,
            "note": "no new cases"
        })),
    )
    .await?;
    Ok(json!({"qualities_total": taxonomy.len(), "cases_processed": 0}))
}

async fn cluster_qualities(
    bedrock: &BedrockClient,
    cases: &[&Case],
) -> StageResult<Vec<serde_json::Value>> {
    info!("Pass 1: Clustering enabling conditions...");
    let enabling_conditions = cases
        .iter()
        .map(|case| {
            format!(
                "CASE: {}\nENABLING CONDITION: {}",
                case.case_name, case.enabling_condition
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n");
    let prompt = BedrockClient::render_prompt(
        STAGE2_CLUSTER_PROMPT,
        &[
            ("enabling_conditions", &enabling_conditions),
            ("num_cases", &cases.len().to_string()),
        ],
    );
    let clusters = bedrock
        .invoke_json(&prompt, SYSTEM_CLUSTER, None, Some(4096))
        .await?;
    let qualities = response_array(clusters, "qualities");
    info!("Identified {} draft qualities.", qualities.len());
    Ok(qualities)
}

async fn refine_qualities(
    bedrock: &BedrockClient,
    drafts: &[serde_json::Value],
) -> StageResult<Vec<TaxonomyQuality>> {
    info!("Pass 2: Refining each quality...");
    let all_names = quality_names(drafts);
    let mut refined = Vec::new();

    for draft in drafts {
        refined.push(refine_quality(bedrock, draft, &all_names).await?);
    }

    Ok(refined)
}

async fn refine_quality(
    bedrock: &BedrockClient,
    draft: &serde_json::Value,
    all_names: &[String],
) -> StageResult<TaxonomyQuality> {
    let name = json_string(draft, "name");
    info!("Refining: {}", name);

    let other_names = all_names
        .iter()
        .filter(|n| n.as_str() != name)
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");
    let examples = draft
        .get("enabling_conditions")
        .map(|e| serde_json::to_string_pretty(e).unwrap_or_default())
        .unwrap_or_default();
    let prompt = BedrockClient::render_prompt(
        STAGE2_REFINE_PROMPT,
        &[
            ("quality_name", &name),
            ("quality_definition", draft_str(draft, "definition")),
            ("example_conditions", &examples),
            ("other_quality_names", &other_names),
        ],
    );
    let refined = bedrock
        .invoke_json(&prompt, SYSTEM_REFINE, None, Some(2048))
        .await?;
    Ok(build_quality(&name, &refined))
}

async fn deduplicate_qualities(
    db_client: &Client,
    bedrock: &BedrockClient,
    refined_qualities: &[TaxonomyQuality],
) -> StageResult<DedupOutcome> {
    info!("Pass 3: Semantic deduplication...");
    let mut existing = db::get_taxonomy(db_client).await?;
    let mut outcome = DedupOutcome {
        novel: Vec::new(),
        merged_count: 0,
    };

    for draft in refined_qualities {
        match insert_or_merge_quality(db_client, bedrock, draft, &existing).await? {
            DedupAction::Merged => outcome.merged_count += 1,
            DedupAction::Novel(quality) => {
                let quality = *quality;
                outcome.novel.push(quality.clone());
                existing.push(quality);
            }
        }
    }

    Ok(outcome)
}

async fn insert_or_merge_quality(
    db_client: &Client,
    bedrock: &BedrockClient,
    draft: &TaxonomyQuality,
    existing: &[TaxonomyQuality],
) -> StageResult<DedupAction> {
    if let Some(existing_id) = matched_existing_quality(bedrock, draft, existing).await? {
        info!("MERGED: '{}' -> existing '{}'", draft.name, existing_id);
        merge_canonical_examples(db_client, &existing_id, draft).await?;
        return Ok(DedupAction::Merged);
    }

    info!("NOVEL: '{}' -- adding as draft", draft.name);
    db::insert_quality(db_client, draft).await?;
    Ok(DedupAction::Novel(Box::new(draft.clone())))
}

async fn matched_existing_quality(
    bedrock: &BedrockClient,
    draft: &TaxonomyQuality,
    existing: &[TaxonomyQuality],
) -> StageResult<Option<String>> {
    if existing.is_empty() {
        return Ok(None);
    }
    let existing_text = existing_taxonomy_text(existing);
    let prompt = BedrockClient::render_prompt(
        STAGE2_DEDUP_PROMPT,
        &[
            ("new_name", &draft.name),
            ("new_definition", &draft.definition),
            ("new_exploitation_logic", &draft.exploitation_logic),
            ("existing_taxonomy", &existing_text),
        ],
    );
    let result = bedrock
        .invoke_json(&prompt, SYSTEM_DEDUP, None, Some(1024))
        .await?;
    Ok(valid_match_id(&result, existing))
}

fn valid_match_id(result: &serde_json::Value, existing: &[TaxonomyQuality]) -> Option<String> {
    let is_match = result
        .get("match")
        .and_then(|m| m.as_bool())
        .unwrap_or(false);
    let matched_id = result
        .get("existing_quality_id")
        .and_then(|id| id.as_str())?;
    (is_match && existing.iter().any(|q| q.quality_id == matched_id))
        .then(|| matched_id.to_string())
}

async fn merge_canonical_examples(
    db_client: &Client,
    existing_id: &str,
    draft: &TaxonomyQuality,
) -> StageResult<()> {
    let Some(examples) = &draft.canonical_examples else {
        return Ok(());
    };
    let Some(arr) = examples.as_array() else {
        return Ok(());
    };
    let strs: Vec<String> = arr
        .iter()
        .filter_map(|v| v.as_str().map(String::from))
        .collect();
    db::merge_quality_examples(db_client, existing_id, &strs).await
}

async fn complete_or_request_review(
    db_client: &Client,
    run_id: &str,
    taxonomy: &[TaxonomyQuality],
    cases_processed: usize,
    dedup: &DedupOutcome,
) -> StageResult<()> {
    if !dedup.novel.is_empty() {
        db::log_stage_pending_review(db_client, run_id, 2).await?;
        info!(
            "HUMAN REVIEW REQUIRED -- {} new draft qualities need approval.",
            dedup.novel.len()
        );
        return Ok(());
    }

    db::log_stage_complete(
        db_client,
        run_id,
        2,
        Some(&json!({
            "qualities_total": taxonomy.len(),
            "cases_processed": cases_processed,
            "merged": dedup.merged_count,
            "novel": 0,
        })),
    )
    .await
}

fn response_array(response: serde_json::Value, key: &str) -> Vec<serde_json::Value> {
    if response.is_array() {
        return response.as_array().cloned().unwrap_or_default();
    }
    response
        .get(key)
        .and_then(|q| q.as_array())
        .cloned()
        .unwrap_or_default()
}

fn quality_names(drafts: &[serde_json::Value]) -> Vec<String> {
    drafts
        .iter()
        .filter_map(|q| q.get("name").and_then(|n| n.as_str()).map(String::from))
        .collect()
}

fn build_quality(fallback_name: &str, refined: &serde_json::Value) -> TaxonomyQuality {
    let final_name = refined
        .get("name")
        .and_then(|n| n.as_str())
        .unwrap_or(fallback_name)
        .to_string();
    let mut hasher = Sha256::new();
    hasher.update(final_name.as_bytes());

    TaxonomyQuality {
        quality_id: format!("{:x}", hasher.finalize())[..8].to_string(),
        name: final_name,
        definition: json_string(refined, "definition"),
        recognition_test: json_string(refined, "recognition_test"),
        exploitation_logic: json_string(refined, "exploitation_logic"),
        canonical_examples: refined.get("canonical_examples").cloned(),
        review_status: Some("draft".to_string()),
        reviewer_notes: None,
        created_at: String::new(),
        color: None,
        case_count: None,
    }
}

fn existing_taxonomy_text(existing: &[TaxonomyQuality]) -> String {
    existing
        .iter()
        .map(|q| {
            format!(
                "ID: {}\nName: {}\nDefinition: {}\nExploitation Logic: {}",
                q.quality_id, q.name, q.definition, q.exploitation_logic
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn draft_str<'a>(value: &'a serde_json::Value, key: &str) -> &'a str {
    value.get(key).and_then(|v| v.as_str()).unwrap_or("")
}

fn json_string(value: &serde_json::Value, key: &str) -> String {
    draft_str(value, key).to_string()
}
