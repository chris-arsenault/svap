//! RAG (Retrieval-Augmented Generation) utilities.
//!
//! Handles document ingestion, chunking, retrieval, and context assembly.
//! Uses keyword-based retrieval by default (matching the Python implementation).

use regex::Regex;
use sha2::{Digest, Sha256};
use tokio_postgres::Client;

use crate::db;
use crate::types::{Case, Config, TaxonomyQuality};

/// Count tokens using tiktoken cl100k_base encoding.
pub fn count_tokens(text: &str) -> usize {
    tiktoken_rs::cl100k_base()
        .map(|bpe| bpe.encode_with_special_tokens(text).len())
        .unwrap_or_else(|_| text.split_whitespace().count() * 4 / 3)
}

// ── Document Ingester ────────────────────────────────────────────────────

pub struct DocumentIngester {
    chunk_size: usize,
    chunk_overlap: usize,
}

impl DocumentIngester {
    pub fn new(config: &Config) -> Self {
        Self {
            chunk_size: config.rag.chunk_size,
            chunk_overlap: config.rag.chunk_overlap,
        }
    }

    /// Ingest raw text directly. Returns (doc_id, n_chunks).
    pub async fn ingest_text(
        &self,
        client: &Client,
        text: &str,
        filename: &str,
        doc_type: &str,
        metadata: Option<&serde_json::Value>,
    ) -> Result<(String, usize), Box<dyn std::error::Error + Send + Sync>> {
        let mut hasher = Sha256::new();
        hasher.update(format!("{}:{}", filename, &text[..text.len().min(200)]));
        let doc_id = format!("{:x}", hasher.finalize())[..16].to_string();

        db::insert_document(client, &doc_id, filename, doc_type, text, metadata).await?;

        let chunks = self.chunk_text(text);
        for (i, chunk_text) in chunks.iter().enumerate() {
            let chunk_id = format!("{}_c{:04}", doc_id, i);
            let token_count = count_tokens(chunk_text) as i32;
            db::insert_chunk(
                client,
                &chunk_id,
                &doc_id,
                i as i32,
                chunk_text,
                token_count,
            )
            .await?;
        }

        Ok((doc_id, chunks.len()))
    }

    fn chunk_text(&self, text: &str) -> Vec<String> {
        let paragraphs: Vec<&str> = Regex::new(r"\n\s*\n").unwrap().split(text).collect();
        let mut chunks = Vec::new();
        let mut current_chunk = String::new();
        let mut current_tokens = 0;

        for para in &paragraphs {
            let para_tokens = count_tokens(para);
            if current_tokens + para_tokens > self.chunk_size && !current_chunk.is_empty() {
                chunks.push(current_chunk.trim().to_string());
                let overlap_text = self.get_overlap(&current_chunk);
                current_chunk = format!("{}\n\n{}", overlap_text, para);
                current_tokens = count_tokens(&current_chunk);
            } else {
                if current_chunk.is_empty() {
                    current_chunk = para.to_string();
                } else {
                    current_chunk.push_str("\n\n");
                    current_chunk.push_str(para);
                }
                current_tokens += para_tokens;
            }
        }

        if !current_chunk.trim().is_empty() {
            chunks.push(current_chunk.trim().to_string());
        }

        if chunks.is_empty() {
            vec![text.to_string()]
        } else {
            chunks
        }
    }

    fn get_overlap(&self, text: &str) -> String {
        let words: Vec<&str> = text.split_whitespace().collect();
        let overlap_words = (self.chunk_overlap * 3 / 4).max(1);
        if words.len() > overlap_words {
            words[words.len() - overlap_words..].join(" ")
        } else {
            text.to_string()
        }
    }
}

// ── Context Assembler ────────────────────────────────────────────────────

pub struct ContextAssembler {
    max_chunks: usize,
}

impl ContextAssembler {
    pub fn new(config: &Config) -> Self {
        Self {
            max_chunks: config.rag.max_context_chunks,
        }
    }

    /// Retrieve relevant chunks and format as context block.
    pub async fn retrieve(
        &self,
        client: &Client,
        query: &str,
        doc_type: Option<&str>,
        max_chunks: Option<usize>,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let limit = max_chunks.unwrap_or(self.max_chunks);
        let chunks = db::search_chunks(client, query, doc_type, limit).await?;

        if chunks.is_empty() {
            return Ok(String::new());
        }

        let context_parts: Vec<String> = chunks
            .iter()
            .map(|chunk| {
                let source = chunk.filename.as_deref().unwrap_or("unknown");
                format!("[Source: {}]\n{}", source, chunk.text)
            })
            .collect();

        Ok(context_parts.join("\n\n---\n\n"))
    }

    /// Return all documents of a given type concatenated.
    pub async fn retrieve_all_of_type(
        &self,
        client: &Client,
        doc_type: &str,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let docs = db::get_all_documents(client, Some(doc_type)).await?;
        let parts: Vec<String> = docs
            .iter()
            .map(|d| {
                format!(
                    "[{}]\n{}",
                    d.filename.as_deref().unwrap_or("unknown"),
                    d.full_text
                )
            })
            .collect();
        Ok(parts.join("\n\n===\n\n"))
    }

    /// Format case data as structured context for prompts.
    pub fn format_cases_context(cases: &[Case]) -> String {
        cases
            .iter()
            .map(|c| {
                format!(
                    "CASE: {}\n  Scheme: {}\n  Exploited Policy: {}\n  Enabling Condition: {}\n  Scale: ${}\n  Detection: {}",
                    c.case_name,
                    c.scheme_mechanics,
                    c.exploited_policy,
                    c.enabling_condition,
                    c.scale_dollars.map(|v| format!("{:.0}", v)).unwrap_or_else(|| "unknown".to_string()),
                    c.detection_method.as_deref().unwrap_or("unknown"),
                )
            })
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    /// Format taxonomy as structured context for prompts.
    pub fn format_taxonomy_context(taxonomy: &[TaxonomyQuality]) -> String {
        taxonomy
            .iter()
            .map(|q| {
                format!(
                    "{} -- {}\n  Definition: {}\n  Recognition Test: {}\n  Exploitation Logic: {}",
                    q.quality_id, q.name, q.definition, q.recognition_test, q.exploitation_logic,
                )
            })
            .collect::<Vec<_>>()
            .join("\n\n")
    }
}
