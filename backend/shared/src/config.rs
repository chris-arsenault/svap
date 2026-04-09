//! Configuration loading for the SVAP pipeline.
//!
//! Loads config from S3 (Lambda), local file (dev), or falls back to defaults.

use crate::types::{BedrockConfig, Config, PipelineConfig, RagConfig};
use std::env;
use tracing::warn;

/// Build the default configuration (matches Python defaults.py).
pub fn default_config() -> Config {
    Config {
        bedrock: BedrockConfig {
            region: "us-east-1".to_string(),
            model_id: "us.anthropic.claude-sonnet-4-6".to_string(),
            max_tokens: 4096,
            temperature: 0.2,
            retry_attempts: 3,
            retry_delay_seconds: 5,
        },
        rag: RagConfig::default(),
        pipeline: PipelineConfig::default(),
        storage: None,
        discovery: None,
        research: None,
        deadline: None,
    }
}

/// Load pipeline config from S3 (when CONFIG_BUCKET is set) or fall back to defaults.
///
/// In Lambda, config.yaml is stored in the S3 config bucket. In dev, we could
/// load from a local file, but the Rust Lambda doesn't need that — the stage-runner
/// and API both receive config via the event or environment.
pub async fn load_config(overrides: Option<&serde_json::Value>) -> Config {
    let bucket = env::var("SVAP_CONFIG_BUCKET").unwrap_or_default();

    let mut config = if !bucket.is_empty() {
        match load_from_s3(&bucket).await {
            Ok(c) => c,
            Err(e) => {
                warn!("Failed to load config from S3: {e}. Using defaults.");
                default_config()
            }
        }
    } else {
        default_config()
    };

    if let Some(overrides) = overrides {
        apply_overrides(&mut config, overrides);
    }

    config
}

async fn load_from_s3(bucket: &str) -> Result<Config, Box<dyn std::error::Error + Send + Sync>> {
    let sdk_config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
    let s3 = aws_sdk_s3::Client::new(&sdk_config);

    let resp = s3
        .get_object()
        .bucket(bucket)
        .key("config.yaml")
        .send()
        .await?;

    let body = resp.body.collect().await?;
    let yaml_str = String::from_utf8(body.into_bytes().to_vec())?;
    let config: Config = serde_yaml::from_str(&yaml_str)?;
    Ok(config)
}

fn apply_overrides(config: &mut Config, overrides: &serde_json::Value) {
    // Merge top-level keys from the overrides into config.
    // This is a shallow merge matching the Python `config.update(overrides)`.
    if let Some(obj) = overrides.as_object() {
        if let Some(bedrock) = obj.get("bedrock") {
            if let Ok(bc) = serde_json::from_value::<BedrockConfig>(bedrock.clone()) {
                config.bedrock = bc;
            }
        }
        if let Some(rag) = obj.get("rag") {
            if let Ok(rc) = serde_json::from_value::<RagConfig>(rag.clone()) {
                config.rag = rc;
            }
        }
        if let Some(pipeline) = obj.get("pipeline") {
            if let Ok(pc) = serde_json::from_value::<PipelineConfig>(pipeline.clone()) {
                config.pipeline = pc;
            }
        }
    }
}

/// Resolve DATABASE_URL from environment variables.
///
/// Resolution order:
///   1. DATABASE_URL environment variable
///   2. Individual DB_HOST/DB_PORT/DB_NAME/DB_USERNAME/DB_PASSWORD vars
pub fn resolve_database_url() -> String {
    if let Ok(url) = env::var("DATABASE_URL") {
        return url;
    }

    // Build from individual vars (common in ahara platform)
    let host = env::var("DB_HOST").unwrap_or_else(|_| "localhost".to_string());
    let port = env::var("DB_PORT").unwrap_or_else(|_| "5432".to_string());
    let name = env::var("DB_NAME").unwrap_or_else(|_| "svap".to_string());
    let user = env::var("DB_USERNAME").unwrap_or_else(|_| "svap".to_string());
    let pass = env::var("DB_PASSWORD").unwrap_or_else(|_| "password".to_string());

    format!("postgresql://{user}:{pass}@{host}:{port}/{name}")
}
