//! Stage 1: Case Corpus Assembly
//!
//! Reads enforcement documents and extracts structured case data using LLM.

use regex::Regex;
use serde_json::json;
use sha2::{Digest, Sha256};
use tokio_postgres::Client;
use tracing::info;

use crate::bedrock::BedrockClient;
use crate::db;
use crate::types::{Case, Config, Document};

const SYSTEM_PROMPT: &str =
    "You are an analyst extracting structured information from enforcement \
     documents. You extract the mechanical details of how schemes operated, not just legal \
     conclusions. Be precise about the enabling policy structure -- identify the specific \
     design feature that was exploited, not generic labels like \"weak oversight\".";

const STAGE1_EXTRACT_PROMPT: &str = include_str!("../../prompts/stage1_extract.txt");
type StageResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

pub async fn run(
    db_client: &Client,
    bedrock: &BedrockClient,
    run_id: &str,
    config: &Config,
) -> StageResult<serde_json::Value> {
    info!("Stage 1: Case Corpus Assembly");
    db::log_stage_start(db_client, run_id, 1).await?;

    match run_inner(db_client, bedrock, run_id, config).await {
        Ok(result) => {
            db::log_stage_complete(db_client, run_id, 1, Some(&result)).await?;
            Ok(result)
        }
        Err(e) => {
            db::log_stage_failed(db_client, run_id, 1, &e.to_string()).await?;
            Err(e)
        }
    }
}

async fn run_inner(
    db_client: &Client,
    bedrock: &BedrockClient,
    _run_id: &str,
    _config: &Config,
) -> StageResult<serde_json::Value> {
    let docs = db::get_all_documents(db_client, Some("enforcement")).await?;
    if docs.is_empty() {
        info!("No enforcement documents found.");
        return Ok(json!({"cases_extracted": 0, "note": "no documents"}));
    }

    let (new_docs, skipped) = documents_needing_cases(db_client, &docs).await?;

    let mut total_cases = 0;
    for doc in &new_docs {
        total_cases += extract_cases_for_document(db_client, bedrock, doc).await?;
    }

    let result = json!({
        "cases_extracted": total_cases,
        "documents_processed": new_docs.len(),
        "documents_skipped": skipped,
    });
    info!(
        "Stage 1 complete: {} cases from {} new documents ({} skipped).",
        total_cases,
        new_docs.len(),
        skipped
    );
    Ok(result)
}

async fn documents_needing_cases<'a>(
    db_client: &Client,
    docs: &'a [Document],
) -> StageResult<(Vec<&'a Document>, usize)> {
    let mut new_docs = Vec::new();
    let mut skipped = 0;
    for doc in docs {
        if db::cases_exist_for_document(db_client, &doc.doc_id).await? {
            skipped += 1;
        } else {
            new_docs.push(doc);
        }
    }
    Ok((new_docs, skipped))
}

async fn extract_cases_for_document(
    db_client: &Client,
    bedrock: &BedrockClient,
    doc: &Document,
) -> StageResult<usize> {
    info!(
        "Processing: {}",
        doc.filename.as_deref().unwrap_or("unknown")
    );
    let truncated = truncate(&doc.full_text, 12000);
    let prompt =
        BedrockClient::render_prompt(STAGE1_EXTRACT_PROMPT, &[("document_text", &truncated)]);
    let response = bedrock
        .invoke_json(&prompt, SYSTEM_PROMPT, None, Some(4096))
        .await?;

    let cases = response_cases(response);
    for case_data in &cases {
        let case = build_case(doc, case_data);
        db::insert_case(db_client, &case).await?;
        info!("Extracted: {}", case.case_name);
    }
    Ok(cases.len())
}

fn response_cases(response: serde_json::Value) -> Vec<serde_json::Value> {
    if response.is_array() {
        return response.as_array().cloned().unwrap_or_default();
    }
    if let Some(cases) = response.get("cases").and_then(|c| c.as_array()) {
        return cases.clone();
    }
    vec![response]
}

fn build_case(doc: &Document, case_data: &serde_json::Value) -> Case {
    let case_name = case_data
        .get("case_name")
        .and_then(|v| v.as_str())
        .unwrap_or("Unknown");

    let mut hasher = Sha256::new();
    hasher.update(format!(
        "{}:{}",
        doc.filename.as_deref().unwrap_or(""),
        case_name
    ));
    let case_id = format!("{:x}", hasher.finalize())[..12].to_string();

    Case {
        case_id,
        source_doc_id: Some(doc.doc_id.clone()),
        case_name: case_name.to_string(),
        scheme_mechanics: json_string(case_data, "scheme_mechanics"),
        exploited_policy: json_string(case_data, "exploited_policy"),
        enabling_condition: json_string(case_data, "enabling_condition"),
        scale_dollars: parse_dollars(case_data.get("scale_dollars")),
        scale_defendants: case_data
            .get("scale_defendants")
            .and_then(|v| v.as_i64())
            .map(|v| v as i32),
        scale_duration: optional_json_string(case_data, "scale_duration"),
        detection_method: optional_json_string(case_data, "detection_method"),
        raw_extraction: Some(case_data.clone()),
        created_at: String::new(),
        qualities: Vec::new(),
    }
}

fn json_string(value: &serde_json::Value, key: &str) -> String {
    value
        .get(key)
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

fn optional_json_string(value: &serde_json::Value, key: &str) -> Option<String> {
    value.get(key).and_then(|v| v.as_str()).map(String::from)
}

fn truncate(text: &str, max_chars: usize) -> String {
    if text.len() <= max_chars {
        text.to_string()
    } else {
        format!(
            "{}\n\n[TRUNCATED -- document continues]",
            &text[..max_chars]
        )
    }
}

/// Parse dollar amounts from LLM output, tolerating messy text.
pub fn parse_dollars(val: Option<&serde_json::Value>) -> Option<f64> {
    let val = val?;
    if let Some(n) = val.as_f64() {
        return Some(n);
    }
    if let Some(n) = val.as_i64() {
        return Some(n as f64);
    }
    let text = val.as_str()?.to_lowercase().replace([',', '$'], "");
    let text = text.trim();
    let re = Regex::new(r"[\d.]+").unwrap();
    let multipliers = [("billion", 1e9), ("million", 1e6), ("thousand", 1e3)];
    for (word, mult) in &multipliers {
        if text.contains(word) {
            return re
                .find(text)
                .and_then(|m| m.as_str().parse::<f64>().ok())
                .map(|v| v * mult);
        }
    }
    re.find(text).and_then(|m| m.as_str().parse::<f64>().ok())
}
