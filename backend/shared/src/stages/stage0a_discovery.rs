//! Stage 0A: Case Discovery -- Feed Monitoring
//!
//! Checks configured enforcement source listing pages for new entries,
//! fetches each, evaluates richness, and queues for ingestion.

use chrono::Utc;
use regex::Regex;
use serde_json::json;
use sha2::{Digest, Sha256};
use tokio_postgres::Client;
use tracing::{error, info};

use crate::bedrock::BedrockClient;
use crate::db;
use crate::rag::DocumentIngester;
use crate::stages::stage0_source_fetch::{extract_text, fetch_url, is_binary_content};
use crate::types::{Config, SourceCandidate};

const RICHNESS_SYSTEM: &str =
    "You are an analyst evaluating enforcement documents for a healthcare fraud \
     analysis pipeline. You assess whether a document contains sufficient structural \
     detail about fraud schemes to be useful for case extraction.";

/// Prompt templates are loaded from the prompts directory.
const DISCOVERY_EXTRACT_LINKS_PROMPT: &str =
    include_str!("../../prompts/discovery_extract_links.txt");
const DISCOVERY_RICHNESS_PROMPT: &str = include_str!("../../prompts/discovery_richness.txt");

pub async fn run(
    db_client: &Client,
    bedrock: &BedrockClient,
    _run_id: &str,
    config: &Config,
) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
    info!("Case Discovery -- Feed Monitoring");

    let feeds = db::get_source_feeds(db_client, true).await?;
    if feeds.is_empty() {
        info!("No source feeds configured.");
        return Ok(json!({"feeds_checked": 0}));
    }

    let max_per_feed = config
        .discovery
        .as_ref()
        .and_then(|d| d.max_candidates_per_feed)
        .unwrap_or(50);
    let accept_threshold = config
        .discovery
        .as_ref()
        .and_then(|d| d.richness_accept_threshold)
        .unwrap_or(0.7);
    let review_threshold = config
        .discovery
        .as_ref()
        .and_then(|d| d.richness_review_threshold)
        .unwrap_or(0.4);

    let mut total_discovered = 0;
    let mut total_accepted = 0;
    let mut total_rejected = 0;
    let mut total_review = 0;

    let ingester = DocumentIngester::new(config);

    let link_re = Regex::new(r#"<a\s+[^>]*href\s*=\s*"([^"]*)"[^>]*>([\s\S]*?)</a>"#).unwrap();

    for feed in &feeds {
        info!("Checking feed: {}", feed.name);
        let html = match fetch_url(&feed.listing_url).await {
            Ok(h) => h,
            Err(e) => {
                error!("Failed to fetch listing page: {}", e);
                continue;
            }
        };

        let page_text = extract_text(&html);

        // Extract links via regex selector or LLM
        let raw_links: Vec<(String, String)> = link_re
            .captures_iter(&html)
            .map(|cap| (cap[1].to_string(), cap[2].to_string()))
            .collect();

        let mut filtered_urls: Vec<String> = Vec::new();
        if let Some(selector) = &feed.link_selector {
            if let Ok(sel_re) = Regex::new(selector) {
                for (url, _text) in &raw_links {
                    let resolved = resolve_url(&feed.listing_url, url);
                    if sel_re.is_match(&resolved) {
                        filtered_urls.push(resolved);
                    }
                }
            }
        }

        if filtered_urls.is_empty() {
            // Fall back to LLM-based link extraction
            let link_list: String = raw_links
                .iter()
                .take(100)
                .map(|(url, text)| {
                    let resolved = resolve_url(&feed.listing_url, url);
                    let clean_text: String = extract_text(text).chars().take(80).collect();
                    format!("- [{}]({})", clean_text, resolved)
                })
                .collect::<Vec<_>>()
                .join("\n");

            let prompt = BedrockClient::render_prompt(
                DISCOVERY_EXTRACT_LINKS_PROMPT,
                &[
                    ("feed_name", &feed.name),
                    ("content_type", &feed.content_type),
                    ("base_url", &feed.listing_url),
                    ("link_list", &link_list),
                    ("page_text", &page_text[..page_text.len().min(3000)]),
                ],
            );

            let result = bedrock
                .invoke_json(
                    &prompt,
                    "You are an analyst identifying enforcement case document links.",
                    Some(0.1),
                    Some(2000),
                )
                .await?;

            if let Some(links) = result.as_array() {
                for link in links {
                    if let Some(url) = link.get("url").and_then(|u| u.as_str()) {
                        filtered_urls.push(url.to_string());
                    }
                }
            } else if let Some(links) = result.get("links").and_then(|l| l.as_array()) {
                for link in links {
                    if let Some(url) = link.get("url").and_then(|u| u.as_str()) {
                        filtered_urls.push(url.to_string());
                    }
                }
            }
        }

        // Deduplicate and create candidates
        let mut new_candidates = Vec::new();
        for url in filtered_urls.iter().take(max_per_feed) {
            if url.is_empty() {
                continue;
            }
            if db::get_candidate_by_url(db_client, url).await?.is_some() {
                continue;
            }

            let mut hasher = Sha256::new();
            hasher.update(url.as_bytes());
            let candidate_id = format!("{:x}", hasher.finalize())[..12].to_string();

            let candidate = SourceCandidate {
                candidate_id: candidate_id.clone(),
                feed_id: Some(feed.feed_id.clone()),
                title: "Untitled".to_string(),
                url: url.clone(),
                discovered_at: Utc::now().to_rfc3339(),
                published_date: None,
                status: "discovered".to_string(),
                richness_score: None,
                richness_rationale: None,
                estimated_cases: None,
                source_id: None,
                doc_id: None,
                reviewed_by: Some("auto".to_string()),
                created_at: Utc::now().to_rfc3339(),
                updated_at: Utc::now().to_rfc3339(),
            };
            db::insert_candidate(db_client, &candidate).await?;
            new_candidates.push(candidate);
        }

        total_discovered += new_candidates.len();
        info!(
            "Found {} links, {} new candidates",
            filtered_urls.len(),
            new_candidates.len()
        );

        // Fetch, evaluate richness, and apply disposition for each candidate
        for candidate in &new_candidates {
            let text = match fetch_url(&candidate.url).await {
                Ok(html) => extract_text(&html),
                Err(e) => {
                    error!("Failed to fetch {}: {}", candidate.url, e);
                    db::update_candidate_status(db_client, &candidate.candidate_id, "error")
                        .await?;
                    continue;
                }
            };

            if text.len() < 200 || is_binary_content(&text) {
                db::update_candidate_status(db_client, &candidate.candidate_id, "error").await?;
                continue;
            }

            db::update_candidate_status(db_client, &candidate.candidate_id, "fetched").await?;

            // Evaluate richness
            let prompt = BedrockClient::render_prompt(
                DISCOVERY_RICHNESS_PROMPT,
                &[
                    ("title", &candidate.title),
                    ("url", &candidate.url),
                    ("text_sample", &text[..text.len().min(6000)]),
                ],
            );
            let eval = bedrock
                .invoke_json(&prompt, RICHNESS_SYSTEM, Some(0.1), Some(500))
                .await?;
            let richness_score = eval
                .get("richness_score")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);
            let rationale = eval
                .get("rationale")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let estimated_cases = eval
                .get("estimated_cases")
                .and_then(|v| v.as_i64())
                .unwrap_or(1) as i32;

            db::update_candidate_richness(
                db_client,
                &candidate.candidate_id,
                richness_score,
                &rationale,
                estimated_cases,
            )
            .await?;

            // Apply disposition
            if richness_score >= accept_threshold {
                // Auto-accept: ingest
                let metadata = json!({
                    "url": candidate.url,
                    "source_name": candidate.title,
                    "candidate_id": candidate.candidate_id,
                });
                let source_id = format!("disc_{}", candidate.candidate_id);
                let (doc_id, _) = ingester
                    .ingest_text(db_client, &text, &source_id, "enforcement", Some(&metadata))
                    .await?;
                db::update_candidate_ingested(
                    db_client,
                    &candidate.candidate_id,
                    &source_id,
                    &doc_id,
                )
                .await?;
                total_accepted += 1;
            } else if richness_score >= review_threshold {
                total_review += 1;
            } else {
                db::update_candidate_status(db_client, &candidate.candidate_id, "rejected").await?;
                total_rejected += 1;
            }

            // Be polite to government servers
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }

        db::update_feed_last_checked(db_client, &feed.feed_id).await?;
    }

    let summary = json!({
        "feeds_checked": feeds.len(),
        "candidates_discovered": total_discovered,
        "candidates_accepted": total_accepted,
        "candidates_rejected": total_rejected,
        "candidates_pending_review": total_review,
    });
    info!(
        "Discovery complete: {} discovered, {} accepted, {} rejected, {} pending review.",
        total_discovered, total_accepted, total_rejected, total_review
    );
    Ok(summary)
}

fn resolve_url(base: &str, href: &str) -> String {
    if href.starts_with("http://") || href.starts_with("https://") {
        return href.to_string();
    }
    // Simple URL resolution
    if href.starts_with('/') {
        if let Some(origin) = base
            .find("//")
            .and_then(|i| base[i + 2..].find('/').map(|j| &base[..i + 2 + j]))
        {
            return format!("{}{}", origin, href);
        }
    }
    format!(
        "{}/{}",
        base.trim_end_matches('/'),
        href.trim_start_matches('/')
    )
}
