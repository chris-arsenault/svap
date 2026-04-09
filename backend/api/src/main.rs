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
    // Reconstruct route key from method + path
    let method = event.method().to_string();
    let path = event.uri().path().to_string();
    let route_key = format!("{} {}", method, path);

    info!("Request: {}", route_key);

    let database_url = resolve_database_url();
    let db_client = match db::connect(&database_url).await {
        Ok(c) => c,
        Err(e) => {
            error!("Database connection failed: {}", e);
            return ok_json(500, json!({"detail": "Database connection failed"}));
        }
    };

    // Extract path parameters by matching route patterns
    let result = route(&route_key, &method, &path, &event, &db_client).await;

    match result {
        Ok(body) => {
            // If the body already has a statusCode, it's a raw API GW response
            if let Some(status) = body.get("statusCode").and_then(|s| s.as_u64()) {
                ok_json(status as u16, body)
            } else {
                ok_json(200, body)
            }
        }
        Err(e) => {
            let msg = e.to_string();
            if let Some(detail) = msg.strip_prefix("404:") {
                ok_json(404, json!({"detail": detail.trim()}))
            } else if let Some(detail) = msg.strip_prefix("400:") {
                ok_json(400, json!({"detail": detail.trim()}))
            } else if let Some(detail) = msg.strip_prefix("409:") {
                ok_json(409, json!({"detail": detail.trim()}))
            } else {
                error!("Unhandled error on {}: {}", route_key, msg);
                ok_json(500, json!({"detail": "Internal server error"}))
            }
        }
    }
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
    route_key: &str,
    method: &str,
    path: &str,
    event: &Request,
    db_client: &tokio_postgres::Client,
) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
    // Static data routes
    let is_lambda = env::var("AWS_LAMBDA_FUNCTION_NAME").is_ok();

    match route_key {
        "GET /api/health" => {
            Ok(json!({"status": "ok", "database": "postgresql", "lambda": is_lambda}))
        }

        "GET /api/status" => {
            let run_id = db::get_latest_run(db_client).await?.unwrap_or_default();
            let stages = if !run_id.is_empty() {
                db::get_pipeline_status(db_client, &run_id).await?
            } else {
                Vec::new()
            };
            let counts = db::get_corpus_counts(db_client).await?;
            Ok(json!({"run_id": run_id, "stages": stages, "counts": counts}))
        }

        "GET /api/dashboard" => {
            let run_id = db::get_latest_run(db_client).await?.unwrap_or_default();
            let cases = db::get_cases(db_client).await?;
            let taxonomy = db::get_taxonomy(db_client).await?;
            let policies = db::get_policies(db_client).await?;
            let pipeline_status = if !run_id.is_empty() {
                db::get_pipeline_status(db_client, &run_id).await?
            } else {
                Vec::new()
            };
            let convergence_matrix = db::get_convergence_matrix(db_client).await?;
            let policy_scores = db::get_policy_scores(db_client).await?;
            let calibration = db::get_calibration(db_client).await?;
            let trees = db::get_exploitation_trees(db_client, false).await?;
            let all_steps = db::get_all_exploitation_steps(db_client).await?;
            let patterns = db::get_detection_patterns(db_client).await?;
            let enforcement_sources = db::get_enforcement_sources(db_client).await?;

            let threshold = calibration.as_ref().map(|c| c.threshold).unwrap_or(3);

            // Enrich cases with qualities
            let mut enriched_cases = cases.clone();
            let mut case_qualities: std::collections::HashMap<String, Vec<String>> =
                std::collections::HashMap::new();
            for row in &convergence_matrix {
                if row.present {
                    case_qualities
                        .entry(row.case_id.clone())
                        .or_default()
                        .push(row.quality_id.clone());
                }
            }
            for case in &mut enriched_cases {
                if let Some(quals) = case_qualities.get(&case.case_id) {
                    case.qualities = quals.clone();
                    case.qualities.sort();
                }
            }

            // Enrich policies with qualities
            let mut enriched_policies = policies.clone();
            let mut policy_qualities: std::collections::HashMap<String, Vec<String>> =
                std::collections::HashMap::new();
            for row in &policy_scores {
                if row.present {
                    policy_qualities
                        .entry(row.policy_id.clone())
                        .or_default()
                        .push(row.quality_id.clone());
                }
            }
            for policy in &mut enriched_policies {
                if let Some(quals) = policy_qualities.get(&policy.policy_id) {
                    let mut sorted = quals.clone();
                    sorted.sort();
                    let score = sorted.len() as i32;
                    policy.qualities = sorted;
                    policy.convergence_score = Some(score);
                    policy.risk_level = Some(compute_risk_level(score, threshold));
                }
            }

            // Enrich trees with steps
            let mut enriched_trees = trees.clone();
            let mut steps_by_tree: std::collections::HashMap<String, Vec<ExploitationStep>> =
                std::collections::HashMap::new();
            for step in &all_steps {
                steps_by_tree
                    .entry(step.tree_id.clone())
                    .or_default()
                    .push(step.clone());
            }
            for tree in &mut enriched_trees {
                if let Some(steps) = steps_by_tree.remove(&tree.tree_id) {
                    tree.steps = steps;
                }
            }

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
                "cases": enriched_cases,
                "taxonomy": taxonomy,
                "policies": enriched_policies,
                "exploitation_trees": enriched_trees,
                "detection_patterns": patterns,
                "enforcement_sources": enforcement_sources,
            }))
        }

        "GET /api/cases" => {
            let cases = db::get_cases(db_client).await?;
            let matrix = db::get_convergence_matrix(db_client).await?;
            let enriched = enrich_cases(cases, &matrix);
            Ok(json!(enriched))
        }

        "GET /api/taxonomy" => {
            let taxonomy = db::get_taxonomy(db_client).await?;
            Ok(json!(taxonomy))
        }

        "GET /api/policies" => {
            let policies = db::get_policies(db_client).await?;
            let scores = db::get_policy_scores(db_client).await?;
            let calibration = db::get_calibration(db_client).await?;
            let enriched = enrich_policies(policies, &scores, calibration.as_ref());
            Ok(json!(enriched))
        }

        "GET /api/predictions" => {
            let trees = db::get_exploitation_trees(db_client, false).await?;
            let steps = db::get_all_exploitation_steps(db_client).await?;
            let enriched = enrich_trees(trees, steps);
            Ok(json!(enriched))
        }

        "GET /api/detection-patterns" => {
            let patterns = db::get_detection_patterns(db_client).await?;
            Ok(json!(patterns))
        }

        "GET /api/convergence/cases" => {
            let matrix = db::get_convergence_matrix(db_client).await?;
            let calibration = db::get_calibration(db_client).await?;
            Ok(json!({"matrix": matrix, "calibration": calibration}))
        }

        "GET /api/convergence/policies" => {
            let scores = db::get_policy_scores(db_client).await?;
            let calibration = db::get_calibration(db_client).await?;
            Ok(json!({"scores": scores, "calibration": calibration}))
        }

        "GET /api/enforcement-sources" => {
            let sources = db::get_enforcement_sources(db_client).await?;
            Ok(json!(sources))
        }

        "GET /api/dimensions" => {
            let dimensions = db::get_dimensions(db_client).await?;
            Ok(json!(dimensions))
        }

        "GET /api/management/runs" => {
            let runs = db::list_runs(db_client).await?;
            Ok(json!(runs))
        }

        "GET /api/research/triage" => {
            let results = db::get_triage_results(db_client).await?;
            Ok(json!(results))
        }

        "GET /api/research/sessions" => {
            let status = query_param(event, "status");
            let sessions = db::get_research_sessions(db_client, status.as_deref()).await?;
            Ok(json!(sessions))
        }

        "GET /api/discovery/candidates" => {
            let feed_id = query_param(event, "feed_id");
            let status = query_param(event, "status");
            let candidates =
                db::get_candidates(db_client, feed_id.as_deref(), status.as_deref()).await?;
            Ok(json!(candidates))
        }

        "GET /api/discovery/feeds" => {
            let feeds = db::get_source_feeds(db_client, false).await?;
            Ok(json!(feeds))
        }

        "POST /api/pipeline/run" => {
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

            let sfn_arn = env::var("PIPELINE_STATE_MACHINE_ARN").unwrap_or_default();
            if is_lambda && !sfn_arn.is_empty() {
                let sdk_config =
                    aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
                let sfn = aws_sdk_sfn::Client::new(&sdk_config);
                let resp = sfn
                    .start_execution()
                    .state_machine_arn(&sfn_arn)
                    .name(&run_id)
                    .input(serde_json::to_string(&json!({"run_id": run_id}))?)
                    .send()
                    .await?;
                return Ok(json!({
                    "statusCode": 202,
                    "status": "started",
                    "run_id": run_id,
                    "execution_arn": resp.execution_arn(),
                }));
            }

            Ok(json!({"statusCode": 202, "status": "started", "run_id": run_id}))
        }

        "POST /api/pipeline/approve" => {
            let body = json_body(event);
            let stage = body
                .get("stage")
                .and_then(|s| s.as_i64())
                .ok_or_else(|| api_error(400, "Missing stage"))? as i32;
            if stage != 2 && stage != 5 {
                return Err(api_error(
                    400,
                    "Only stages 2 and 5 have human review gates.",
                ));
            }
            let run_id = db::get_latest_run(db_client)
                .await?
                .ok_or_else(|| api_error(404, "No pipeline runs found"))?;
            let status = db::get_stage_status(db_client, &run_id, stage).await?;
            if status.as_deref() != Some("pending_review") {
                return Err(api_error(
                    400,
                    &format!("Stage {} is '{:?}', not pending review.", stage, status),
                ));
            }
            db::approve_stage(db_client, &run_id, stage).await?;
            Ok(json!({"status": "approved", "stage": stage}))
        }

        "POST /api/enforcement-sources" => {
            let body = json_body(event);
            let name = body
                .get("name")
                .and_then(|n| n.as_str())
                .unwrap_or("")
                .trim()
                .to_string();
            if name.is_empty() {
                return Err(api_error(400, "Missing required field: name"));
            }
            let source_id = body
                .get("source_id")
                .and_then(|s| s.as_str())
                .map(String::from)
                .unwrap_or_else(|| {
                    let re = Regex::new(r"[^a-z0-9_]").unwrap();
                    re.replace_all(&name.to_lowercase().replace(' ', "_"), "")[..50.min(name.len())]
                        .to_string()
                });
            if db::get_enforcement_source(db_client, &source_id)
                .await?
                .is_some()
            {
                return Err(api_error(
                    409,
                    &format!("Source '{}' already exists", source_id),
                ));
            }
            let source = EnforcementSource {
                source_id: source_id.clone(),
                name,
                url: body.get("url").and_then(|u| u.as_str()).map(String::from),
                source_type: body
                    .get("source_type")
                    .and_then(|t| t.as_str())
                    .unwrap_or("press_release")
                    .to_string(),
                description: body
                    .get("description")
                    .and_then(|d| d.as_str())
                    .map(String::from),
                has_document: false,
                s3_key: None,
                doc_id: None,
                summary: None,
                validation_status: Some("pending".to_string()),
                created_at: Utc::now().to_rfc3339(),
                updated_at: Utc::now().to_rfc3339(),
                candidate_id: None,
                feed_id: None,
            };
            db::upsert_enforcement_source(db_client, &source).await?;
            let result = db::get_enforcement_source(db_client, &source_id).await?;
            Ok(json!(result))
        }

        "POST /api/enforcement-sources/delete" => {
            let body = json_body(event);
            let source_id = body
                .get("source_id")
                .and_then(|s| s.as_str())
                .ok_or_else(|| api_error(400, "Missing source_id"))?;
            if db::get_enforcement_source(db_client, source_id)
                .await?
                .is_none()
            {
                return Err(api_error(404, &format!("Source '{}' not found", source_id)));
            }
            db::delete_enforcement_source(db_client, source_id).await?;
            Ok(json!({"status": "deleted", "source_id": source_id}))
        }

        "POST /api/discovery/feeds" => {
            let body = json_body(event);
            let name = body
                .get("name")
                .and_then(|n| n.as_str())
                .unwrap_or("")
                .trim()
                .to_string();
            let listing_url = body
                .get("listing_url")
                .and_then(|u| u.as_str())
                .unwrap_or("")
                .trim()
                .to_string();
            if name.is_empty() || listing_url.is_empty() {
                return Err(api_error(400, "Missing required fields: name, listing_url"));
            }
            let re = Regex::new(r"[^a-z0-9_]").unwrap();
            let feed_id = re.replace_all(&name.to_lowercase().replace(' ', "_"), "")
                [..50.min(name.len())]
                .to_string();
            let feed = SourceFeed {
                feed_id: feed_id.clone(),
                name,
                listing_url,
                content_type: body
                    .get("content_type")
                    .and_then(|t| t.as_str())
                    .unwrap_or("press_release")
                    .to_string(),
                link_selector: body
                    .get("link_selector")
                    .and_then(|l| l.as_str())
                    .map(String::from),
                last_checked_at: None,
                last_entry_url: None,
                enabled: Some(true),
                created_at: Utc::now().to_rfc3339(),
                updated_at: Utc::now().to_rfc3339(),
            };
            db::upsert_source_feed(db_client, &feed).await?;
            Ok(json!({"status": "created", "feed_id": feed_id}))
        }

        "POST /api/management/runs/delete" => {
            let body = json_body(event);
            let run_id = body
                .get("run_id")
                .and_then(|r| r.as_str())
                .ok_or_else(|| api_error(400, "Missing run_id"))?
                .trim();
            if run_id.is_empty() {
                return Err(api_error(400, "Missing required field: run_id"));
            }
            db::delete_run(db_client, run_id).await?;
            Ok(json!({"status": "deleted", "run_id": run_id}))
        }

        _ => {
            // Try pattern-matched routes
            if let Some(case_id) = extract_param(path, "/api/cases/") {
                if method == "GET" {
                    let cases = db::get_cases(db_client).await?;
                    let matrix = db::get_convergence_matrix(db_client).await?;
                    let enriched = enrich_cases(cases, &matrix);
                    let case = enriched.into_iter().find(|c| c.case_id == case_id);
                    return case
                        .map(|c| json!(c))
                        .ok_or_else(|| api_error(404, &format!("Case {} not found", case_id)));
                }
            }
            if let Some(quality_id) = extract_param(path, "/api/taxonomy/") {
                if method == "GET" {
                    let taxonomy = db::get_taxonomy(db_client).await?;
                    let q = taxonomy.into_iter().find(|q| q.quality_id == quality_id);
                    return q.map(|q| json!(q)).ok_or_else(|| {
                        api_error(404, &format!("Quality {} not found", quality_id))
                    });
                }
            }
            if let Some(policy_id) = extract_param(path, "/api/policies/") {
                if method == "GET" {
                    let policies = db::get_policies(db_client).await?;
                    let scores = db::get_policy_scores(db_client).await?;
                    let calibration = db::get_calibration(db_client).await?;
                    let enriched = enrich_policies(policies, &scores, calibration.as_ref());
                    let policy = enriched.into_iter().find(|p| p.policy_id == policy_id);
                    return policy
                        .map(|p| json!(p))
                        .ok_or_else(|| api_error(404, &format!("Policy {} not found", policy_id)));
                }
            }
            if let Some(policy_id) = extract_param(path, "/api/research/findings/") {
                if method == "GET" {
                    let findings = db::get_structural_findings(db_client, &policy_id).await?;
                    return Ok(json!({"policy_id": policy_id, "findings": findings}));
                }
            }
            if let Some(policy_id) = extract_param(path, "/api/research/assessments/") {
                if method == "GET" {
                    let assessments =
                        db::get_quality_assessments(db_client, Some(&policy_id)).await?;
                    return Ok(json!({"policy_id": policy_id, "assessments": assessments}));
                }
            }

            Err(api_error(404, &format!("Not found: {}", route_key)))
        }
    }
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
