//! SVAP Stage Runner -- Lambda handler for Step Functions stage execution.
//!
//! Invoked by AWS Step Functions in two modes:
//! 1. Gate mode: stores task token, marks stage as pending_review
//! 2. Stage mode: runs the actual LLM pipeline stage

use lambda_runtime::{run, service_fn, Error, LambdaEvent};
use serde_json::{json, Value};
use tracing::{error, info};

use svap_shared::bedrock::BedrockClient;
use svap_shared::config::{load_config, resolve_database_url};
use svap_shared::db;
use svap_shared::stages;

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing_subscriber::fmt()
        .json()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .init();

    run(service_fn(handler)).await
}

async fn handler(event: LambdaEvent<Value>) -> Result<Value, Error> {
    let (payload, context) = (event.payload, event.context);

    // Step Functions wraps the payload under a 'Payload' key
    let payload = if let Some(inner) = payload.get("Payload") {
        inner.clone()
    } else {
        payload
    };

    let run_id = payload
        .get("run_id")
        .and_then(|r| r.as_str())
        .ok_or("Missing run_id")?
        .to_string();
    let stage = payload
        .get("stage")
        .and_then(|s| s.as_i64())
        .ok_or("Missing stage")? as i32;

    let database_url = resolve_database_url();
    let db_client = db::connect(&database_url).await?;

    // -- Gate mode --
    if payload
        .get("gate")
        .and_then(|g| g.as_bool())
        .unwrap_or(false)
    {
        if let Some(task_token) = payload.get("task_token").and_then(|t| t.as_str()) {
            info!(
                "Gate registered for run_id={} stage={}; waiting for approval",
                run_id, stage
            );
            db::log_stage_start(&db_client, &run_id, stage).await?;
            db::log_stage_pending_review(&db_client, &run_id, stage).await?;
            db::store_task_token(&db_client, &run_id, stage, task_token).await?;

            return Ok(json!({
                "status": "waiting_for_approval",
                "run_id": run_id,
                "stage": stage,
            }));
        }
    }

    // -- Stage mode --
    let config_overrides = payload.get("config_overrides");
    let mut config = load_config(config_overrides).await;

    // Set deadline 60s before Lambda timeout
    // context.deadline is millis since epoch in lambda_runtime 0.13
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let deadline_ms = context.deadline;
    if deadline_ms > now_ms + 60000 {
        config.deadline =
            Some((now_ms as f64 / 1000.0) + ((deadline_ms - now_ms) as f64 / 1000.0) - 60.0);
    }

    let bedrock = BedrockClient::new(&config.bedrock).await;

    match stages::run_stage(&db_client, &bedrock, &run_id, stage, &config).await {
        Ok(result) => Ok(json!({
            "status": "completed",
            "run_id": run_id,
            "stage": stage,
            "result": result,
        })),
        Err(e) => {
            error!("Stage {} failed for run_id={}: {}", stage, run_id, e);
            db::log_stage_failed(&db_client, &run_id, stage, &e.to_string()).await?;
            Err(e)
        }
    }
}
