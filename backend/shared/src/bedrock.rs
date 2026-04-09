//! AWS Bedrock client for Claude API calls.
//!
//! Handles prompt rendering, API calls, structured output parsing,
//! retries, and JSON extraction from LLM responses.

use aws_sdk_bedrockruntime::primitives::Blob;
use aws_sdk_bedrockruntime::Client;
use serde_json::Value;
use std::time::Duration;
use tokio::time::sleep;
use tracing::warn;

use crate::types::BedrockConfig;

/// Prompt template directory, relative to the crate's manifest.
/// In Lambda, prompts are embedded via include_str! in the stages module.
/// This constant is for reference only.
pub const PROMPTS_DIR: &str = "src/svap/prompts";

pub struct BedrockClient {
    client: Client,
    pub model_id: String,
    pub max_tokens: i32,
    pub default_temperature: f64,
    pub retry_attempts: u32,
    pub retry_delay: u64,
}

impl BedrockClient {
    pub async fn new(config: &BedrockConfig) -> Self {
        let sdk_config = aws_config::defaults(aws_config::BehaviorVersion::latest())
            .region(aws_config::Region::new(config.region.clone()))
            .load()
            .await;

        let client = Client::new(&sdk_config);

        Self {
            client,
            model_id: config.model_id.clone(),
            max_tokens: config.max_tokens,
            default_temperature: config.temperature,
            retry_attempts: config.retry_attempts,
            retry_delay: config.retry_delay_seconds,
        }
    }

    /// Send a prompt to Claude via Bedrock and return the text response.
    pub async fn invoke(
        &self,
        prompt: &str,
        system: &str,
        temperature: Option<f64>,
        max_tokens: Option<i32>,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let temp = temperature.unwrap_or(self.default_temperature);
        let tokens = max_tokens.unwrap_or(self.max_tokens);

        let mut body = serde_json::json!({
            "anthropic_version": "bedrock-2023-05-31",
            "max_tokens": tokens,
            "temperature": temp,
            "messages": [
                {
                    "role": "user",
                    "content": [{"type": "text", "text": prompt}]
                }
            ]
        });

        if !system.is_empty() {
            body["system"] = serde_json::json!([{"type": "text", "text": system}]);
        }

        let body_bytes = serde_json::to_vec(&body)?;

        for attempt in 0..self.retry_attempts {
            match self
                .client
                .invoke_model()
                .model_id(&self.model_id)
                .content_type("application/json")
                .accept("application/json")
                .body(Blob::new(body_bytes.clone()))
                .send()
                .await
            {
                Ok(response) => {
                    let result: Value = serde_json::from_slice(response.body().as_ref())?;

                    let text_parts: Vec<&str> = result
                        .get("content")
                        .and_then(|c| c.as_array())
                        .map(|blocks| {
                            blocks
                                .iter()
                                .filter_map(|block| {
                                    if block.get("type").and_then(|t| t.as_str()) == Some("text") {
                                        block.get("text").and_then(|t| t.as_str())
                                    } else {
                                        None
                                    }
                                })
                                .collect()
                        })
                        .unwrap_or_default();

                    return Ok(text_parts.join("\n"));
                }
                Err(e) => {
                    if attempt < self.retry_attempts - 1 {
                        let wait = self.retry_delay * 2u64.pow(attempt);
                        warn!(
                            "Bedrock call failed (attempt {}): {}. Retrying in {}s...",
                            attempt + 1,
                            e,
                            wait
                        );
                        sleep(Duration::from_secs(wait)).await;
                    } else {
                        return Err(format!(
                            "Bedrock call failed after {} attempts: {e}",
                            self.retry_attempts
                        )
                        .into());
                    }
                }
            }
        }

        unreachable!()
    }

    /// Invoke and parse JSON from the response. Handles markdown fences.
    pub async fn invoke_json(
        &self,
        prompt: &str,
        system: &str,
        temperature: Option<f64>,
        max_tokens: Option<i32>,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let raw = self.invoke(prompt, system, temperature, max_tokens).await?;
        parse_json_response(&raw)
    }

    /// Load a prompt template and fill in variables.
    ///
    /// `template` is the raw template text (loaded via include_str! by the caller).
    /// Variables are `{key}` placeholders.
    pub fn render_prompt(template: &str, variables: &[(&str, &str)]) -> String {
        let mut result = template.to_string();
        for (key, value) in variables {
            result = result.replace(&format!("{{{key}}}"), value);
        }
        result
    }
}

/// Extract JSON from an LLM response, handling markdown fences and preamble.
pub fn parse_json_response(text: &str) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
    let mut cleaned = text.trim().to_string();

    // Strip markdown json fences
    if cleaned.starts_with("```json") {
        cleaned = cleaned[7..].to_string();
    } else if cleaned.starts_with("```") {
        cleaned = cleaned[3..].to_string();
    }
    if cleaned.ends_with("```") {
        cleaned = cleaned[..cleaned.len() - 3].to_string();
    }
    cleaned = cleaned.trim().to_string();

    // Try direct parse
    if let Ok(val) = serde_json::from_str::<Value>(&cleaned) {
        return Ok(val);
    }

    // Try to find JSON object or array in the text
    for (start_char, end_char) in [("{", "}"), ("[", "]")] {
        if let Some(start) = cleaned.find(start_char) {
            if let Some(end) = cleaned.rfind(end_char) {
                if end > start {
                    if let Ok(val) = serde_json::from_str::<Value>(&cleaned[start..=end]) {
                        return Ok(val);
                    }
                }
            }
        }
    }

    Err(format!(
        "Could not parse JSON from response:\n{}",
        &text[..text.len().min(500)]
    )
    .into())
}
