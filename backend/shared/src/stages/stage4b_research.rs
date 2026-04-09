//! Stage 4B: Deep Structural Research
//!
//! Per-policy investigation using regulatory sources (eCFR, Federal Register).

use serde_json::json;
use sha2::{Digest, Sha256};
use tokio_postgres::Client;
use tracing::{error, info, warn};

use crate::bedrock::BedrockClient;
use crate::db;
use crate::types::{Config, RegulatorySource, StructuralFinding};

const RESEARCH_PLAN_SYSTEM: &str = "You are a regulatory research planner. You identify which specific sections of the Code of Federal Regulations should be consulted.";
const FINDING_EXTRACTION_SYSTEM: &str = "You are extracting factual structural observations from regulatory text. Each finding must be a single atomic observation about how a policy works mechanically.";

const STAGE4B_PLAN_PROMPT: &str = include_str!("../../prompts/stage4b_plan_research.txt");
const STAGE4B_EXTRACT_PROMPT: &str = include_str!("../../prompts/stage4b_extract_findings.txt");

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
    info!("Stage 4B: Deep Structural Research");
    db::log_stage_start(db_client, run_id, 41).await?;

    match run_inner(db_client, bedrock, run_id, config).await {
        Ok(result) => {
            db::log_stage_complete(db_client, run_id, 41, Some(&result)).await?;
            Ok(result)
        }
        Err(e) => {
            db::log_stage_failed(db_client, run_id, 41, &e.to_string()).await?;
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
    let dimensions = db::get_dimensions(db_client).await?;
    let top_n = config.research.as_ref().and_then(|r| r.top_n).unwrap_or(10);

    let triage = db::get_triage_results(db_client).await?;
    let policies_to_research: Vec<_> = triage.iter().take(top_n).collect();

    if policies_to_research.is_empty() {
        info!("No policies to research. Run triage first.");
        return Ok(json!({"policies_researched": 0}));
    }

    // Delta detection
    let dim_fp = {
        let mut ids: Vec<&str> = dimensions.iter().map(|d| d.dimension_id.as_str()).collect();
        ids.sort();
        compute_hash(&[&ids.join(":")])
    };
    let stored_hashes = db::get_processing_hashes(db_client, 41).await?;
    let all_policies = db::get_policies(db_client).await?;

    let mut delta_entries = Vec::new();
    let mut skipped = 0;
    for entry in &policies_to_research {
        let policy = all_policies.iter().find(|p| p.policy_id == entry.policy_id);
        let h = compute_hash(&[
            policy
                .map(|p| p.description.as_deref().unwrap_or(""))
                .unwrap_or(""),
            &entry.triage_score.to_string(),
            &dim_fp,
        ]);
        if stored_hashes.get(&entry.policy_id).map(|s| s.as_str()) == Some(&h) {
            skipped += 1;
        } else {
            delta_entries.push((entry, h));
        }
    }

    if delta_entries.is_empty() {
        info!("All policies unchanged. Skipping research.");
        return Ok(json!({"policies_researched": 0, "skipped": skipped}));
    }

    info!(
        "Researching {} policies ({} unchanged)...",
        delta_entries.len(),
        skipped
    );
    let mut researched = 0;

    let dimensions_text: String = dimensions
        .iter()
        .map(|d| {
            format!(
                "- {}: {} -- {}",
                d.dimension_id,
                d.name,
                &d.definition[..d.definition.len().min(100)]
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let strip_re = regex::Regex::new(r"<[^>]+>").unwrap();

    for (entry, h) in &delta_entries {
        let policy = match all_policies.iter().find(|p| p.policy_id == entry.policy_id) {
            Some(p) => p,
            None => {
                warn!("Policy {} not found, skipping", entry.policy_id);
                continue;
            }
        };

        info!("Researching: {}", policy.name);
        let session_id = {
            let mut hasher = Sha256::new();
            hasher.update(format!("{}:{}", run_id, entry.policy_id));
            format!("{:x}", hasher.finalize())[..12].to_string()
        };
        db::create_research_session(db_client, run_id, &entry.policy_id, &session_id).await?;
        db::update_research_session(db_client, &session_id, "researching", None, None).await?;
        db::update_policy_lifecycle(db_client, &entry.policy_id, "research_in_progress").await?;

        // Plan research
        let prompt = BedrockClient::render_prompt(
            STAGE4B_PLAN_PROMPT,
            &[
                ("policy_name", &policy.name),
                (
                    "policy_description",
                    policy.description.as_deref().unwrap_or(""),
                ),
                ("dimensions", &dimensions_text),
                (
                    "known_cfr_references",
                    "None pre-mapped. Use your knowledge of federal healthcare regulations.",
                ),
            ],
        );
        let plan = bedrock
            .invoke_json(&prompt, RESEARCH_PLAN_SYSTEM, None, Some(2000))
            .await?;

        // Process eCFR queries
        if let Some(ecfr_queries) = plan.get("ecfr_queries").and_then(|e| e.as_array()) {
            for ecfr_ref in ecfr_queries {
                let title = ecfr_ref.get("title").and_then(|t| t.as_i64()).unwrap_or(42);
                let part = match ecfr_ref.get("part").and_then(|p| p.as_str()) {
                    Some(p) => p.to_string(),
                    None => continue,
                };

                let source_id = format!("ecfr_t{}_p{}", title, part);

                // Check cache
                let cached = db::get_regulatory_source(db_client, &source_id).await?;
                let reg_text = if let Some(cached) = cached {
                    cached.full_text
                } else {
                    // Fetch from eCFR API
                    let url = format!(
                        "https://www.ecfr.gov/api/versioner/v1/full/{}/title-{}.xml?part={}",
                        chrono::Utc::now().format("%Y-%m-%d"),
                        title,
                        part
                    );
                    match reqwest::get(&url).await {
                        Ok(resp) => {
                            let text = resp.text().await.unwrap_or_default();
                            let source = RegulatorySource {
                                source_id: source_id.clone(),
                                source_type: "ecfr".to_string(),
                                url: format!(
                                    "https://www.ecfr.gov/current/title-{}/part-{}",
                                    title, part
                                ),
                                title: Some(format!("Title {} Part {}", title, part)),
                                cfr_reference: Some(format!("{} CFR Part {}", title, part)),
                                full_text: text.clone(),
                                fetched_at: String::new(),
                                metadata: None,
                            };
                            db::insert_regulatory_source(db_client, &source).await?;
                            text
                        }
                        Err(e) => {
                            error!("Failed to fetch eCFR: {}", e);
                            continue;
                        }
                    }
                };

                // Extract text chunks from XML (simplified: strip tags)
                let clean_text = strip_re.replace_all(&reg_text, " ").to_string();
                if clean_text.len() < 100 {
                    continue;
                }

                let source_citation = format!("{} CFR Part {}", title, part);
                let prompt = BedrockClient::render_prompt(
                    STAGE4B_EXTRACT_PROMPT,
                    &[
                        ("policy_name", &policy.name),
                        ("source_citation", &source_citation),
                        ("dimensions_text", &dimensions_text),
                        ("source_text", &clean_text[..clean_text.len().min(4000)]),
                    ],
                );
                let result = bedrock
                    .invoke_json(&prompt, FINDING_EXTRACTION_SYSTEM, Some(0.1), Some(2000))
                    .await?;

                if let Some(findings) = result.get("findings").and_then(|f| f.as_array()) {
                    for f in findings {
                        let finding_id = {
                            let mut hasher = Sha256::new();
                            hasher.update(format!(
                                "{}:{}:{}",
                                entry.policy_id,
                                f.get("dimension_id").and_then(|d| d.as_str()).unwrap_or(""),
                                f.get("source_citation")
                                    .and_then(|s| s.as_str())
                                    .unwrap_or("")
                            ));
                            format!("{:x}", hasher.finalize())[..12].to_string()
                        };
                        let finding = StructuralFinding {
                            finding_id,
                            run_id: run_id.to_string(),
                            policy_id: entry.policy_id.clone(),
                            dimension_id: f
                                .get("dimension_id")
                                .and_then(|d| d.as_str())
                                .map(String::from),
                            observation: f
                                .get("observation")
                                .and_then(|o| o.as_str())
                                .unwrap_or("")
                                .to_string(),
                            source_type: "ecfr".to_string(),
                            source_citation: f
                                .get("source_citation")
                                .and_then(|s| s.as_str())
                                .map(String::from),
                            source_text: f
                                .get("source_text_excerpt")
                                .and_then(|s| s.as_str())
                                .map(String::from),
                            confidence: f
                                .get("confidence")
                                .and_then(|c| c.as_str())
                                .unwrap_or("medium")
                                .to_string(),
                            status: "active".to_string(),
                            stale_reason: None,
                            created_at: String::new(),
                            created_by: Some("stage4b_research".to_string()),
                            dimension_name: None,
                        };
                        db::insert_structural_finding(db_client, run_id, &finding).await?;
                    }
                }

                tokio::time::sleep(std::time::Duration::from_millis(300)).await;
            }
        }

        db::update_research_session(db_client, &session_id, "findings_complete", None, None)
            .await?;
        db::update_policy_lifecycle(db_client, &entry.policy_id, "structurally_characterized")
            .await?;
        db::record_processing(db_client, 41, &entry.policy_id, h, run_id).await?;
        researched += 1;
    }

    info!("Research complete: {} policies.", researched);
    Ok(json!({"policies_researched": researched, "skipped": skipped}))
}
