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
use crate::types::{Config, SourceCandidate, SourceFeed};

const RICHNESS_SYSTEM: &str =
    "You are an analyst evaluating enforcement documents for a healthcare fraud \
     analysis pipeline. You assess whether a document contains sufficient structural \
     detail about fraud schemes to be useful for case extraction.";

/// Prompt templates are loaded from the prompts directory.
const DISCOVERY_EXTRACT_LINKS_PROMPT: &str =
    include_str!("../../prompts/discovery_extract_links.txt");
const DISCOVERY_RICHNESS_PROMPT: &str = include_str!("../../prompts/discovery_richness.txt");

type StageResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[derive(Default)]
struct DiscoveryStats {
    discovered: usize,
    accepted: usize,
    rejected: usize,
    review: usize,
}

impl DiscoveryStats {
    fn add(&mut self, other: DiscoveryStats) {
        self.discovered += other.discovered;
        self.accepted += other.accepted;
        self.rejected += other.rejected;
        self.review += other.review;
    }
}

struct DiscoveryThresholds {
    max_per_feed: usize,
    accept: f64,
    review: f64,
}

impl DiscoveryThresholds {
    fn from_config(config: &Config) -> Self {
        let discovery = config.discovery.as_ref();
        Self {
            max_per_feed: discovery
                .and_then(|d| d.max_candidates_per_feed)
                .unwrap_or(50),
            accept: discovery
                .and_then(|d| d.richness_accept_threshold)
                .unwrap_or(0.7),
            review: discovery
                .and_then(|d| d.richness_review_threshold)
                .unwrap_or(0.4),
        }
    }
}

enum CandidateDisposition {
    Accepted,
    Review,
    Rejected,
}

pub async fn run(
    db_client: &Client,
    bedrock: &BedrockClient,
    _run_id: &str,
    config: &Config,
) -> StageResult<serde_json::Value> {
    info!("Case Discovery -- Feed Monitoring");

    let feeds = db::get_source_feeds(db_client, true).await?;
    if feeds.is_empty() {
        info!("No source feeds configured.");
        return Ok(json!({"feeds_checked": 0}));
    }

    let thresholds = DiscoveryThresholds::from_config(config);
    let ingester = DocumentIngester::new(config);
    let link_re = Regex::new(r#"<a\s+[^>]*href\s*=\s*"([^"]*)"[^>]*>([\s\S]*?)</a>"#).unwrap();
    let mut totals = DiscoveryStats::default();

    for feed in &feeds {
        let stats =
            process_feed(db_client, bedrock, &ingester, &thresholds, &link_re, feed).await?;
        totals.add(stats);
    }

    let summary = json!({
        "feeds_checked": feeds.len(),
        "candidates_discovered": totals.discovered,
        "candidates_accepted": totals.accepted,
        "candidates_rejected": totals.rejected,
        "candidates_pending_review": totals.review,
    });
    info!(
        "Discovery complete: {} discovered, {} accepted, {} rejected, {} pending review.",
        totals.discovered, totals.accepted, totals.rejected, totals.review
    );
    Ok(summary)
}

async fn process_feed(
    db_client: &Client,
    bedrock: &BedrockClient,
    ingester: &DocumentIngester,
    thresholds: &DiscoveryThresholds,
    link_re: &Regex,
    feed: &SourceFeed,
) -> StageResult<DiscoveryStats> {
    info!("Checking feed: {}", feed.name);
    let Some((html, page_text)) = fetch_feed_page(feed).await else {
        return Ok(DiscoveryStats::default());
    };

    let raw_links = extract_raw_links(link_re, &html);
    let urls = discover_candidate_urls(bedrock, feed, &raw_links, &page_text).await?;
    let candidates = insert_new_candidates(db_client, feed, &urls, thresholds.max_per_feed).await?;
    info!(
        "Found {} links, {} new candidates",
        urls.len(),
        candidates.len()
    );

    let mut stats =
        evaluate_candidates(db_client, bedrock, ingester, thresholds, &candidates).await?;
    stats.discovered = candidates.len();
    db::update_feed_last_checked(db_client, &feed.feed_id).await?;
    Ok(stats)
}

async fn fetch_feed_page(feed: &SourceFeed) -> Option<(String, String)> {
    match fetch_url(&feed.listing_url).await {
        Ok(html) => {
            let page_text = extract_text(&html);
            Some((html, page_text))
        }
        Err(e) => {
            error!("Failed to fetch listing page: {}", e);
            None
        }
    }
}

fn extract_raw_links(link_re: &Regex, html: &str) -> Vec<(String, String)> {
    link_re
        .captures_iter(html)
        .map(|cap| (cap[1].to_string(), cap[2].to_string()))
        .collect()
}

async fn discover_candidate_urls(
    bedrock: &BedrockClient,
    feed: &SourceFeed,
    raw_links: &[(String, String)],
    page_text: &str,
) -> StageResult<Vec<String>> {
    let urls = selector_urls(feed, raw_links);
    if urls.is_empty() {
        llm_selected_urls(bedrock, feed, raw_links, page_text).await
    } else {
        Ok(urls)
    }
}

fn selector_urls(feed: &SourceFeed, raw_links: &[(String, String)]) -> Vec<String> {
    let Some(selector) = &feed.link_selector else {
        return Vec::new();
    };
    let Ok(sel_re) = Regex::new(selector) else {
        return Vec::new();
    };
    raw_links
        .iter()
        .map(|(url, _)| resolve_url(&feed.listing_url, url))
        .filter(|url| sel_re.is_match(url))
        .collect()
}

async fn llm_selected_urls(
    bedrock: &BedrockClient,
    feed: &SourceFeed,
    raw_links: &[(String, String)],
    page_text: &str,
) -> StageResult<Vec<String>> {
    let link_list = format_link_list(feed, raw_links);
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
    Ok(extract_llm_urls(&result))
}

fn format_link_list(feed: &SourceFeed, raw_links: &[(String, String)]) -> String {
    raw_links
        .iter()
        .take(100)
        .map(|(url, text)| {
            let resolved = resolve_url(&feed.listing_url, url);
            let clean_text: String = extract_text(text).chars().take(80).collect();
            format!("- [{}]({})", clean_text, resolved)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn extract_llm_urls(result: &serde_json::Value) -> Vec<String> {
    let links = result
        .as_array()
        .or_else(|| result.get("links").and_then(|links| links.as_array()));
    links
        .into_iter()
        .flatten()
        .filter_map(|link| link.get("url").and_then(|url| url.as_str()))
        .map(String::from)
        .collect()
}

async fn insert_new_candidates(
    db_client: &Client,
    feed: &SourceFeed,
    urls: &[String],
    max_per_feed: usize,
) -> StageResult<Vec<SourceCandidate>> {
    let mut candidates = Vec::new();
    for url in urls.iter().take(max_per_feed).filter(|url| !url.is_empty()) {
        if db::get_candidate_by_url(db_client, url).await?.is_some() {
            continue;
        }
        let candidate = build_candidate(feed, url);
        db::insert_candidate(db_client, &candidate).await?;
        candidates.push(candidate);
    }
    Ok(candidates)
}

fn build_candidate(feed: &SourceFeed, url: &str) -> SourceCandidate {
    let mut hasher = Sha256::new();
    hasher.update(url.as_bytes());
    let candidate_id = format!("{:x}", hasher.finalize())[..12].to_string();
    let now = Utc::now().to_rfc3339();

    SourceCandidate {
        candidate_id,
        feed_id: Some(feed.feed_id.clone()),
        title: "Untitled".to_string(),
        url: url.to_string(),
        discovered_at: now.clone(),
        published_date: None,
        status: "discovered".to_string(),
        richness_score: None,
        richness_rationale: None,
        estimated_cases: None,
        source_id: None,
        doc_id: None,
        reviewed_by: Some("auto".to_string()),
        created_at: now.clone(),
        updated_at: now,
    }
}

async fn evaluate_candidates(
    db_client: &Client,
    bedrock: &BedrockClient,
    ingester: &DocumentIngester,
    thresholds: &DiscoveryThresholds,
    candidates: &[SourceCandidate],
) -> StageResult<DiscoveryStats> {
    let mut stats = DiscoveryStats::default();
    for candidate in candidates {
        let Some(text) = fetch_candidate_text(db_client, candidate).await? else {
            continue;
        };
        let richness = evaluate_richness(db_client, bedrock, candidate, &text).await?;
        match apply_disposition(db_client, ingester, candidate, &text, richness, thresholds).await?
        {
            CandidateDisposition::Accepted => stats.accepted += 1,
            CandidateDisposition::Review => stats.review += 1,
            CandidateDisposition::Rejected => stats.rejected += 1,
        }
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
    Ok(stats)
}

async fn fetch_candidate_text(
    db_client: &Client,
    candidate: &SourceCandidate,
) -> StageResult<Option<String>> {
    let text = match fetch_url(&candidate.url).await {
        Ok(html) => extract_text(&html),
        Err(e) => {
            error!("Failed to fetch {}: {}", candidate.url, e);
            db::update_candidate_status(db_client, &candidate.candidate_id, "error").await?;
            return Ok(None);
        }
    };

    if text.len() < 200 || is_binary_content(&text) {
        db::update_candidate_status(db_client, &candidate.candidate_id, "error").await?;
        return Ok(None);
    }

    db::update_candidate_status(db_client, &candidate.candidate_id, "fetched").await?;
    Ok(Some(text))
}

async fn evaluate_richness(
    db_client: &Client,
    bedrock: &BedrockClient,
    candidate: &SourceCandidate,
    text: &str,
) -> StageResult<f64> {
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
    Ok(richness_score)
}

async fn apply_disposition(
    db_client: &Client,
    ingester: &DocumentIngester,
    candidate: &SourceCandidate,
    text: &str,
    richness_score: f64,
    thresholds: &DiscoveryThresholds,
) -> StageResult<CandidateDisposition> {
    if richness_score >= thresholds.accept {
        ingest_candidate(db_client, ingester, candidate, text).await?;
        return Ok(CandidateDisposition::Accepted);
    }
    if richness_score >= thresholds.review {
        return Ok(CandidateDisposition::Review);
    }
    db::update_candidate_status(db_client, &candidate.candidate_id, "rejected").await?;
    Ok(CandidateDisposition::Rejected)
}

async fn ingest_candidate(
    db_client: &Client,
    ingester: &DocumentIngester,
    candidate: &SourceCandidate,
    text: &str,
) -> StageResult<()> {
    let metadata = json!({
        "url": candidate.url,
        "source_name": candidate.title,
        "candidate_id": candidate.candidate_id,
    });
    let source_id = format!("disc_{}", candidate.candidate_id);
    let (doc_id, _) = ingester
        .ingest_text(db_client, text, &source_id, "enforcement", Some(&metadata))
        .await?;
    db::update_candidate_ingested(db_client, &candidate.candidate_id, &source_id, &doc_id).await?;
    Ok(())
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
