//! SVAP API -- Lambda handler for API Gateway V2 HTTP API.
//!
//! Routes directly on event["routeKey"] without a web framework.
//! API Gateway handles CORS, JWT auth, and path parameter extraction.

use chrono::Utc;
use lambda_http::{run, service_fn, Body, Error, Request, Response};
use regex::Regex;
use serde_json::{json, Value};
use std::env;
use tracing::{error, info};

use svap_shared::config::{load_config, resolve_database_url};
use svap_shared::db;
use svap_shared::types::*;

type LambdaResult = Result<Response<Body>, Error>;
type ApiResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

struct RouteInfo {
    method: String,
    path: String,
    route_key: String,
}

impl RouteInfo {
    fn from_request(event: &Request) -> Self {
        let method = event.method().to_string();
        let path = event.uri().path().to_string();
        let route_key = format!("{} {}", method, path);
        Self {
            method,
            path,
            route_key,
        }
    }
}

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

async fn handler(event: Request) -> LambdaResult {
    let route_info = RouteInfo::from_request(&event);
    info!("Request: {}", route_info.route_key);

    let db_client = match connect_database().await {
        Ok(client) => client,
        Err(response) => return response,
    };
    let result = route(&route_info, &event, &db_client).await;
    route_response(result, &route_info.route_key)
}

fn ok_json(status: u16, body: Value) -> LambdaResult {
    Ok(Response::builder()
        .status(status)
        .header("Content-Type", "application/json")
        .body(Body::Text(serde_json::to_string(&body).unwrap_or_default()))
        .unwrap())
}

fn api_error(code: u16, msg: &str) -> Box<dyn std::error::Error + Send + Sync> {
    format!("{}:{}", code, msg).into()
}

async fn connect_database() -> Result<tokio_postgres::Client, LambdaResult> {
    let database_url = resolve_database_url();
    match db::connect(&database_url).await {
        Ok(client) => Ok(client),
        Err(e) => {
            error!("Database connection failed: {}", e);
            Err(ok_json(
                500,
                json!({"detail": "Database connection failed"}),
            ))
        }
    }
}

fn route_response(result: ApiResult<Value>, route_key: &str) -> LambdaResult {
    match result {
        Ok(body) => success_response(body),
        Err(e) => error_response(&e.to_string(), route_key),
    }
}

fn success_response(body: Value) -> LambdaResult {
    let status = body
        .get("statusCode")
        .and_then(|status| status.as_u64())
        .unwrap_or(200) as u16;
    ok_json(status, body)
}

fn error_response(message: &str, route_key: &str) -> LambdaResult {
    for code in [404, 400, 409] {
        if let Some(detail) = message.strip_prefix(&format!("{code}:")) {
            return ok_json(code, json!({"detail": detail.trim()}));
        }
    }
    error!("Unhandled error on {}: {}", route_key, message);
    ok_json(500, json!({"detail": "Internal server error"}))
}

fn json_body(event: &Request) -> Value {
    match event.body() {
        Body::Text(text) => serde_json::from_str(text).unwrap_or(json!({})),
        Body::Binary(bytes) => serde_json::from_slice(bytes).unwrap_or(json!({})),
        Body::Empty => json!({}),
    }
}

fn query_param(event: &Request, name: &str) -> Option<String> {
    event.uri().query().and_then(|q| {
        q.split('&').find_map(|pair| {
            let (key, val) = pair.split_once('=')?;

            if key == name {
                Some(val.to_string())
            } else {
                None
            }
        })
    })
}

async fn route(
    route_info: &RouteInfo,
    event: &Request,
    db_client: &tokio_postgres::Client,
) -> ApiResult<Value> {
    if let Some(response) = get_route(route_info, event, db_client).await? {
        return Ok(response);
    }
    if let Some(response) = post_route(route_info, event, db_client).await? {
        return Ok(response);
    }
    if let Some(response) = dynamic_get_route(route_info, db_client).await? {
        return Ok(response);
    }
    Err(api_error(
        404,
        &format!("Not found: {}", route_info.route_key),
    ))
}

async fn get_route(
    route_info: &RouteInfo,
    event: &Request,
    db_client: &tokio_postgres::Client,
) -> ApiResult<Option<Value>> {
    let is_lambda = env::var("AWS_LAMBDA_FUNCTION_NAME").is_ok();
    let response = match route_info.route_key.as_str() {
        "GET /api/health" => {
            json!({"status": "ok", "database": "postgresql", "lambda": is_lambda})
        }
        "GET /api/status" => status_response_body(db_client).await?,
        "GET /api/dashboard" => dashboard_response(db_client).await?,
        "GET /api/cases" => cases_response(db_client).await?,
        "GET /api/taxonomy" => json!(db::get_taxonomy(db_client).await?),
        "GET /api/policies" => policies_response(db_client).await?,
        "GET /api/predictions" => predictions_response(db_client).await?,
        "GET /api/detection-patterns" => json!(db::get_detection_patterns(db_client).await?),
        "GET /api/convergence/cases" => convergence_cases_response(db_client).await?,
        "GET /api/convergence/policies" => convergence_policies_response(db_client).await?,
        "GET /api/enforcement-sources" => json!(db::get_enforcement_sources(db_client).await?),
        "GET /api/dimensions" => json!(db::get_dimensions(db_client).await?),
        "GET /api/management/runs" => json!(db::list_runs(db_client).await?),
        "GET /api/research/triage" => json!(db::get_triage_results(db_client).await?),
        "GET /api/research/sessions" => research_sessions_response(db_client, event).await?,
        "GET /api/discovery/candidates" => discovery_candidates_response(db_client, event).await?,
        "GET /api/discovery/feeds" => json!(db::get_source_feeds(db_client, false).await?),
        _ => return Ok(None),
    };
    Ok(Some(response))
}

async fn post_route(
    route_info: &RouteInfo,
    event: &Request,
    db_client: &tokio_postgres::Client,
) -> ApiResult<Option<Value>> {
    let is_lambda = env::var("AWS_LAMBDA_FUNCTION_NAME").is_ok();
    let response = match route_info.route_key.as_str() {
        "POST /api/pipeline/run" => start_pipeline(db_client, event, is_lambda).await?,
        "POST /api/pipeline/approve" => approve_pipeline_stage(db_client, event).await?,
        "POST /api/enforcement-sources" => create_enforcement_source(db_client, event).await?,
        "POST /api/enforcement-sources/delete" => {
            delete_enforcement_source(db_client, event).await?
        }
        "POST /api/discovery/feeds" => create_discovery_feed(db_client, event).await?,
        "POST /api/management/runs/delete" => delete_run(db_client, event).await?,
        _ => return Ok(None),
    };
    Ok(Some(response))
}

async fn dynamic_get_route(
    route_info: &RouteInfo,
    db_client: &tokio_postgres::Client,
) -> ApiResult<Option<Value>> {
    if route_info.method != "GET" {
        return Ok(None);
    }
    if let Some(case_id) = extract_param(&route_info.path, "/api/cases/") {
        return Ok(Some(case_response(db_client, &case_id).await?));
    }
    if let Some(quality_id) = extract_param(&route_info.path, "/api/taxonomy/") {
        return Ok(Some(quality_response(db_client, &quality_id).await?));
    }
    if let Some(policy_id) = extract_param(&route_info.path, "/api/policies/") {
        return Ok(Some(policy_response(db_client, &policy_id).await?));
    }
    if let Some(policy_id) = extract_param(&route_info.path, "/api/research/findings/") {
        let findings = db::get_structural_findings(db_client, &policy_id).await?;
        return Ok(Some(json!({"policy_id": policy_id, "findings": findings})));
    }
    if let Some(policy_id) = extract_param(&route_info.path, "/api/research/assessments/") {
        let assessments = db::get_quality_assessments(db_client, Some(&policy_id)).await?;
        return Ok(Some(
            json!({"policy_id": policy_id, "assessments": assessments}),
        ));
    }
    Ok(None)
}

async fn status_response_body(db_client: &tokio_postgres::Client) -> ApiResult<Value> {
    let run_id = db::get_latest_run(db_client).await?.unwrap_or_default();
    let stages = pipeline_status_for_run(db_client, &run_id).await?;
    let counts = db::get_corpus_counts(db_client).await?;
    Ok(json!({"run_id": run_id, "stages": stages, "counts": counts}))
}

async fn pipeline_status_for_run(
    db_client: &tokio_postgres::Client,
    run_id: &str,
) -> ApiResult<Vec<StageStatusEntry>> {
    if run_id.is_empty() {
        return Ok(Vec::new());
    }
    db::get_pipeline_status(db_client, run_id).await
}

async fn dashboard_response(db_client: &tokio_postgres::Client) -> ApiResult<Value> {
    let run_id = db::get_latest_run(db_client).await?.unwrap_or_default();
    let cases = db::get_cases(db_client).await?;
    let taxonomy = db::get_taxonomy(db_client).await?;
    let policies = db::get_policies(db_client).await?;
    let pipeline_status = pipeline_status_for_run(db_client, &run_id).await?;
    let convergence_matrix = db::get_convergence_matrix(db_client).await?;
    let policy_scores = db::get_policy_scores(db_client).await?;
    let calibration = db::get_calibration(db_client).await?;
    let trees = db::get_exploitation_trees(db_client, false).await?;
    let all_steps = db::get_all_exploitation_steps(db_client).await?;
    let patterns = db::get_detection_patterns(db_client).await?;
    let enforcement_sources = db::get_enforcement_sources(db_client).await?;

    Ok(json!({
        "run_id": run_id,
        "source": "api",
        "pipeline_status": pipeline_status,
        "counts": {
            "cases": cases.len(),
            "taxonomy_qualities": taxonomy.len(),
            "policies": policies.len(),
            "exploitation_trees": trees.len(),
            "detection_patterns": patterns.len(),
        },
        "calibration": calibration.as_ref().map(|c| json!({"threshold": c.threshold})).unwrap_or(json!({"threshold": 3})),
        "cases": enrich_cases(cases, &convergence_matrix),
        "taxonomy": taxonomy,
        "policies": enrich_policies(policies, &policy_scores, calibration.as_ref()),
        "exploitation_trees": enrich_trees(trees, all_steps),
        "detection_patterns": patterns,
        "enforcement_sources": enforcement_sources,
    }))
}

async fn cases_response(db_client: &tokio_postgres::Client) -> ApiResult<Value> {
    let cases = db::get_cases(db_client).await?;
    let matrix = db::get_convergence_matrix(db_client).await?;
    Ok(json!(enrich_cases(cases, &matrix)))
}

async fn policies_response(db_client: &tokio_postgres::Client) -> ApiResult<Value> {
    let policies = db::get_policies(db_client).await?;
    let scores = db::get_policy_scores(db_client).await?;
    let calibration = db::get_calibration(db_client).await?;
    Ok(json!(enrich_policies(
        policies,
        &scores,
        calibration.as_ref()
    )))
}

async fn predictions_response(db_client: &tokio_postgres::Client) -> ApiResult<Value> {
    let trees = db::get_exploitation_trees(db_client, false).await?;
    let steps = db::get_all_exploitation_steps(db_client).await?;
    Ok(json!(enrich_trees(trees, steps)))
}

async fn convergence_cases_response(db_client: &tokio_postgres::Client) -> ApiResult<Value> {
    let matrix = db::get_convergence_matrix(db_client).await?;
    let calibration = db::get_calibration(db_client).await?;
    Ok(json!({"matrix": matrix, "calibration": calibration}))
}

async fn convergence_policies_response(db_client: &tokio_postgres::Client) -> ApiResult<Value> {
    let scores = db::get_policy_scores(db_client).await?;
    let calibration = db::get_calibration(db_client).await?;
    Ok(json!({"scores": scores, "calibration": calibration}))
}

async fn research_sessions_response(
    db_client: &tokio_postgres::Client,
    event: &Request,
) -> ApiResult<Value> {
    let status = query_param(event, "status");
    let sessions = db::get_research_sessions(db_client, status.as_deref()).await?;
    Ok(json!(sessions))
}

async fn discovery_candidates_response(
    db_client: &tokio_postgres::Client,
    event: &Request,
) -> ApiResult<Value> {
    let feed_id = query_param(event, "feed_id");
    let status = query_param(event, "status");
    let candidates = db::get_candidates(db_client, feed_id.as_deref(), status.as_deref()).await?;
    Ok(json!(candidates))
}

async fn start_pipeline(
    db_client: &tokio_postgres::Client,
    event: &Request,
    is_lambda: bool,
) -> ApiResult<Value> {
    let body = json_body(event);
    let run_id = format!("run_{}", Utc::now().format("%Y%m%d_%H%M%S"));
    let config = load_config(body.get("config_overrides")).await;
    db::create_run(
        db_client,
        &run_id,
        &serde_json::to_value(&config)?,
        body.get("notes").and_then(|n| n.as_str()).unwrap_or(""),
    )
    .await?;

    if let Some(execution_arn) = start_step_function(&run_id, is_lambda).await? {
        return Ok(json!({
            "statusCode": 202,
            "status": "started",
            "run_id": run_id,
            "execution_arn": execution_arn,
        }));
    }

    Ok(json!({"statusCode": 202, "status": "started", "run_id": run_id}))
}

async fn start_step_function(run_id: &str, is_lambda: bool) -> ApiResult<Option<String>> {
    let sfn_arn = env::var("PIPELINE_STATE_MACHINE_ARN").unwrap_or_default();
    if !is_lambda || sfn_arn.is_empty() {
        return Ok(None);
    }
    let sdk_config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
    let sfn = aws_sdk_sfn::Client::new(&sdk_config);
    let resp = sfn
        .start_execution()
        .state_machine_arn(&sfn_arn)
        .name(run_id)
        .input(serde_json::to_string(&json!({"run_id": run_id}))?)
        .send()
        .await?;
    Ok(Some(resp.execution_arn().to_string()))
}

async fn approve_pipeline_stage(
    db_client: &tokio_postgres::Client,
    event: &Request,
) -> ApiResult<Value> {
    let body = json_body(event);
    let stage = required_stage(&body)?;
    let run_id = db::get_latest_run(db_client)
        .await?
        .ok_or_else(|| api_error(404, "No pipeline runs found"))?;
    ensure_pending_review(db_client, &run_id, stage).await?;
    db::approve_stage(db_client, &run_id, stage).await?;
    Ok(json!({"status": "approved", "stage": stage}))
}

fn required_stage(body: &Value) -> ApiResult<i32> {
    let stage = body
        .get("stage")
        .and_then(|s| s.as_i64())
        .ok_or_else(|| api_error(400, "Missing stage"))? as i32;
    if stage == 2 || stage == 5 {
        Ok(stage)
    } else {
        Err(api_error(
            400,
            "Only stages 2 and 5 have human review gates.",
        ))
    }
}

async fn ensure_pending_review(
    db_client: &tokio_postgres::Client,
    run_id: &str,
    stage: i32,
) -> ApiResult<()> {
    let status = db::get_stage_status(db_client, run_id, stage).await?;
    if status.as_deref() == Some("pending_review") {
        return Ok(());
    }
    Err(api_error(
        400,
        &format!("Stage {} is '{:?}', not pending review.", stage, status),
    ))
}

async fn create_enforcement_source(
    db_client: &tokio_postgres::Client,
    event: &Request,
) -> ApiResult<Value> {
    let body = json_body(event);
    let name = required_trimmed(&body, "name")?;
    let source_id = body
        .get("source_id")
        .and_then(|s| s.as_str())
        .map(String::from)
        .unwrap_or_else(|| slug_id(&name));
    ensure_source_available(db_client, &source_id).await?;

    let source = enforcement_source_from_body(&body, &source_id, name);
    db::upsert_enforcement_source(db_client, &source).await?;
    Ok(json!(
        db::get_enforcement_source(db_client, &source_id).await?
    ))
}

async fn ensure_source_available(
    db_client: &tokio_postgres::Client,
    source_id: &str,
) -> ApiResult<()> {
    if db::get_enforcement_source(db_client, source_id)
        .await?
        .is_none()
    {
        return Ok(());
    }
    Err(api_error(
        409,
        &format!("Source '{}' already exists", source_id),
    ))
}

fn enforcement_source_from_body(body: &Value, source_id: &str, name: String) -> EnforcementSource {
    EnforcementSource {
        source_id: source_id.to_string(),
        name,
        url: json_opt_string(body, "url"),
        source_type: body
            .get("source_type")
            .and_then(|t| t.as_str())
            .unwrap_or("press_release")
            .to_string(),
        description: json_opt_string(body, "description"),
        has_document: false,
        s3_key: None,
        doc_id: None,
        summary: None,
        validation_status: Some("pending".to_string()),
        created_at: Utc::now().to_rfc3339(),
        updated_at: Utc::now().to_rfc3339(),
        candidate_id: None,
        feed_id: None,
    }
}

async fn delete_enforcement_source(
    db_client: &tokio_postgres::Client,
    event: &Request,
) -> ApiResult<Value> {
    let body = json_body(event);
    let source_id = required_str(&body, "source_id")?;
    if db::get_enforcement_source(db_client, source_id)
        .await?
        .is_none()
    {
        return Err(api_error(404, &format!("Source '{}' not found", source_id)));
    }
    db::delete_enforcement_source(db_client, source_id).await?;
    Ok(json!({"status": "deleted", "source_id": source_id}))
}

async fn create_discovery_feed(
    db_client: &tokio_postgres::Client,
    event: &Request,
) -> ApiResult<Value> {
    let body = json_body(event);
    let name = required_trimmed(&body, "name")?;
    let listing_url = required_trimmed(&body, "listing_url")?;
    let feed_id = slug_id(&name);
    let feed = SourceFeed {
        feed_id: feed_id.clone(),
        name,
        listing_url,
        content_type: body
            .get("content_type")
            .and_then(|t| t.as_str())
            .unwrap_or("press_release")
            .to_string(),
        link_selector: json_opt_string(&body, "link_selector"),
        last_checked_at: None,
        last_entry_url: None,
        enabled: Some(true),
        created_at: Utc::now().to_rfc3339(),
        updated_at: Utc::now().to_rfc3339(),
    };
    db::upsert_source_feed(db_client, &feed).await?;
    Ok(json!({"status": "created", "feed_id": feed_id}))
}

async fn delete_run(db_client: &tokio_postgres::Client, event: &Request) -> ApiResult<Value> {
    let body = json_body(event);
    let run_id = required_str(&body, "run_id")?.trim();
    if run_id.is_empty() {
        return Err(api_error(400, "Missing required field: run_id"));
    }
    db::delete_run(db_client, run_id).await?;
    Ok(json!({"status": "deleted", "run_id": run_id}))
}

async fn case_response(db_client: &tokio_postgres::Client, case_id: &str) -> ApiResult<Value> {
    let cases = db::get_cases(db_client).await?;
    let matrix = db::get_convergence_matrix(db_client).await?;
    let case = enrich_cases(cases, &matrix)
        .into_iter()
        .find(|case| case.case_id == case_id);
    case.map(|case| json!(case))
        .ok_or_else(|| api_error(404, &format!("Case {} not found", case_id)))
}

async fn quality_response(
    db_client: &tokio_postgres::Client,
    quality_id: &str,
) -> ApiResult<Value> {
    let taxonomy = db::get_taxonomy(db_client).await?;
    let quality = taxonomy
        .into_iter()
        .find(|quality| quality.quality_id == quality_id);
    quality
        .map(|quality| json!(quality))
        .ok_or_else(|| api_error(404, &format!("Quality {} not found", quality_id)))
}

async fn policy_response(db_client: &tokio_postgres::Client, policy_id: &str) -> ApiResult<Value> {
    let policies = db::get_policies(db_client).await?;
    let scores = db::get_policy_scores(db_client).await?;
    let calibration = db::get_calibration(db_client).await?;
    let policy = enrich_policies(policies, &scores, calibration.as_ref())
        .into_iter()
        .find(|policy| policy.policy_id == policy_id);
    policy
        .map(|policy| json!(policy))
        .ok_or_else(|| api_error(404, &format!("Policy {} not found", policy_id)))
}

fn required_str<'a>(body: &'a Value, key: &str) -> ApiResult<&'a str> {
    body.get(key)
        .and_then(|value| value.as_str())
        .ok_or_else(|| api_error(400, &format!("Missing {key}")))
}

fn required_trimmed(body: &Value, key: &str) -> ApiResult<String> {
    let value = required_str(body, key)?.trim().to_string();
    if value.is_empty() {
        Err(api_error(400, &format!("Missing required field: {key}")))
    } else {
        Ok(value)
    }
}

fn slug_id(name: &str) -> String {
    let re = Regex::new(r"[^a-z0-9_]").unwrap();
    re.replace_all(&name.to_lowercase().replace(' ', "_"), "")
        .chars()
        .take(50)
        .collect()
}

fn json_opt_string(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(|v| v.as_str()).map(String::from)
}

fn extract_param(path: &str, prefix: &str) -> Option<String> {
    if let Some(rest) = path.strip_prefix(prefix) {
        // Take until next / or end
        let param = rest.split('/').next().unwrap_or("");
        if !param.is_empty() {
            return Some(param.to_string());
        }
    }
    None
}

fn compute_risk_level(score: i32, threshold: i32) -> String {
    if score >= threshold + 2 {
        "critical".to_string()
    } else if score >= threshold {
        "high".to_string()
    } else if score >= threshold - 1 {
        "medium".to_string()
    } else {
        "low".to_string()
    }
}

fn enrich_cases(mut cases: Vec<Case>, matrix: &[ConvergenceRow]) -> Vec<Case> {
    let mut case_qualities: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();
    for row in matrix {
        if row.present {
            case_qualities
                .entry(row.case_id.clone())
                .or_default()
                .push(row.quality_id.clone());
        }
    }
    for case in &mut cases {
        if let Some(quals) = case_qualities.get(&case.case_id) {
            case.qualities = quals.clone();
            case.qualities.sort();
        }
    }
    cases
}

fn enrich_policies(
    mut policies: Vec<Policy>,
    policy_scores: &[PolicyScore],
    calibration: Option<&Calibration>,
) -> Vec<Policy> {
    let threshold = calibration.map(|c| c.threshold).unwrap_or(3);
    let mut policy_qualities: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();
    for row in policy_scores {
        if row.present {
            policy_qualities
                .entry(row.policy_id.clone())
                .or_default()
                .push(row.quality_id.clone());
        }
    }
    for policy in &mut policies {
        if let Some(quals) = policy_qualities.get(&policy.policy_id) {
            let mut sorted = quals.clone();
            sorted.sort();
            let score = sorted.len() as i32;
            policy.qualities = sorted;
            policy.convergence_score = Some(score);
            policy.risk_level = Some(compute_risk_level(score, threshold));
        }
    }
    policies
}

fn enrich_trees(
    mut trees: Vec<ExploitationTree>,
    all_steps: Vec<ExploitationStep>,
) -> Vec<ExploitationTree> {
    let mut steps_by_tree: std::collections::HashMap<String, Vec<ExploitationStep>> =
        std::collections::HashMap::new();
    for step in all_steps {
        steps_by_tree
            .entry(step.tree_id.clone())
            .or_default()
            .push(step);
    }
    for tree in &mut trees {
        if let Some(steps) = steps_by_tree.remove(&tree.tree_id) {
            tree.steps = steps;
        }
    }
    trees
}
