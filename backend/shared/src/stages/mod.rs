//! Pipeline stage implementations.
//!
//! Each stage is a single async function with signature:
//!   run(client: &Client, bedrock: &BedrockClient, run_id: &str, config: &Config) -> Result<()>

pub mod stage0_source_fetch;
pub mod stage0a_discovery;
pub mod stage1_case_assembly;
pub mod stage2_taxonomy;
pub mod stage3_scoring;
pub mod stage4_scanning;
pub mod stage4a_triage;
pub mod stage4b_research;
pub mod stage4c_assessment;
pub mod stage5_prediction;
pub mod stage6_detection;

use tokio_postgres::Client;

use crate::bedrock::BedrockClient;
use crate::types::Config;

type StageResult = Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>>;

/// Run a single pipeline stage by number.
pub async fn run_stage(
    db: &Client,
    bedrock: &BedrockClient,
    run_id: &str,
    stage: i32,
    config: &Config,
) -> StageResult {
    match stage {
        0 => stage0_source_fetch::run(db, bedrock, run_id, config).await,
        1 => stage1_case_assembly::run(db, bedrock, run_id, config).await,
        2 => stage2_taxonomy::run(db, bedrock, run_id, config).await,
        3 => stage3_scoring::run(db, bedrock, run_id, config).await,
        4 => stage4_scanning::run(db, bedrock, run_id, config).await,
        5 => stage5_prediction::run(db, bedrock, run_id, config).await,
        6 => stage6_detection::run(db, bedrock, run_id, config).await,
        40 => stage4a_triage::run(db, bedrock, run_id, config).await,
        41 => stage4b_research::run(db, bedrock, run_id, config).await,
        42 => stage4c_assessment::run(db, bedrock, run_id, config).await,
        _ => Err(format!("Unknown stage: {stage}").into()),
    }
}
