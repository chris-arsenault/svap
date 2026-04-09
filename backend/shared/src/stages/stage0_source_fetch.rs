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
use crate::types::Config;

const VALIDATION_SYSTEM: &str =
    "You are an analyst evaluating enforcement documents for a healthcare fraud \
     analysis pipeline. You determine whether a document describes a real enforcement \
     action and provide a concise summary.";

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
pub async fn fetch_url(url: &str) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
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
) -> Result<(String, bool), Box<dyn std::error::Error + Send + Sync>> {
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
) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
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
) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
    let sources = db::get_enforcement_sources(db_client).await?;
    let ingester = DocumentIngester::new(config);

    let mut fetched = 0;
    let mut skipped = 0;
    let mut failed = 0;

    for source in &sources {
        if source.has_document {
            skipped += 1;
            continue;
        }
        let url = match &source.url {
            Some(u) if !u.is_empty() => u.clone(),
            _ => {
                skipped += 1;
                continue;
            }
        };

        info!("Fetching: {}", source.name);
        match fetch_url(&url).await {
            Ok(html) => {
                let text = extract_text(&html);
                if text.len() < 200 {
                    info!("Skipped: text too short ({} chars)", text.len());
                    failed += 1;
                    continue;
                }
                if is_binary_content(&text) {
                    info!("Skipped: binary/non-readable content");
                    failed += 1;
                    continue;
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
                db::update_enforcement_source_document(
                    db_client,
                    &source.source_id,
                    &s3_key,
                    &doc_id,
                )
                .await?;
                info!("Ingested: {} chars, {} chunks", text.len(), n_chunks);
                fetched += 1;
            }
            Err(e) => {
                error!("Failed to fetch {}: {}", url, e);
                failed += 1;
            }
        }
    }

    // Phase 2: Validate pending documents
    let mut validated = 0;
    let sources = db::get_enforcement_sources(db_client).await?;
    let docs = db::get_all_documents(db_client, Some("enforcement")).await?;

    for source in &sources {
        if !source.has_document || source.validation_status.as_deref() != Some("pending") {
            continue;
        }
        info!("Validating: {}", source.name);
        if let Some(doc) = docs
            .iter()
            .find(|d| Some(&d.doc_id) == source.doc_id.as_ref())
        {
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
                    db::update_enforcement_source_summary(
                        db_client,
                        &source.source_id,
                        &summary,
                        status,
                    )
                    .await?;
                    validated += 1;
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
                }
            }
        }
    }

    let result = json!({
        "documents_fetched": fetched,
        "documents_skipped": skipped,
        "documents_failed": failed,
        "documents_validated": validated,
    });
    info!(
        "Stage 0 complete: {} fetched, {} skipped, {} failed, {} validated.",
        fetched, skipped, failed, validated
    );
    Ok(result)
}
