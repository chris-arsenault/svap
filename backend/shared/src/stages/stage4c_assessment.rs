//! Stage 4C: Quality Assessment from Structural Findings

use serde_json::json;
use sha2::{Digest, Sha256};
use tokio_postgres::Client;
use tracing::info;

use crate::bedrock::BedrockClient;
use crate::db;
use crate::types::{
    Config, Policy, QualityAssessment, ResearchSession, StructuralFinding, TaxonomyQuality,
};

const ASSESSMENT_SYSTEM: &str = "You are assessing whether a structural vulnerability quality is present in a policy based on specific, cited structural findings. Be conservative -- a quality is present only if findings directly support it.";

const STAGE4C_ASSESS_PROMPT: &str = include_str!("../../prompts/stage4c_assess_quality.txt");
type StageResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;
type SessionAssessment = (ResearchSession, Vec<StructuralFinding>, String);

struct AssessmentContext<'a> {
    db_client: &'a Client,
    bedrock: &'a BedrockClient,
    run_id: &'a str,
}

fn compute_hash(parts: &[&str]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(parts.join("|"));
    format!("{:x}", hasher.finalize())[..12].to_string()
}

fn taxonomy_fingerprint(taxonomy: &[TaxonomyQuality]) -> String {
    let mut ids: Vec<&str> = taxonomy.iter().map(|q| q.quality_id.as_str()).collect();
    ids.sort();
    let mut hasher = Sha256::new();
    hasher.update(ids.join(":"));
    format!("{:x}", hasher.finalize())[..12].to_string()
}

pub async fn run(
    db_client: &Client,
    bedrock: &BedrockClient,
    run_id: &str,
    config: &Config,
) -> StageResult<serde_json::Value> {
    info!("Stage 4C: Quality Assessment from Findings");
    db::log_stage_start(db_client, run_id, 42).await?;

    match run_inner(db_client, bedrock, run_id, config).await {
        Ok(result) => {
            db::log_stage_complete(db_client, run_id, 42, Some(&result)).await?;
            Ok(result)
        }
        Err(e) => {
            db::log_stage_failed(db_client, run_id, 42, &e.to_string()).await?;
            Err(e)
        }
    }
}

async fn run_inner(
    db_client: &Client,
    bedrock: &BedrockClient,
    run_id: &str,
    _config: &Config,
) -> StageResult<serde_json::Value> {
    let taxonomy = db::get_approved_taxonomy(db_client).await?;
    if taxonomy.is_empty() {
        return Ok(json!({"policies_assessed": 0}));
    }

    let sessions = assessable_sessions(db_client).await?;

    if sessions.is_empty() {
        return Ok(json!({"policies_assessed": 0}));
    }

    // Delta detection
    let tax_fp = taxonomy_fingerprint(&taxonomy);
    let stored_hashes = db::get_processing_hashes(db_client, 42).await?;
    let (sessions_to_assess, skipped) =
        changed_sessions(db_client, &sessions, &stored_hashes, &tax_fp).await?;

    if sessions_to_assess.is_empty() {
        return Ok(json!({"policies_assessed": 0, "skipped_unchanged": skipped}));
    }

    let all_policies = db::get_policies(db_client).await?;
    let context = AssessmentContext {
        db_client,
        bedrock,
        run_id,
    };
    let assessed = assess_sessions(&context, &taxonomy, &all_policies, &sessions_to_assess).await?;

    info!(
        "Assessment complete: {} policies ({} unchanged).",
        assessed, skipped
    );
    Ok(
        json!({"policies_assessed": assessed, "skipped": skipped, "qualities_per_policy": taxonomy.len()}),
    )
}

async fn assessable_sessions(db_client: &Client) -> StageResult<Vec<ResearchSession>> {
    let sessions_new = db::get_research_sessions(db_client, Some("findings_complete")).await?;
    let sessions_done = db::get_research_sessions(db_client, Some("assessment_complete")).await?;
    let mut seen = std::collections::HashSet::new();
    let mut sessions = Vec::new();
    for session in sessions_new.iter().chain(sessions_done.iter()) {
        if seen.insert(session.policy_id.clone()) {
            sessions.push(session.clone());
        }
    }
    Ok(sessions)
}

async fn changed_sessions(
    db_client: &Client,
    sessions: &[ResearchSession],
    stored_hashes: &std::collections::HashMap<String, String>,
    tax_fp: &str,
) -> StageResult<(Vec<SessionAssessment>, usize)> {
    let mut changed = Vec::new();
    let mut skipped = 0;

    for session in sessions {
        let findings = db::get_structural_findings(db_client, &session.policy_id).await?;
        if findings.is_empty() {
            continue;
        }
        let hash = assessment_hash(&findings, tax_fp);
        if stored_hashes.get(&session.policy_id).map(|s| s.as_str()) == Some(&hash) {
            skipped += 1;
        } else {
            changed.push((session.clone(), findings, hash));
        }
    }

    Ok((changed, skipped))
}

fn assessment_hash(findings: &[StructuralFinding], tax_fp: &str) -> String {
    let mut finding_ids: Vec<&str> = findings.iter().map(|f| f.finding_id.as_str()).collect();
    finding_ids.sort();
    compute_hash(&[&finding_ids.join(":"), tax_fp])
}

async fn assess_sessions(
    context: &AssessmentContext<'_>,
    taxonomy: &[TaxonomyQuality],
    all_policies: &[Policy],
    sessions: &[SessionAssessment],
) -> StageResult<usize> {
    let mut assessed = 0;
    for (session, findings, hash) in sessions {
        assess_session(context, taxonomy, all_policies, session, findings).await?;
        db::record_processing(
            context.db_client,
            42,
            &session.policy_id,
            hash,
            context.run_id,
        )
        .await?;
        assessed += 1;
    }
    Ok(assessed)
}

async fn assess_session(
    context: &AssessmentContext<'_>,
    taxonomy: &[TaxonomyQuality],
    all_policies: &[Policy],
    session: &ResearchSession,
    findings: &[StructuralFinding],
) -> StageResult<()> {
    let policy_name = policy_name(all_policies, session);
    info!("Assessing: {} ({} findings)", policy_name, findings.len());
    let findings_text = findings_text(findings);

    for quality in taxonomy {
        assess_quality(
            context,
            session,
            findings,
            quality,
            &policy_name,
            &findings_text,
        )
        .await?;
    }

    db::update_research_session(
        context.db_client,
        &session.session_id,
        "assessment_complete",
        None,
        None,
    )
    .await?;
    db::update_policy_lifecycle(context.db_client, &session.policy_id, "fully_assessed").await
}

fn policy_name(all_policies: &[Policy], session: &ResearchSession) -> String {
    all_policies
        .iter()
        .find(|p| p.policy_id == session.policy_id)
        .map(|p| p.name.clone())
        .unwrap_or_else(|| session.policy_id.clone())
}

fn findings_text(findings: &[StructuralFinding]) -> String {
    findings
        .iter()
        .map(|f| {
            let dim_name = f
                .dimension_name
                .as_deref()
                .unwrap_or(f.dimension_id.as_deref().unwrap_or("Unknown"));
            format!(
                "[{}] ({}, {} confidence)\n  {}\n  Source: {}",
                f.finding_id,
                dim_name,
                f.confidence,
                f.observation,
                f.source_citation.as_deref().unwrap_or("N/A")
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

async fn assess_quality(
    context: &AssessmentContext<'_>,
    session: &ResearchSession,
    findings: &[StructuralFinding],
    quality: &TaxonomyQuality,
    policy_name: &str,
    findings_text: &str,
) -> StageResult<()> {
    let result = invoke_assessment(context.bedrock, quality, policy_name, findings_text).await?;
    let assessment = quality_assessment(context.run_id, session, findings, quality, &result)?;
    db::upsert_quality_assessment(context.db_client, context.run_id, &assessment).await?;
    db::insert_policy_score(
        context.db_client,
        context.run_id,
        &session.policy_id,
        &quality.quality_id,
        assessment.present == "yes",
        assessment.rationale.as_deref().unwrap_or(""),
    )
    .await
}

async fn invoke_assessment(
    bedrock: &BedrockClient,
    quality: &TaxonomyQuality,
    policy_name: &str,
    findings_text: &str,
) -> StageResult<serde_json::Value> {
    let prompt = BedrockClient::render_prompt(
        STAGE4C_ASSESS_PROMPT,
        &[
            ("quality_id", &quality.quality_id),
            ("quality_name", &quality.name),
            ("quality_definition", &quality.definition),
            ("quality_recognition_test", &quality.recognition_test),
            ("policy_name", policy_name),
            ("findings_text", findings_text),
        ],
    );
    bedrock
        .invoke_json(&prompt, ASSESSMENT_SYSTEM, Some(0.1), Some(1000))
        .await
}

fn quality_assessment(
    run_id: &str,
    session: &ResearchSession,
    findings: &[StructuralFinding],
    quality: &TaxonomyQuality,
    result: &serde_json::Value,
) -> StageResult<QualityAssessment> {
    let present = result
        .get("present")
        .and_then(|p| p.as_str())
        .unwrap_or("uncertain")
        .to_string();
    Ok(QualityAssessment {
        assessment_id: assessment_id(&session.policy_id, &quality.quality_id),
        run_id: run_id.to_string(),
        policy_id: session.policy_id.clone(),
        quality_id: quality.quality_id.clone(),
        taxonomy_version: Some(run_id.to_string()),
        present,
        evidence_finding_ids: Some(serde_json::to_string(&validated_finding_ids(
            result, findings,
        ))?),
        confidence: result
            .get("confidence")
            .and_then(|c| c.as_str())
            .unwrap_or("medium")
            .to_string(),
        rationale: result
            .get("reasoning")
            .and_then(|r| r.as_str())
            .map(String::from),
        created_at: String::new(),
    })
}

fn assessment_id(policy_id: &str, quality_id: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(format!("{}:{}", policy_id, quality_id));
    format!("{:x}", hasher.finalize())[..12].to_string()
}

fn validated_finding_ids(
    result: &serde_json::Value,
    findings: &[StructuralFinding],
) -> Vec<String> {
    let valid_ids: std::collections::HashSet<&str> =
        findings.iter().map(|f| f.finding_id.as_str()).collect();
    result
        .get("finding_ids")
        .and_then(|f| f.as_array())
        .into_iter()
        .flatten()
        .filter_map(|v| v.as_str())
        .filter(|id| valid_ids.contains(*id))
        .map(String::from)
        .collect()
}
