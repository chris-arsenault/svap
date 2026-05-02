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
    let payload = unwrap_step_function_payload(payload);
    let run_id = required_string(&payload, "run_id")?;
    let stage = required_i32(&payload, "stage")?;

    let database_url = resolve_database_url();
    let db_client = db::connect(&database_url).await?;

    if let Some(result) = handle_gate_mode(&db_client, &payload, &run_id, stage).await? {
        return Ok(result);
    }

    let config = load_runtime_config(&payload, context.deadline).await;
    let bedrock = BedrockClient::new(&config.bedrock).await;
    run_pipeline_stage(&db_client, &bedrock, &run_id, stage, &config).await
}

fn unwrap_step_function_payload(payload: Value) -> Value {
    payload.get("Payload").cloned().unwrap_or(payload)
}

fn required_string(payload: &Value, key: &str) -> Result<String, Error> {
    Ok(payload
        .get(key)
        .and_then(|value| value.as_str())
        .ok_or_else(|| format!("Missing {key}"))?
        .to_string())
}

fn required_i32(payload: &Value, key: &str) -> Result<i32, Error> {
    Ok(payload
        .get(key)
        .and_then(|value| value.as_i64())
        .ok_or_else(|| format!("Missing {key}"))? as i32)
}

async fn handle_gate_mode(
    db_client: &tokio_postgres::Client,
    payload: &Value,
    run_id: &str,
    stage: i32,
) -> Result<Option<Value>, Error> {
    let is_gate = payload
        .get("gate")
        .and_then(|gate| gate.as_bool())
        .unwrap_or(false);
    let Some(task_token) = payload.get("task_token").and_then(|token| token.as_str()) else {
        return Ok(None);
    };
    if !is_gate {
        return Ok(None);
    }

    info!(
        "Gate registered for run_id={} stage={}; waiting for approval",
        run_id, stage
    );
    db::log_stage_start(db_client, run_id, stage).await?;
    db::log_stage_pending_review(db_client, run_id, stage).await?;
    db::store_task_token(db_client, run_id, stage, task_token).await?;

    Ok(Some(json!({
        "status": "waiting_for_approval",
        "run_id": run_id,
        "stage": stage,
    })))
}

async fn load_runtime_config(payload: &Value, deadline_ms: u64) -> svap_shared::types::Config {
    let mut config = load_config(payload.get("config_overrides")).await;
    config.deadline = lambda_deadline_seconds(deadline_ms);
    config
}

fn lambda_deadline_seconds(deadline_ms: u64) -> Option<f64> {
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    (deadline_ms > now_ms + 60000)
        .then(|| (now_ms as f64 / 1000.0) + ((deadline_ms - now_ms) as f64 / 1000.0) - 60.0)
}

async fn run_pipeline_stage(
    db_client: &tokio_postgres::Client,
    bedrock: &BedrockClient,
    run_id: &str,
    stage: i32,
    config: &svap_shared::types::Config,
) -> Result<Value, Error> {
    match stages::run_stage(db_client, bedrock, run_id, stage, config).await {
        Ok(result) => Ok(json!({
            "status": "completed",
            "run_id": run_id,
            "stage": stage,
            "result": result,
        })),
        Err(e) => {
            error!("Stage {} failed for run_id={}: {}", stage, run_id, e);
            db::log_stage_failed(db_client, run_id, stage, &e.to_string()).await?;
            Err(e)
        }
    }
}
