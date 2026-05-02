//! Stage 0: Enforcement Source Fetching
//!
//! Fetches enforcement action documents from URLs, extracts visible text,
//! and ingests into the RAG store. Then validates pending documents via LLM.

use regex::Regex;
use serde_json::json;
use tokio_postgres::Client;
use tracing::{error, info};

use crate::bedrock::BedrockClient;
use crate::db;
use crate::rag::DocumentIngester;
use crate::types::{Config, Document, EnforcementSource};

const VALIDATION_SYSTEM: &str =
    "You are an analyst evaluating enforcement documents for a healthcare fraud \
     analysis pipeline. You determine whether a document describes a real enforcement \
     action and provide a concise summary.";

type StageResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[derive(Default)]
struct FetchStats {
    fetched: usize,
    skipped: usize,
    failed: usize,
}

enum FetchOutcome {
    Fetched,
    Skipped,
    Failed,
}

/// Extract visible text from HTML content by stripping tags.
pub fn extract_text(html: &str) -> String {
    // Remove script, style, noscript, svg, head elements (case-insensitive)
    let script_re = Regex::new(r"(?is)<script[^>]*>.*?</script>").unwrap();
    let style_re = Regex::new(r"(?is)<style[^>]*>.*?</style>").unwrap();
    let noscript_re = Regex::new(r"(?is)<noscript[^>]*>.*?</noscript>").unwrap();
    let svg_re = Regex::new(r"(?is)<svg[^>]*>.*?</svg>").unwrap();
    let head_re = Regex::new(r"(?is)<head[^>]*>.*?</head>").unwrap();

    let cleaned = script_re.replace_all(html, " ");
    let cleaned = style_re.replace_all(&cleaned, " ");
    let cleaned = noscript_re.replace_all(&cleaned, " ");
    let cleaned = svg_re.replace_all(&cleaned, " ");
    let cleaned = head_re.replace_all(&cleaned, " ");

    let strip_re = Regex::new(r"<[^>]+>").unwrap();
    let text = strip_re.replace_all(&cleaned, " ");
    let lines: Vec<&str> = text
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect();
    let result = lines.join("\n");
    let multi_newline = Regex::new(r"\n{3,}").unwrap();
    multi_newline
        .replace_all(&result, "\n\n")
        .trim()
        .to_string()
        .replace('\0', "")
}

/// Detect binary/non-readable content.
pub fn is_binary_content(text: &str) -> bool {
    if text.is_empty() {
        return true;
    }
    let sample = &text[..text.len().min(2000)];
    if sample.trim_start().starts_with("%PDF") {
        return true;
    }
    let non_printable = sample
        .chars()
        .filter(|c| !c.is_alphanumeric() && !c.is_whitespace() && !c.is_ascii_punctuation())
        .count();
    non_printable as f64 / sample.len().max(1) as f64 > 0.15
}

/// Fetch a URL and return the response body as text.
pub async fn fetch_url(url: &str) -> StageResult<String> {
    let client = reqwest::Client::builder()
        .user_agent("SVAP-Pipeline/1.0 (Structural Vulnerability Analysis)")
        .timeout(std::time::Duration::from_secs(30))
        .build()?;
    let resp = client.get(url).send().await?;
    Ok(resp.text().await?)
}

async fn validate_document(
    bedrock: &BedrockClient,
    source_name: &str,
    description: &str,
    url: &str,
    doc_text: &str,
) -> StageResult<(String, bool)> {
    let text_sample = &doc_text[..doc_text.len().min(4000)];
    let prompt = format!(
        "Analyze this enforcement document and provide:\n\
         1. A 2-3 sentence summary of the enforcement action described\n\
         2. Whether this is a valid healthcare fraud enforcement document (true/false)\n\n\
         Document source: {source_name}\n\
         Description: {description}\n\
         URL: {url}\n\n\
         Document text (first portion):\n{text_sample}\n\n\
         Respond in JSON: {{\"summary\": \"...\", \"is_valid\": true/false}}"
    );
    let result = bedrock
        .invoke_json(&prompt, VALIDATION_SYSTEM, Some(0.1), Some(500))
        .await?;
    let summary = result
        .get("summary")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let is_valid = result
        .get("is_valid")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    Ok((summary, is_valid))
}

pub async fn run(
    db_client: &Client,
    bedrock: &BedrockClient,
    run_id: &str,
    config: &Config,
) -> StageResult<serde_json::Value> {
    info!("Stage 0: Enforcement Source Fetching");
    db::log_stage_start(db_client, run_id, 0).await?;

    match run_inner(db_client, bedrock, run_id, config).await {
        Ok(result) => {
            db::log_stage_complete(db_client, run_id, 0, Some(&result)).await?;
            Ok(result)
        }
        Err(e) => {
            db::log_stage_failed(db_client, run_id, 0, &e.to_string()).await?;
            Err(e)
        }
    }
}

async fn run_inner(
    db_client: &Client,
    bedrock: &BedrockClient,
    _run_id: &str,
    config: &Config,
) -> StageResult<serde_json::Value> {
    let sources = db::get_enforcement_sources(db_client).await?;
    let ingester = DocumentIngester::new(config);
    let fetch_stats = fetch_missing_sources(db_client, &ingester, &sources).await?;
    let validated = validate_pending_documents(db_client, bedrock).await?;

    let result = json!({
        "documents_fetched": fetch_stats.fetched,
        "documents_skipped": fetch_stats.skipped,
        "documents_failed": fetch_stats.failed,
        "documents_validated": validated,
    });
    info!(
        "Stage 0 complete: {} fetched, {} skipped, {} failed, {} validated.",
        fetch_stats.fetched, fetch_stats.skipped, fetch_stats.failed, validated
    );
    Ok(result)
}

async fn fetch_missing_sources(
    db_client: &Client,
    ingester: &DocumentIngester,
    sources: &[EnforcementSource],
) -> StageResult<FetchStats> {
    let mut stats = FetchStats::default();
    for source in sources {
        match fetch_source_document(db_client, ingester, source).await? {
            FetchOutcome::Fetched => stats.fetched += 1,
            FetchOutcome::Skipped => stats.skipped += 1,
            FetchOutcome::Failed => stats.failed += 1,
        }
    }
    Ok(stats)
}

async fn fetch_source_document(
    db_client: &Client,
    ingester: &DocumentIngester,
    source: &EnforcementSource,
) -> StageResult<FetchOutcome> {
    let Some(url) = source.url.as_deref().filter(|u| !u.is_empty()) else {
        return Ok(FetchOutcome::Skipped);
    };
    if source.has_document {
        return Ok(FetchOutcome::Skipped);
    }

    info!("Fetching: {}", source.name);
    let html = match fetch_url(url).await {
        Ok(html) => html,
        Err(e) => {
            error!("Failed to fetch {}: {}", url, e);
            return Ok(FetchOutcome::Failed);
        }
    };
    let text = extract_text(&html);
    if !is_usable_source_text(&text) {
        return Ok(FetchOutcome::Failed);
    }

    let metadata = json!({
        "url": url,
        "source_name": source.name,
        "source_id": source.source_id,
    });

    let (doc_id, n_chunks) = ingester
        .ingest_text(
            db_client,
            &text,
            &source.source_id,
            "enforcement",
            Some(&metadata),
        )
        .await?;

    let s3_key = format!(
        "enforcement-sources/{}/press_release.html",
        source.source_id
    );
    db::update_enforcement_source_document(db_client, &source.source_id, &s3_key, &doc_id).await?;
    info!("Ingested: {} chars, {} chunks", text.len(), n_chunks);
    Ok(FetchOutcome::Fetched)
}

fn is_usable_source_text(text: &str) -> bool {
    if text.len() < 200 {
        info!("Skipped: text too short ({} chars)", text.len());
        return false;
    }
    if is_binary_content(text) {
        info!("Skipped: binary/non-readable content");
        return false;
    }
    true
}

async fn validate_pending_documents(
    db_client: &Client,
    bedrock: &BedrockClient,
) -> StageResult<usize> {
    let sources = db::get_enforcement_sources(db_client).await?;
    let docs = db::get_all_documents(db_client, Some("enforcement")).await?;
    let mut validated = 0;

    for source in sources.iter().filter(|source| should_validate(source)) {
        if validate_source_document(db_client, bedrock, source, &docs).await? {
            validated += 1;
        }
    }

    Ok(validated)
}

fn should_validate(source: &EnforcementSource) -> bool {
    source.has_document && source.validation_status.as_deref() == Some("pending")
}

async fn validate_source_document(
    db_client: &Client,
    bedrock: &BedrockClient,
    source: &EnforcementSource,
    docs: &[Document],
) -> StageResult<bool> {
    let Some(doc) = docs
        .iter()
        .find(|d| Some(&d.doc_id) == source.doc_id.as_ref())
    else {
        return Ok(false);
    };

    info!("Validating: {}", source.name);
    match validate_document(
        bedrock,
        &source.name,
        source.description.as_deref().unwrap_or("N/A"),
        source.url.as_deref().unwrap_or("uploaded"),
        &doc.full_text,
    )
    .await
    {
        Ok((summary, is_valid)) => {
            let status = if is_valid { "valid" } else { "invalid" };
            db::update_enforcement_source_summary(db_client, &source.source_id, &summary, status)
                .await?;
            Ok(true)
        }
        Err(e) => {
            error!("Validation failed: {}", e);
            db::update_enforcement_source_summary(
                db_client,
                &source.source_id,
                &format!("Validation error: {e}"),
                "error",
            )
            .await?;
            Ok(false)
        }
    }
}
