//! Database operations for the SVAP pipeline.
//!
//! All queries use tokio-postgres directly (no ORM). Schema migration runs
//! on first connection via advisory lock, matching the Python storage.py pattern.

use chrono::Utc;
use serde_json::Value;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio_postgres::{Client, NoTls};
use tracing::{info, warn};

use crate::types::*;

static SCHEMA_READY: AtomicBool = AtomicBool::new(false);

fn now() -> String {
    Utc::now().to_rfc3339()
}

/// Connect to PostgreSQL and run migrations if needed.
pub async fn connect(
    database_url: &str,
) -> Result<Client, Box<dyn std::error::Error + Send + Sync>> {
    let (client, connection) = tokio_postgres::connect(database_url, NoTls).await?;

    // Spawn the connection handler
    tokio::spawn(async move {
        if let Err(e) = connection.await {
            tracing::error!("PostgreSQL connection error: {e}");
        }
    });

    if !SCHEMA_READY.load(Ordering::Relaxed) {
        migrate(&client).await?;
        SCHEMA_READY.store(true, Ordering::Relaxed);
    }

    Ok(client)
}

/// Run pending schema migrations inside a transaction with advisory lock.
async fn migrate(client: &Client) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Check/acquire advisory lock
    let row = client
        .query_one("SELECT pg_try_advisory_lock(42)", &[])
        .await?;
    let acquired: bool = row.get(0);

    if !acquired {
        // Check if schema is already at target version
        let result = client
            .query_opt("SELECT version FROM _svap_schema WHERE id = 1", &[])
            .await;
        if let Ok(Some(row)) = result {
            let version: i32 = row.get(0);
            if version >= SCHEMA_VERSION {
                return Ok(());
            }
        }
        info!("Migration: another instance holds the lock, skipping");
        return Ok(());
    }

    // Ensure schema version table exists
    client
        .execute(
            "CREATE TABLE IF NOT EXISTS _svap_schema (
                id INTEGER PRIMARY KEY DEFAULT 1 CHECK(id = 1),
                version INTEGER NOT NULL DEFAULT 0
            )",
            &[],
        )
        .await?;

    client
        .execute(
            "INSERT INTO _svap_schema (id, version) VALUES (1, 0) ON CONFLICT (id) DO NOTHING",
            &[],
        )
        .await?;

    let row = client
        .query_one("SELECT version FROM _svap_schema WHERE id = 1", &[])
        .await?;
    let current: i32 = row.get(0);

    if current >= SCHEMA_VERSION {
        client.execute("SELECT pg_advisory_unlock(42)", &[]).await?;
        return Ok(());
    }

    for (version, statements) in MIGRATIONS {
        if *version <= current {
            continue;
        }
        info!(
            "Migration: applying v{} ({} statements)",
            version,
            statements.len()
        );
        for stmt in *statements {
            if let Err(e) = client.execute(*stmt, &[]).await {
                warn!("Migration statement failed (may be expected for IF NOT EXISTS): {e}");
            }
        }
    }

    client
        .execute(
            "UPDATE _svap_schema SET version = $1 WHERE id = 1",
            &[&SCHEMA_VERSION],
        )
        .await?;

    client.execute("SELECT pg_advisory_unlock(42)", &[]).await?;

    info!("Migration: schema now at v{SCHEMA_VERSION}");
    Ok(())
}

const SCHEMA_VERSION: i32 = 7;

// Migrations are stored as static arrays of SQL statements, matching the Python MIGRATIONS list.
// Only the v1 initial schema is included here; v2-v7 are ALTER migrations that have already
// been applied in production. New Rust-era migrations would start at v8+.
const MIGRATIONS: &[(i32, &[&str])] = &[
    (
        1,
        &[
            "CREATE TABLE IF NOT EXISTS pipeline_runs (
                run_id          TEXT PRIMARY KEY,
                created_at      TEXT NOT NULL,
                config_snapshot TEXT NOT NULL,
                notes           TEXT
            )",
            "CREATE TABLE IF NOT EXISTS stage_log (
                id              SERIAL PRIMARY KEY,
                run_id          TEXT NOT NULL,
                stage           INTEGER NOT NULL,
                status          TEXT NOT NULL CHECK(status IN ('running','completed','failed','pending_review','approved')),
                started_at      TEXT,
                completed_at    TEXT,
                error_message   TEXT,
                metadata        TEXT,
                task_token      TEXT,
                FOREIGN KEY (run_id) REFERENCES pipeline_runs(run_id)
            )",
            "CREATE TABLE IF NOT EXISTS cases (
                case_id             TEXT PRIMARY KEY,
                source_doc_id       TEXT,
                case_name           TEXT NOT NULL,
                scheme_mechanics    TEXT NOT NULL,
                exploited_policy    TEXT NOT NULL,
                enabling_condition  TEXT NOT NULL,
                scale_dollars       REAL,
                scale_defendants    INTEGER,
                scale_duration      TEXT,
                detection_method    TEXT,
                raw_extraction      TEXT,
                created_at          TEXT NOT NULL
            )",
            "CREATE TABLE IF NOT EXISTS taxonomy (
                quality_id          TEXT PRIMARY KEY,
                name                TEXT NOT NULL,
                definition          TEXT NOT NULL,
                recognition_test    TEXT NOT NULL,
                exploitation_logic  TEXT NOT NULL,
                canonical_examples  TEXT,
                review_status       TEXT DEFAULT 'draft' CHECK(review_status IN ('draft','approved','rejected','revised')),
                reviewer_notes      TEXT,
                created_at          TEXT NOT NULL
            )",
            "CREATE TABLE IF NOT EXISTS taxonomy_case_log (
                case_id             TEXT PRIMARY KEY,
                processed_at        TEXT NOT NULL,
                FOREIGN KEY (case_id) REFERENCES cases(case_id)
            )",
            "CREATE TABLE IF NOT EXISTS convergence_scores (
                id                  SERIAL PRIMARY KEY,
                run_id              TEXT NOT NULL,
                case_id             TEXT NOT NULL,
                quality_id          TEXT NOT NULL,
                present             INTEGER NOT NULL CHECK(present IN (0, 1)),
                evidence            TEXT,
                created_at          TEXT NOT NULL,
                FOREIGN KEY (case_id) REFERENCES cases(case_id),
                FOREIGN KEY (quality_id) REFERENCES taxonomy(quality_id)
            )",
            "CREATE TABLE IF NOT EXISTS calibration (
                id                  INTEGER PRIMARY KEY DEFAULT 1 CHECK(id = 1),
                run_id              TEXT,
                threshold           INTEGER NOT NULL,
                correlation_notes   TEXT,
                quality_frequency   TEXT,
                quality_combinations TEXT,
                created_at          TEXT NOT NULL
            )",
            "CREATE TABLE IF NOT EXISTS policies (
                policy_id           TEXT PRIMARY KEY,
                name                TEXT NOT NULL,
                description         TEXT,
                source_document     TEXT,
                structural_characterization TEXT,
                created_at          TEXT NOT NULL,
                lifecycle_status    TEXT DEFAULT 'cataloged',
                lifecycle_updated_at TEXT
            )",
            "CREATE TABLE IF NOT EXISTS policy_scores (
                id                  SERIAL PRIMARY KEY,
                run_id              TEXT NOT NULL,
                policy_id           TEXT NOT NULL,
                quality_id          TEXT NOT NULL,
                present             INTEGER NOT NULL CHECK(present IN (0, 1)),
                evidence            TEXT,
                created_at          TEXT NOT NULL,
                FOREIGN KEY (policy_id) REFERENCES policies(policy_id),
                FOREIGN KEY (quality_id) REFERENCES taxonomy(quality_id)
            )",
            "CREATE TABLE IF NOT EXISTS predictions (
                prediction_id       TEXT PRIMARY KEY,
                run_id              TEXT NOT NULL,
                policy_id           TEXT NOT NULL,
                convergence_score   INTEGER NOT NULL,
                mechanics           TEXT NOT NULL,
                enabling_qualities  TEXT NOT NULL,
                actor_profile       TEXT,
                lifecycle_stage     TEXT,
                detection_difficulty TEXT,
                review_status       TEXT DEFAULT 'draft',
                reviewer_notes      TEXT,
                created_at          TEXT NOT NULL,
                FOREIGN KEY (policy_id) REFERENCES policies(policy_id)
            )",
            "CREATE TABLE IF NOT EXISTS detection_patterns (
                pattern_id          TEXT PRIMARY KEY,
                run_id              TEXT NOT NULL,
                prediction_id       TEXT,
                data_source         TEXT NOT NULL,
                anomaly_signal      TEXT NOT NULL,
                baseline            TEXT,
                false_positive_risk TEXT,
                detection_latency   TEXT,
                priority            TEXT CHECK(priority IN ('critical','high','medium','low')),
                implementation_notes TEXT,
                step_id             TEXT,
                created_at          TEXT NOT NULL
            )",
            "CREATE TABLE IF NOT EXISTS documents (
                doc_id              TEXT PRIMARY KEY,
                filename            TEXT,
                doc_type            TEXT CHECK(doc_type IN ('enforcement','policy','guidance','report','other')),
                full_text           TEXT NOT NULL,
                metadata            TEXT,
                created_at          TEXT NOT NULL
            )",
            "CREATE TABLE IF NOT EXISTS chunks (
                chunk_id            TEXT PRIMARY KEY,
                doc_id              TEXT NOT NULL,
                chunk_index         INTEGER NOT NULL,
                text                TEXT NOT NULL,
                token_count         INTEGER,
                FOREIGN KEY (doc_id) REFERENCES documents(doc_id)
            )",
            "CREATE TABLE IF NOT EXISTS enforcement_sources (
                source_id         TEXT PRIMARY KEY,
                name              TEXT NOT NULL,
                url               TEXT,
                source_type       TEXT NOT NULL DEFAULT 'press_release',
                description       TEXT,
                has_document      BOOLEAN NOT NULL DEFAULT FALSE,
                s3_key            TEXT,
                doc_id            TEXT,
                summary           TEXT,
                validation_status TEXT DEFAULT 'pending'
                    CHECK(validation_status IN ('pending','valid','invalid','error')),
                created_at        TEXT NOT NULL,
                updated_at        TEXT NOT NULL,
                candidate_id      TEXT,
                feed_id           TEXT
            )",
            "CREATE UNIQUE INDEX IF NOT EXISTS uq_convergence ON convergence_scores(case_id, quality_id)",
            "CREATE UNIQUE INDEX IF NOT EXISTS uq_policy_score ON policy_scores(policy_id, quality_id)",
            "CREATE UNIQUE INDEX IF NOT EXISTS uq_enforcement_source_url ON enforcement_sources(url) WHERE url IS NOT NULL",
            "CREATE TABLE IF NOT EXISTS dimension_registry (
                dimension_id        TEXT PRIMARY KEY,
                name                TEXT NOT NULL,
                definition          TEXT NOT NULL,
                probing_questions   TEXT,
                origin              TEXT NOT NULL CHECK(origin IN ('case_derived','policy_derived','manual','seed')),
                related_quality_ids TEXT,
                created_at          TEXT NOT NULL,
                created_by          TEXT
            )",
            "CREATE TABLE IF NOT EXISTS structural_findings (
                finding_id          TEXT PRIMARY KEY,
                run_id              TEXT NOT NULL,
                policy_id           TEXT NOT NULL,
                dimension_id        TEXT,
                observation         TEXT NOT NULL,
                source_type         TEXT NOT NULL DEFAULT 'llm_knowledge',
                source_citation     TEXT,
                source_text         TEXT,
                confidence          TEXT NOT NULL DEFAULT 'medium'
                    CHECK(confidence IN ('high','medium','low')),
                status              TEXT NOT NULL DEFAULT 'active'
                    CHECK(status IN ('active','stale','superseded')),
                stale_reason        TEXT,
                created_at          TEXT NOT NULL,
                created_by          TEXT,
                FOREIGN KEY (policy_id) REFERENCES policies(policy_id),
                FOREIGN KEY (dimension_id) REFERENCES dimension_registry(dimension_id)
            )",
            "CREATE TABLE IF NOT EXISTS quality_assessments (
                assessment_id       TEXT PRIMARY KEY,
                run_id              TEXT NOT NULL,
                policy_id           TEXT NOT NULL,
                quality_id          TEXT NOT NULL,
                taxonomy_version    TEXT,
                present             TEXT NOT NULL DEFAULT 'uncertain'
                    CHECK(present IN ('yes','no','uncertain')),
                evidence_finding_ids TEXT,
                confidence          TEXT NOT NULL DEFAULT 'medium'
                    CHECK(confidence IN ('high','medium','low')),
                rationale           TEXT,
                created_at          TEXT NOT NULL,
                FOREIGN KEY (policy_id) REFERENCES policies(policy_id),
                FOREIGN KEY (quality_id) REFERENCES taxonomy(quality_id)
            )",
            "CREATE UNIQUE INDEX IF NOT EXISTS uq_quality_assessment ON quality_assessments(policy_id, quality_id)",
            "CREATE TABLE IF NOT EXISTS source_feeds (
                feed_id             TEXT PRIMARY KEY,
                name                TEXT NOT NULL,
                listing_url         TEXT NOT NULL UNIQUE,
                content_type        TEXT NOT NULL DEFAULT 'press_release',
                link_selector       TEXT,
                last_checked_at     TEXT,
                last_entry_url      TEXT,
                enabled             BOOLEAN DEFAULT TRUE,
                created_at          TEXT NOT NULL,
                updated_at          TEXT NOT NULL
            )",
            "CREATE TABLE IF NOT EXISTS source_candidates (
                candidate_id        TEXT PRIMARY KEY,
                feed_id             TEXT,
                title               TEXT NOT NULL,
                url                 TEXT NOT NULL UNIQUE,
                discovered_at       TEXT NOT NULL,
                published_date      TEXT,
                status              TEXT NOT NULL DEFAULT 'discovered'
                    CHECK(status IN ('discovered','fetched','scored','accepted','rejected','ingested','error')),
                richness_score      REAL,
                richness_rationale  TEXT,
                estimated_cases     INTEGER,
                source_id           TEXT,
                doc_id              TEXT,
                reviewed_by         TEXT DEFAULT 'auto',
                created_at          TEXT NOT NULL,
                updated_at          TEXT NOT NULL,
                FOREIGN KEY (feed_id) REFERENCES source_feeds(feed_id)
            )",
            "CREATE TABLE IF NOT EXISTS triage_results (
                id                  SERIAL PRIMARY KEY,
                run_id              TEXT NOT NULL,
                policy_id           TEXT NOT NULL,
                triage_score        REAL NOT NULL,
                rationale           TEXT NOT NULL,
                uncertainty         TEXT,
                priority_rank       INTEGER NOT NULL,
                created_at          TEXT NOT NULL,
                FOREIGN KEY (policy_id) REFERENCES policies(policy_id)
            )",
            "CREATE UNIQUE INDEX IF NOT EXISTS uq_triage ON triage_results(policy_id)",
            "CREATE TABLE IF NOT EXISTS research_sessions (
                session_id          TEXT PRIMARY KEY,
                run_id              TEXT NOT NULL,
                policy_id           TEXT NOT NULL,
                status              TEXT NOT NULL DEFAULT 'pending'
                    CHECK(status IN ('pending','researching','findings_complete','assessment_complete','failed')),
                sources_queried     TEXT,
                started_at          TEXT,
                completed_at        TEXT,
                error_message       TEXT,
                trigger             TEXT DEFAULT 'initial'
                    CHECK(trigger IN ('initial','taxonomy_change','regulatory_change','manual')),
                FOREIGN KEY (policy_id) REFERENCES policies(policy_id)
            )",
            "CREATE TABLE IF NOT EXISTS regulatory_sources (
                source_id           TEXT PRIMARY KEY,
                source_type         TEXT NOT NULL,
                url                 TEXT NOT NULL,
                title               TEXT,
                cfr_reference       TEXT,
                full_text           TEXT NOT NULL,
                fetched_at          TEXT NOT NULL,
                metadata            TEXT
            )",
            "CREATE TABLE IF NOT EXISTS stage_processing_log (
                stage        INTEGER NOT NULL,
                entity_id    TEXT NOT NULL,
                input_hash   TEXT NOT NULL,
                run_id       TEXT,
                processed_at TEXT NOT NULL,
                PRIMARY KEY (stage, entity_id)
            )",
            "CREATE TABLE IF NOT EXISTS prediction_qualities (
                prediction_id  TEXT NOT NULL,
                quality_id     TEXT NOT NULL,
                PRIMARY KEY (prediction_id, quality_id)
            )",
            "CREATE TABLE IF NOT EXISTS assessment_findings (
                assessment_id  TEXT NOT NULL,
                finding_id     TEXT NOT NULL,
                PRIMARY KEY (assessment_id, finding_id)
            )",
            "CREATE TABLE IF NOT EXISTS exploitation_trees (
                tree_id             TEXT PRIMARY KEY,
                policy_id           TEXT NOT NULL UNIQUE,
                convergence_score   INTEGER NOT NULL,
                actor_profile       TEXT,
                lifecycle_stage     TEXT,
                detection_difficulty TEXT,
                review_status       TEXT DEFAULT 'draft'
                    CHECK(review_status IN ('draft','approved','rejected','revised')),
                reviewer_notes      TEXT,
                run_id              TEXT,
                created_at          TEXT NOT NULL,
                FOREIGN KEY (policy_id) REFERENCES policies(policy_id)
            )",
            "CREATE TABLE IF NOT EXISTS exploitation_steps (
                step_id             TEXT PRIMARY KEY,
                tree_id             TEXT NOT NULL,
                parent_step_id      TEXT,
                step_order          INTEGER NOT NULL,
                title               TEXT NOT NULL,
                description         TEXT NOT NULL,
                actor_action        TEXT,
                is_branch_point     BOOLEAN DEFAULT FALSE,
                branch_label        TEXT,
                created_at          TEXT NOT NULL,
                FOREIGN KEY (tree_id) REFERENCES exploitation_trees(tree_id) ON DELETE CASCADE,
                FOREIGN KEY (parent_step_id) REFERENCES exploitation_steps(step_id) ON DELETE CASCADE
            )",
            "CREATE INDEX IF NOT EXISTS idx_steps_tree ON exploitation_steps(tree_id)",
            "CREATE INDEX IF NOT EXISTS idx_steps_parent ON exploitation_steps(parent_step_id)",
            "CREATE TABLE IF NOT EXISTS step_qualities (
                step_id     TEXT NOT NULL,
                quality_id  TEXT NOT NULL,
                PRIMARY KEY (step_id, quality_id)
            )",
            "CREATE INDEX IF NOT EXISTS idx_patterns_step ON detection_patterns(step_id)",
        ],
    ),
    // v2-v7 are migration-only (ALTER TABLE, data migration, etc.)
    // They have already been applied in production. For new deployments,
    // the v1 schema above includes all columns in their final form.
    // Any future migrations start at v8+.
    (2, &[]),
    (3, &[]),
    (4, &[]),
    (5, &[]),
    (6, &[]),
    (7, &[]),
];

// ── Helper to extract optional String from a row ─────────────────────────

fn opt_str(row: &tokio_postgres::Row, col: &str) -> Option<String> {
    row.try_get::<_, String>(col).ok()
}

fn opt_f64(row: &tokio_postgres::Row, col: &str) -> Option<f64> {
    // scale_dollars is REAL in PG, which maps to f32
    row.try_get::<_, f32>(col).ok().map(|v| v as f64)
}

fn opt_i32(row: &tokio_postgres::Row, col: &str) -> Option<i32> {
    row.try_get::<_, i32>(col).ok()
}

fn opt_i64(row: &tokio_postgres::Row, col: &str) -> Option<i64> {
    row.try_get::<_, i64>(col).ok()
}

fn opt_bool(row: &tokio_postgres::Row, col: &str) -> Option<bool> {
    row.try_get::<_, bool>(col).ok()
}

fn get_bool(row: &tokio_postgres::Row, col: &str) -> bool {
    // present is stored as INTEGER 0/1 in PG
    row.try_get::<_, i32>(col).map(|v| v != 0).unwrap_or(false)
}

// ── Run Management ───────────────────────────────────────────────────────

pub async fn create_run(
    client: &Client,
    run_id: &str,
    config: &serde_json::Value,
    notes: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    client
        .execute(
            "INSERT INTO pipeline_runs (run_id, created_at, config_snapshot, notes) VALUES ($1, $2, $3, $4)",
            &[&run_id, &now(), &serde_json::to_string(config)?, &notes],
        )
        .await?;
    Ok(())
}

pub async fn get_latest_run(
    client: &Client,
) -> Result<Option<String>, Box<dyn std::error::Error + Send + Sync>> {
    let row = client
        .query_opt(
            "SELECT run_id FROM pipeline_runs ORDER BY created_at DESC LIMIT 1",
            &[],
        )
        .await?;
    Ok(row.map(|r| r.get::<_, String>(0)))
}

pub async fn list_runs(
    client: &Client,
) -> Result<Vec<RunSummary>, Box<dyn std::error::Error + Send + Sync>> {
    let rows = client
        .query(
            "SELECT run_id, created_at, notes FROM pipeline_runs ORDER BY created_at DESC",
            &[],
        )
        .await?;

    let mut runs = Vec::new();
    for row in &rows {
        let run_id: String = row.get(0);
        let stage_rows = client
            .query(
                "SELECT DISTINCT ON (stage) stage, status FROM stage_log WHERE run_id = $1 ORDER BY stage, id DESC",
                &[&run_id],
            )
            .await?;

        let stages = stage_rows
            .iter()
            .map(|sr| StageStatusEntry {
                stage: sr.get(0),
                status: sr.get(1),
                started_at: None,
                completed_at: None,
                error_message: None,
            })
            .collect();

        runs.push(RunSummary {
            run_id,
            created_at: row.get(1),
            notes: opt_str(row, "notes"),
            stages,
        });
    }
    Ok(runs)
}

pub async fn delete_run(
    client: &Client,
    run_id: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    client.execute("BEGIN", &[]).await?;
    let result = async {
        client
            .execute("DELETE FROM stage_log WHERE run_id = $1", &[&run_id])
            .await?;
        client
            .execute("DELETE FROM pipeline_runs WHERE run_id = $1", &[&run_id])
            .await?;
        Ok::<_, Box<dyn std::error::Error + Send + Sync>>(())
    }
    .await;
    match result {
        Ok(()) => {
            client.execute("COMMIT", &[]).await?;
            Ok(())
        }
        Err(e) => {
            let _ = client.execute("ROLLBACK", &[]).await;
            Err(e)
        }
    }
}

// ── Stage Log ────────────────────────────────────────────────────────────

pub async fn log_stage_start(
    client: &Client,
    run_id: &str,
    stage: i32,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    client
        .execute(
            "INSERT INTO stage_log (run_id, stage, status, started_at) VALUES ($1, $2, 'running', $3)",
            &[&run_id, &stage, &now()],
        )
        .await?;
    Ok(())
}

pub async fn log_stage_complete(
    client: &Client,
    run_id: &str,
    stage: i32,
    metadata: Option<&serde_json::Value>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let meta_str = metadata.map(|m| serde_json::to_string(m).unwrap_or_default());
    client
        .execute(
            "UPDATE stage_log SET status='completed', completed_at=$1, metadata=$2 WHERE run_id=$3 AND stage=$4 AND status='running'",
            &[&now(), &meta_str, &run_id, &stage],
        )
        .await?;
    Ok(())
}

pub async fn log_stage_failed(
    client: &Client,
    run_id: &str,
    stage: i32,
    error: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    client
        .execute(
            "UPDATE stage_log SET status='failed', completed_at=$1, error_message=$2 WHERE run_id=$3 AND stage=$4 AND status='running'",
            &[&now(), &error, &run_id, &stage],
        )
        .await?;
    Ok(())
}

pub async fn log_stage_pending_review(
    client: &Client,
    run_id: &str,
    stage: i32,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    client
        .execute(
            "UPDATE stage_log SET status='pending_review', completed_at=$1 WHERE run_id=$2 AND stage=$3 AND status='running'",
            &[&now(), &run_id, &stage],
        )
        .await?;
    Ok(())
}

pub async fn approve_stage(
    client: &Client,
    run_id: &str,
    stage: i32,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    client
        .execute(
            "UPDATE stage_log SET status='approved' WHERE run_id=$1 AND stage=$2 AND status='pending_review'",
            &[&run_id, &stage],
        )
        .await?;
    // Stage 5 approval also marks exploitation trees as approved
    if stage == 5 {
        client
            .execute(
                "UPDATE exploitation_trees SET review_status='approved' WHERE review_status='draft'",
                &[],
            )
            .await?;
    }
    Ok(())
}

pub async fn get_stage_status(
    client: &Client,
    run_id: &str,
    stage: i32,
) -> Result<Option<String>, Box<dyn std::error::Error + Send + Sync>> {
    let row = client
        .query_opt(
            "SELECT status FROM stage_log WHERE run_id=$1 AND stage=$2 ORDER BY id DESC LIMIT 1",
            &[&run_id, &stage],
        )
        .await?;
    Ok(row.map(|r| r.get::<_, String>(0)))
}

pub async fn get_pipeline_status(
    client: &Client,
    run_id: &str,
) -> Result<Vec<StageStatusEntry>, Box<dyn std::error::Error + Send + Sync>> {
    let rows = client
        .query(
            "SELECT DISTINCT ON (stage) stage, status, started_at, completed_at, error_message FROM stage_log WHERE run_id=$1 ORDER BY stage, id DESC",
            &[&run_id],
        )
        .await?;

    Ok(rows
        .iter()
        .map(|r| StageStatusEntry {
            stage: r.get(0),
            status: r.get(1),
            started_at: opt_str(r, "started_at"),
            completed_at: opt_str(r, "completed_at"),
            error_message: opt_str(r, "error_message"),
        })
        .collect())
}

pub async fn get_corpus_counts(
    client: &Client,
) -> Result<CorpusCounts, Box<dyn std::error::Error + Send + Sync>> {
    let row = client
        .query_one(
            "SELECT
                (SELECT COUNT(*) FROM cases) AS cases,
                (SELECT COUNT(*) FROM taxonomy) AS taxonomy_qualities,
                (SELECT COUNT(*) FROM policies) AS policies,
                (SELECT COUNT(*) FROM exploitation_trees) AS exploitation_trees,
                (SELECT COUNT(*) FROM detection_patterns) AS detection_patterns",
            &[],
        )
        .await?;
    Ok(CorpusCounts {
        cases: row.get(0),
        taxonomy_qualities: row.get(1),
        policies: row.get(2),
        exploitation_trees: row.get(3),
        detection_patterns: row.get(4),
    })
}

// ── Processing Log (Delta Detection) ─────────────────────────────────────

pub async fn get_processing_hashes(
    client: &Client,
    stage: i32,
) -> Result<std::collections::HashMap<String, String>, Box<dyn std::error::Error + Send + Sync>> {
    let rows = client
        .query(
            "SELECT entity_id, input_hash FROM stage_processing_log WHERE stage = $1",
            &[&stage],
        )
        .await?;
    Ok(rows
        .iter()
        .map(|r| (r.get::<_, String>(0), r.get::<_, String>(1)))
        .collect())
}

pub async fn record_processing(
    client: &Client,
    stage: i32,
    entity_id: &str,
    input_hash: &str,
    run_id: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    client
        .execute(
            "INSERT INTO stage_processing_log (stage, entity_id, input_hash, run_id, processed_at)
             VALUES ($1, $2, $3, $4, $5)
             ON CONFLICT (stage, entity_id) DO UPDATE SET
                 input_hash = EXCLUDED.input_hash,
                 run_id = EXCLUDED.run_id,
                 processed_at = EXCLUDED.processed_at",
            &[&stage, &entity_id, &input_hash, &run_id, &now()],
        )
        .await?;
    Ok(())
}

// ── Cases ────────────────────────────────────────────────────────────────

pub async fn insert_case(
    client: &Client,
    case: &Case,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let raw = case
        .raw_extraction
        .as_ref()
        .map(|v| serde_json::to_string(v).unwrap_or_default());
    let scale_f32 = case.scale_dollars.map(|v| v as f32);
    client
        .execute(
            "INSERT INTO cases
            (case_id, source_doc_id, case_name, scheme_mechanics,
             exploited_policy, enabling_condition, scale_dollars, scale_defendants,
             scale_duration, detection_method, raw_extraction, created_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
            ON CONFLICT (case_id) DO UPDATE SET
                source_doc_id = EXCLUDED.source_doc_id,
                case_name = EXCLUDED.case_name,
                scheme_mechanics = EXCLUDED.scheme_mechanics,
                exploited_policy = EXCLUDED.exploited_policy,
                enabling_condition = EXCLUDED.enabling_condition,
                scale_dollars = EXCLUDED.scale_dollars,
                scale_defendants = EXCLUDED.scale_defendants,
                scale_duration = EXCLUDED.scale_duration,
                detection_method = EXCLUDED.detection_method,
                raw_extraction = EXCLUDED.raw_extraction,
                created_at = EXCLUDED.created_at",
            &[
                &case.case_id,
                &case.source_doc_id,
                &case.case_name,
                &case.scheme_mechanics,
                &case.exploited_policy,
                &case.enabling_condition,
                &scale_f32,
                &case.scale_defendants,
                &case.scale_duration,
                &case.detection_method,
                &raw,
                &now(),
            ],
        )
        .await?;
    Ok(())
}

pub async fn cases_exist_for_document(
    client: &Client,
    doc_id: &str,
) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    let row = client
        .query_opt(
            "SELECT 1 FROM cases WHERE source_doc_id = $1 LIMIT 1",
            &[&doc_id],
        )
        .await?;
    Ok(row.is_some())
}

pub async fn get_cases(
    client: &Client,
) -> Result<Vec<Case>, Box<dyn std::error::Error + Send + Sync>> {
    let rows = client.query("SELECT * FROM cases", &[]).await?;
    Ok(rows
        .iter()
        .map(|r| Case {
            case_id: r.get("case_id"),
            source_doc_id: opt_str(r, "source_doc_id"),
            case_name: r.get("case_name"),
            scheme_mechanics: r.get("scheme_mechanics"),
            exploited_policy: r.get("exploited_policy"),
            enabling_condition: r.get("enabling_condition"),
            scale_dollars: opt_f64(r, "scale_dollars"),
            scale_defendants: opt_i32(r, "scale_defendants"),
            scale_duration: opt_str(r, "scale_duration"),
            detection_method: opt_str(r, "detection_method"),
            raw_extraction: opt_str(r, "raw_extraction")
                .and_then(|s| serde_json::from_str(&s).ok()),
            created_at: r.get("created_at"),
            qualities: Vec::new(),
        })
        .collect())
}

// ── Taxonomy ─────────────────────────────────────────────────────────────

pub async fn insert_quality(
    client: &Client,
    q: &TaxonomyQuality,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let examples = q
        .canonical_examples
        .as_ref()
        .map(|v| serde_json::to_string(v).unwrap_or_else(|_| "[]".to_string()));
    client
        .execute(
            "INSERT INTO taxonomy
            (quality_id, name, definition, recognition_test,
             exploitation_logic, canonical_examples, review_status, created_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            ON CONFLICT (quality_id) DO UPDATE SET
                name = EXCLUDED.name,
                definition = EXCLUDED.definition,
                recognition_test = EXCLUDED.recognition_test,
                exploitation_logic = EXCLUDED.exploitation_logic,
                canonical_examples = EXCLUDED.canonical_examples,
                review_status = EXCLUDED.review_status,
                created_at = EXCLUDED.created_at",
            &[
                &q.quality_id,
                &q.name,
                &q.definition,
                &q.recognition_test,
                &q.exploitation_logic,
                &examples,
                &q.review_status.as_deref().unwrap_or("draft"),
                &now(),
            ],
        )
        .await?;
    Ok(())
}

pub async fn get_taxonomy(
    client: &Client,
) -> Result<Vec<TaxonomyQuality>, Box<dyn std::error::Error + Send + Sync>> {
    let rows = client
        .query("SELECT * FROM taxonomy ORDER BY quality_id", &[])
        .await?;
    Ok(rows.iter().map(row_to_quality).collect())
}

pub async fn get_approved_taxonomy(
    client: &Client,
) -> Result<Vec<TaxonomyQuality>, Box<dyn std::error::Error + Send + Sync>> {
    let rows = client
        .query(
            "SELECT * FROM taxonomy WHERE review_status = 'approved' ORDER BY quality_id",
            &[],
        )
        .await?;
    Ok(rows.iter().map(row_to_quality).collect())
}

fn row_to_quality(r: &tokio_postgres::Row) -> TaxonomyQuality {
    let examples_str = opt_str(r, "canonical_examples");
    let canonical_examples = examples_str.and_then(|s| serde_json::from_str(&s).ok());
    TaxonomyQuality {
        quality_id: r.get("quality_id"),
        name: r.get("name"),
        definition: r.get("definition"),
        recognition_test: r.get("recognition_test"),
        exploitation_logic: r.get("exploitation_logic"),
        canonical_examples,
        review_status: opt_str(r, "review_status"),
        reviewer_notes: opt_str(r, "reviewer_notes"),
        created_at: r.get("created_at"),
        color: None,
        case_count: None,
    }
}

pub async fn get_taxonomy_processed_case_ids(
    client: &Client,
) -> Result<std::collections::HashSet<String>, Box<dyn std::error::Error + Send + Sync>> {
    let rows = client
        .query("SELECT case_id FROM taxonomy_case_log", &[])
        .await?;
    Ok(rows.iter().map(|r| r.get::<_, String>(0)).collect())
}

pub async fn record_taxonomy_case_processed(
    client: &Client,
    case_id: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    client
        .execute(
            "INSERT INTO taxonomy_case_log (case_id, processed_at) VALUES ($1, $2) ON CONFLICT (case_id) DO NOTHING",
            &[&case_id, &now()],
        )
        .await?;
    Ok(())
}

pub async fn merge_quality_examples(
    client: &Client,
    quality_id: &str,
    new_examples: &[String],
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let row = client
        .query_opt(
            "SELECT canonical_examples FROM taxonomy WHERE quality_id = $1",
            &[&quality_id],
        )
        .await?;
    if let Some(row) = row {
        let existing_str: Option<String> = row.get(0);
        let mut existing: Vec<String> = existing_str
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        for ex in new_examples {
            if !existing.contains(ex) {
                existing.push(ex.clone());
            }
        }
        client
            .execute(
                "UPDATE taxonomy SET canonical_examples = $1 WHERE quality_id = $2",
                &[&serde_json::to_string(&existing)?, &quality_id],
            )
            .await?;
    }
    Ok(())
}

// ── Convergence Scores ───────────────────────────────────────────────────

pub async fn insert_convergence_score(
    client: &Client,
    run_id: &str,
    case_id: &str,
    quality_id: &str,
    present: bool,
    evidence: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let present_i = if present { 1i32 } else { 0i32 };
    client
        .execute(
            "INSERT INTO convergence_scores
            (run_id, case_id, quality_id, present, evidence, created_at)
            VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (case_id, quality_id) DO UPDATE SET
                run_id = EXCLUDED.run_id,
                present = EXCLUDED.present,
                evidence = EXCLUDED.evidence,
                created_at = EXCLUDED.created_at",
            &[
                &run_id,
                &case_id,
                &quality_id,
                &present_i,
                &evidence,
                &now(),
            ],
        )
        .await?;
    Ok(())
}

pub async fn get_convergence_matrix(
    client: &Client,
) -> Result<Vec<ConvergenceRow>, Box<dyn std::error::Error + Send + Sync>> {
    let rows = client
        .query(
            "SELECT c.case_name, c.case_id, c.scale_dollars,
                    cs.quality_id, cs.present, cs.evidence
             FROM convergence_scores cs
             JOIN cases c ON cs.case_id = c.case_id
             ORDER BY c.case_id, cs.quality_id",
            &[],
        )
        .await?;
    Ok(rows
        .iter()
        .map(|r| ConvergenceRow {
            case_name: r.get("case_name"),
            case_id: r.get("case_id"),
            scale_dollars: opt_f64(r, "scale_dollars"),
            quality_id: r.get("quality_id"),
            present: get_bool(r, "present"),
            evidence: opt_str(r, "evidence"),
        })
        .collect())
}

pub async fn insert_calibration(
    client: &Client,
    run_id: &str,
    threshold: i32,
    notes: &str,
    freq: &serde_json::Value,
    combos: &serde_json::Value,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let freq_s = serde_json::to_string(freq)?;
    let combos_s = serde_json::to_string(combos)?;
    client
        .execute(
            "INSERT INTO calibration
            (id, run_id, threshold, correlation_notes, quality_frequency, quality_combinations, created_at)
            VALUES (1, $1, $2, $3, $4, $5, $6)
            ON CONFLICT (id) DO UPDATE SET
                run_id = EXCLUDED.run_id,
                threshold = EXCLUDED.threshold,
                correlation_notes = EXCLUDED.correlation_notes,
                quality_frequency = EXCLUDED.quality_frequency,
                quality_combinations = EXCLUDED.quality_combinations,
                created_at = EXCLUDED.created_at",
            &[&run_id, &threshold, &notes, &freq_s, &combos_s, &now()],
        )
        .await?;
    Ok(())
}

pub async fn get_calibration(
    client: &Client,
) -> Result<Option<Calibration>, Box<dyn std::error::Error + Send + Sync>> {
    let row = client
        .query_opt("SELECT * FROM calibration WHERE id = 1", &[])
        .await?;
    Ok(row.map(|r| Calibration {
        run_id: opt_str(&r, "run_id"),
        threshold: r.get("threshold"),
        correlation_notes: opt_str(&r, "correlation_notes"),
        quality_frequency: opt_str(&r, "quality_frequency"),
        quality_combinations: opt_str(&r, "quality_combinations"),
        created_at: r.get("created_at"),
    }))
}

// ── Policies ─────────────────────────────────────────────────────────────

pub async fn insert_policy(
    client: &Client,
    policy: &Policy,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    client
        .execute(
            "INSERT INTO policies
            (policy_id, name, description, source_document,
             structural_characterization, created_at)
            VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (policy_id) DO UPDATE SET
                name = EXCLUDED.name,
                description = EXCLUDED.description,
                source_document = EXCLUDED.source_document,
                structural_characterization = EXCLUDED.structural_characterization,
                created_at = EXCLUDED.created_at",
            &[
                &policy.policy_id,
                &policy.name,
                &policy.description,
                &policy.source_document,
                &policy.structural_characterization,
                &now(),
            ],
        )
        .await?;
    Ok(())
}

pub async fn get_policies(
    client: &Client,
) -> Result<Vec<Policy>, Box<dyn std::error::Error + Send + Sync>> {
    let rows = client.query("SELECT * FROM policies", &[]).await?;
    Ok(rows.iter().map(row_to_policy).collect())
}

fn row_to_policy(r: &tokio_postgres::Row) -> Policy {
    Policy {
        policy_id: r.get("policy_id"),
        name: r.get("name"),
        description: opt_str(r, "description"),
        source_document: opt_str(r, "source_document"),
        structural_characterization: opt_str(r, "structural_characterization"),
        created_at: r.get("created_at"),
        lifecycle_status: opt_str(r, "lifecycle_status"),
        lifecycle_updated_at: opt_str(r, "lifecycle_updated_at"),
        qualities: Vec::new(),
        convergence_score: None,
        risk_level: None,
    }
}

pub async fn insert_policy_score(
    client: &Client,
    run_id: &str,
    policy_id: &str,
    quality_id: &str,
    present: bool,
    evidence: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let present_i = if present { 1i32 } else { 0i32 };
    client
        .execute(
            "INSERT INTO policy_scores
            (run_id, policy_id, quality_id, present, evidence, created_at)
            VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (policy_id, quality_id) DO UPDATE SET
                run_id = EXCLUDED.run_id,
                present = EXCLUDED.present,
                evidence = EXCLUDED.evidence,
                created_at = EXCLUDED.created_at",
            &[
                &run_id,
                &policy_id,
                &quality_id,
                &present_i,
                &evidence,
                &now(),
            ],
        )
        .await?;
    Ok(())
}

pub async fn get_policy_scores(
    client: &Client,
) -> Result<Vec<PolicyScore>, Box<dyn std::error::Error + Send + Sync>> {
    let rows = client
        .query(
            "SELECT p.name, p.policy_id, ps.quality_id, ps.present, ps.evidence
             FROM policy_scores ps
             JOIN policies p ON ps.policy_id = p.policy_id
             ORDER BY p.policy_id, ps.quality_id",
            &[],
        )
        .await?;
    Ok(rows
        .iter()
        .map(|r| PolicyScore {
            name: r.get("name"),
            policy_id: r.get("policy_id"),
            quality_id: r.get("quality_id"),
            present: get_bool(r, "present"),
            evidence: opt_str(r, "evidence"),
        })
        .collect())
}

// ── Exploitation Trees ───────────────────────────────────────────────────

pub async fn insert_exploitation_tree(
    client: &Client,
    run_id: &str,
    tree: &ExploitationTree,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    client
        .execute(
            "INSERT INTO exploitation_trees
            (tree_id, policy_id, convergence_score, actor_profile,
             lifecycle_stage, detection_difficulty, review_status, run_id, created_at)
            VALUES ($1, $2, $3, $4, $5, $6, 'draft', $7, $8)
            ON CONFLICT (tree_id) DO UPDATE SET
                convergence_score = EXCLUDED.convergence_score,
                actor_profile = EXCLUDED.actor_profile,
                lifecycle_stage = EXCLUDED.lifecycle_stage,
                detection_difficulty = EXCLUDED.detection_difficulty,
                review_status = EXCLUDED.review_status,
                run_id = EXCLUDED.run_id,
                created_at = EXCLUDED.created_at",
            &[
                &tree.tree_id,
                &tree.policy_id,
                &tree.convergence_score,
                &tree.actor_profile,
                &tree.lifecycle_stage,
                &tree.detection_difficulty,
                &run_id,
                &now(),
            ],
        )
        .await?;
    Ok(())
}

pub async fn insert_exploitation_step(
    client: &Client,
    step: &ExploitationStep,
    quality_ids: &[String],
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    client.execute("BEGIN", &[]).await?;
    let result = async {
        client.execute(
            "INSERT INTO exploitation_steps
            (step_id, tree_id, parent_step_id, step_order, title,
             description, actor_action, is_branch_point, branch_label, created_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            ON CONFLICT (step_id) DO UPDATE SET
                tree_id = EXCLUDED.tree_id,
                parent_step_id = EXCLUDED.parent_step_id,
                step_order = EXCLUDED.step_order,
                title = EXCLUDED.title,
                description = EXCLUDED.description,
                actor_action = EXCLUDED.actor_action,
                is_branch_point = EXCLUDED.is_branch_point,
                branch_label = EXCLUDED.branch_label,
                created_at = EXCLUDED.created_at",
            &[
                &step.step_id,
                &step.tree_id,
                &step.parent_step_id,
                &step.step_order,
                &step.title,
                &step.description,
                &step.actor_action,
                &step.is_branch_point.unwrap_or(false),
                &step.branch_label,
                &now(),
            ],
        )
        .await?;
        for qid in quality_ids {
            client.execute(
                "INSERT INTO step_qualities (step_id, quality_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
                &[&step.step_id, qid],
            )
            .await?;
        }
        Ok::<_, Box<dyn std::error::Error + Send + Sync>>(())
    }
    .await;
    match result {
        Ok(()) => {
            client.execute("COMMIT", &[]).await?;
            Ok(())
        }
        Err(e) => {
            let _ = client.execute("ROLLBACK", &[]).await;
            Err(e)
        }
    }
}

pub async fn get_exploitation_trees(
    client: &Client,
    approved_only: bool,
) -> Result<Vec<ExploitationTree>, Box<dyn std::error::Error + Send + Sync>> {
    let where_clause = if approved_only {
        "WHERE et.review_status = 'approved'"
    } else {
        ""
    };
    let query = format!(
        "SELECT et.*, p.name as policy_name,
                (SELECT count(*) FROM exploitation_steps es WHERE es.tree_id = et.tree_id) as step_count
         FROM exploitation_trees et
         JOIN policies p ON et.policy_id = p.policy_id
         {where_clause}
         ORDER BY et.convergence_score DESC"
    );
    let rows = client.query(&query, &[]).await?;
    Ok(rows
        .iter()
        .map(|r| ExploitationTree {
            tree_id: r.get("tree_id"),
            policy_id: r.get("policy_id"),
            convergence_score: r.get("convergence_score"),
            actor_profile: opt_str(r, "actor_profile"),
            lifecycle_stage: opt_str(r, "lifecycle_stage"),
            detection_difficulty: opt_str(r, "detection_difficulty"),
            review_status: opt_str(r, "review_status"),
            reviewer_notes: opt_str(r, "reviewer_notes"),
            run_id: opt_str(r, "run_id"),
            created_at: r.get("created_at"),
            policy_name: opt_str(r, "policy_name"),
            step_count: opt_i64(r, "step_count"),
            steps: Vec::new(),
        })
        .collect())
}

pub async fn get_exploitation_steps(
    client: &Client,
    tree_id: &str,
) -> Result<Vec<ExploitationStep>, Box<dyn std::error::Error + Send + Sync>> {
    let rows = client
        .query(
            "SELECT es.*,
                    COALESCE(
                        (SELECT json_agg(sq.quality_id ORDER BY sq.quality_id)
                         FROM step_qualities sq WHERE sq.step_id = es.step_id),
                        '[]'::json
                    ) as enabling_qualities
             FROM exploitation_steps es
             WHERE es.tree_id = $1
             ORDER BY es.step_order",
            &[&tree_id],
        )
        .await?;
    Ok(rows.iter().map(row_to_step).collect())
}

pub async fn get_all_exploitation_steps(
    client: &Client,
) -> Result<Vec<ExploitationStep>, Box<dyn std::error::Error + Send + Sync>> {
    let rows = client
        .query(
            "SELECT es.*, et.policy_id, p.name as policy_name,
                    COALESCE(
                        (SELECT json_agg(sq.quality_id ORDER BY sq.quality_id)
                         FROM step_qualities sq WHERE sq.step_id = es.step_id),
                        '[]'::json
                    ) as enabling_qualities
             FROM exploitation_steps es
             JOIN exploitation_trees et ON es.tree_id = et.tree_id
             JOIN policies p ON et.policy_id = p.policy_id
             ORDER BY et.convergence_score DESC, es.step_order",
            &[],
        )
        .await?;
    Ok(rows.iter().map(row_to_step).collect())
}

fn row_to_step(r: &tokio_postgres::Row) -> ExploitationStep {
    let quals_json: Option<Value> = r.try_get("enabling_qualities").ok();
    let enabling_qualities: Vec<String> = quals_json
        .and_then(|v| {
            v.as_array().map(|arr| {
                arr.iter()
                    .filter_map(|item| item.as_str().map(|s| s.to_string()))
                    .collect()
            })
        })
        .unwrap_or_default();

    ExploitationStep {
        step_id: r.get("step_id"),
        tree_id: r.get("tree_id"),
        parent_step_id: opt_str(r, "parent_step_id"),
        step_order: r.get("step_order"),
        title: r.get("title"),
        description: r.get("description"),
        actor_action: opt_str(r, "actor_action"),
        is_branch_point: opt_bool(r, "is_branch_point"),
        branch_label: opt_str(r, "branch_label"),
        created_at: r.get("created_at"),
        policy_id: r.try_get("policy_id").ok(),
        policy_name: r.try_get("policy_name").ok(),
        enabling_qualities,
    }
}

pub async fn delete_tree_for_policy(
    client: &Client,
    policy_id: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    client.execute("BEGIN", &[]).await?;
    let result = async {
        let step_rows = client
            .query(
                "SELECT es.step_id FROM exploitation_steps es
                 JOIN exploitation_trees et ON es.tree_id = et.tree_id
                 WHERE et.policy_id = $1",
                &[&policy_id],
            )
            .await?;
        let step_ids: Vec<String> = step_rows.iter().map(|r| r.get::<_, String>(0)).collect();
        if !step_ids.is_empty() {
            client
                .execute(
                    "DELETE FROM stage_processing_log WHERE stage = 6 AND entity_id = ANY($1)",
                    &[&step_ids],
                )
                .await?;
        }
        client
            .execute(
                "DELETE FROM exploitation_trees WHERE policy_id = $1",
                &[&policy_id],
            )
            .await?;
        Ok::<_, Box<dyn std::error::Error + Send + Sync>>(())
    }
    .await;
    match result {
        Ok(()) => {
            client.execute("COMMIT", &[]).await?;
            Ok(())
        }
        Err(e) => {
            let _ = client.execute("ROLLBACK", &[]).await;
            Err(e)
        }
    }
}

pub async fn delete_patterns_for_step(
    client: &Client,
    step_id: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    client
        .execute(
            "DELETE FROM detection_patterns WHERE step_id = $1",
            &[&step_id],
        )
        .await?;
    Ok(())
}

// ── Detection Patterns ───────────────────────────────────────────────────

pub async fn insert_detection_pattern(
    client: &Client,
    run_id: &str,
    pattern: &DetectionPattern,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    client
        .execute(
            "INSERT INTO detection_patterns
            (pattern_id, run_id, step_id, data_source, anomaly_signal,
             baseline, false_positive_risk, detection_latency, priority,
             implementation_notes, created_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            ON CONFLICT (pattern_id) DO UPDATE SET
                run_id = EXCLUDED.run_id,
                step_id = EXCLUDED.step_id,
                data_source = EXCLUDED.data_source,
                anomaly_signal = EXCLUDED.anomaly_signal,
                baseline = EXCLUDED.baseline,
                false_positive_risk = EXCLUDED.false_positive_risk,
                detection_latency = EXCLUDED.detection_latency,
                priority = EXCLUDED.priority,
                implementation_notes = EXCLUDED.implementation_notes,
                created_at = EXCLUDED.created_at",
            &[
                &pattern.pattern_id,
                &run_id,
                &pattern.step_id,
                &pattern.data_source,
                &pattern.anomaly_signal,
                &pattern.baseline,
                &pattern.false_positive_risk,
                &pattern.detection_latency,
                &pattern.priority,
                &pattern.implementation_notes,
                &now(),
            ],
        )
        .await?;
    Ok(())
}

pub async fn get_detection_patterns(
    client: &Client,
) -> Result<Vec<DetectionPattern>, Box<dyn std::error::Error + Send + Sync>> {
    let rows = client
        .query(
            "SELECT dp.*, es.title as step_title, es.step_id,
                    et.tree_id, p.name as policy_name
             FROM detection_patterns dp
             JOIN exploitation_steps es ON dp.step_id = es.step_id
             JOIN exploitation_trees et ON es.tree_id = et.tree_id
             JOIN policies p ON et.policy_id = p.policy_id
             ORDER BY dp.priority, dp.detection_latency",
            &[],
        )
        .await?;
    Ok(rows
        .iter()
        .map(|r| DetectionPattern {
            pattern_id: r.get("pattern_id"),
            run_id: r.get("run_id"),
            step_id: opt_str(r, "step_id"),
            prediction_id: opt_str(r, "prediction_id"),
            data_source: r.get("data_source"),
            anomaly_signal: r.get("anomaly_signal"),
            baseline: opt_str(r, "baseline"),
            false_positive_risk: opt_str(r, "false_positive_risk"),
            detection_latency: opt_str(r, "detection_latency"),
            priority: opt_str(r, "priority"),
            implementation_notes: opt_str(r, "implementation_notes"),
            created_at: r.get("created_at"),
            step_title: opt_str(r, "step_title"),
            tree_id: opt_str(r, "tree_id"),
            policy_name: opt_str(r, "policy_name"),
        })
        .collect())
}

// ── Documents (RAG) ──────────────────────────────────────────────────────

pub async fn insert_document(
    client: &Client,
    doc_id: &str,
    filename: &str,
    doc_type: &str,
    full_text: &str,
    metadata: Option<&serde_json::Value>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let meta_str = metadata.map(|m| serde_json::to_string(m).unwrap_or_default());
    client
        .execute(
            "INSERT INTO documents
            (doc_id, filename, doc_type, full_text, metadata, created_at)
            VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (doc_id) DO UPDATE SET
                filename = EXCLUDED.filename,
                doc_type = EXCLUDED.doc_type,
                full_text = EXCLUDED.full_text,
                metadata = EXCLUDED.metadata,
                created_at = EXCLUDED.created_at",
            &[&doc_id, &filename, &doc_type, &full_text, &meta_str, &now()],
        )
        .await?;
    Ok(())
}

pub async fn insert_chunk(
    client: &Client,
    chunk_id: &str,
    doc_id: &str,
    chunk_index: i32,
    text: &str,
    token_count: i32,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    client
        .execute(
            "INSERT INTO chunks (chunk_id, doc_id, chunk_index, text, token_count)
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (chunk_id) DO UPDATE SET
                doc_id = EXCLUDED.doc_id,
                chunk_index = EXCLUDED.chunk_index,
                text = EXCLUDED.text,
                token_count = EXCLUDED.token_count",
            &[&chunk_id, &doc_id, &chunk_index, &text, &token_count],
        )
        .await?;
    Ok(())
}

pub async fn search_chunks(
    client: &Client,
    query: &str,
    doc_type: Option<&str>,
    limit: usize,
) -> Result<Vec<Chunk>, Box<dyn std::error::Error + Send + Sync>> {
    let keywords: Vec<String> = query
        .to_lowercase()
        .split_whitespace()
        .map(String::from)
        .collect();
    let fetch_limit = (limit * 5) as i64;

    let rows = if let Some(dt) = doc_type {
        let sql = format!(
            "SELECT c.chunk_id, c.text, c.doc_id, c.chunk_index, c.token_count, d.filename, d.doc_type
             FROM chunks c JOIN documents d ON c.doc_id = d.doc_id
             WHERE d.doc_type = $1
             ORDER BY c.chunk_index
             LIMIT {fetch_limit}"
        );
        client.query(&sql, &[&dt]).await?
    } else {
        let sql = format!(
            "SELECT c.chunk_id, c.text, c.doc_id, c.chunk_index, c.token_count, d.filename, d.doc_type
             FROM chunks c JOIN documents d ON c.doc_id = d.doc_id
             ORDER BY c.chunk_index
             LIMIT {fetch_limit}"
        );
        client.query(&sql, &[]).await?
    };

    let mut scored: Vec<(usize, Chunk)> = Vec::new();
    for r in &rows {
        let text: String = r.get("text");
        let text_lower = text.to_lowercase();
        let score = keywords
            .iter()
            .filter(|kw| text_lower.contains(kw.as_str()))
            .count();
        if score > 0 {
            scored.push((
                score,
                Chunk {
                    chunk_id: r.get("chunk_id"),
                    doc_id: r.get("doc_id"),
                    chunk_index: r.get("chunk_index"),
                    text,
                    token_count: opt_i32(r, "token_count"),
                    filename: opt_str(r, "filename"),
                    doc_type: opt_str(r, "doc_type"),
                },
            ));
        }
    }
    scored.sort_by(|a, b| b.0.cmp(&a.0));
    Ok(scored.into_iter().take(limit).map(|(_, c)| c).collect())
}

pub async fn get_all_documents(
    client: &Client,
    doc_type: Option<&str>,
) -> Result<Vec<Document>, Box<dyn std::error::Error + Send + Sync>> {
    let rows = if let Some(dt) = doc_type {
        client
            .query("SELECT * FROM documents WHERE doc_type=$1", &[&dt])
            .await?
    } else {
        client.query("SELECT * FROM documents", &[]).await?
    };
    Ok(rows
        .iter()
        .map(|r| Document {
            doc_id: r.get("doc_id"),
            filename: opt_str(r, "filename"),
            doc_type: opt_str(r, "doc_type"),
            full_text: r.get("full_text"),
            metadata: opt_str(r, "metadata"),
            created_at: r.get("created_at"),
        })
        .collect())
}

// ── Task Tokens ──────────────────────────────────────────────────────────

pub async fn store_task_token(
    client: &Client,
    run_id: &str,
    stage: i32,
    task_token: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    client
        .execute(
            "UPDATE stage_log SET task_token = $1
             WHERE id = (
                 SELECT id FROM stage_log
                 WHERE run_id = $2 AND stage = $3
                 ORDER BY id DESC LIMIT 1
             )",
            &[&task_token, &run_id, &stage],
        )
        .await?;
    Ok(())
}

pub async fn get_task_token(
    client: &Client,
    run_id: &str,
    stage: i32,
) -> Result<Option<String>, Box<dyn std::error::Error + Send + Sync>> {
    let row = client
        .query_opt(
            "SELECT task_token FROM stage_log WHERE run_id = $1 AND stage = $2 AND task_token IS NOT NULL ORDER BY id DESC LIMIT 1",
            &[&run_id, &stage],
        )
        .await?;
    Ok(row.and_then(|r| opt_str(&r, "task_token")))
}

// ── Enforcement Sources ──────────────────────────────────────────────────

pub async fn upsert_enforcement_source(
    client: &Client,
    source: &EnforcementSource,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    client
        .execute(
            "INSERT INTO enforcement_sources
            (source_id, name, url, source_type, description,
             has_document, s3_key, doc_id, summary, validation_status,
             created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
            ON CONFLICT (source_id) DO UPDATE SET
                name = EXCLUDED.name,
                url = EXCLUDED.url,
                source_type = EXCLUDED.source_type,
                description = EXCLUDED.description,
                has_document = EXCLUDED.has_document,
                s3_key = EXCLUDED.s3_key,
                doc_id = EXCLUDED.doc_id,
                summary = EXCLUDED.summary,
                validation_status = EXCLUDED.validation_status,
                updated_at = EXCLUDED.updated_at",
            &[
                &source.source_id,
                &source.name,
                &source.url,
                &source.source_type,
                &source.description,
                &source.has_document,
                &source.s3_key,
                &source.doc_id,
                &source.summary,
                &source.validation_status.as_deref().unwrap_or("pending"),
                &source.created_at,
                &now(),
            ],
        )
        .await?;
    Ok(())
}

pub async fn get_enforcement_sources(
    client: &Client,
) -> Result<Vec<EnforcementSource>, Box<dyn std::error::Error + Send + Sync>> {
    let rows = client
        .query("SELECT * FROM enforcement_sources ORDER BY created_at", &[])
        .await?;
    Ok(rows.iter().map(row_to_enforcement_source).collect())
}

pub async fn get_enforcement_source(
    client: &Client,
    source_id: &str,
) -> Result<Option<EnforcementSource>, Box<dyn std::error::Error + Send + Sync>> {
    let row = client
        .query_opt(
            "SELECT * FROM enforcement_sources WHERE source_id = $1",
            &[&source_id],
        )
        .await?;
    Ok(row.map(|r| row_to_enforcement_source(&r)))
}

pub async fn get_enforcement_source_by_url(
    client: &Client,
    url: &str,
) -> Result<Option<EnforcementSource>, Box<dyn std::error::Error + Send + Sync>> {
    let row = client
        .query_opt("SELECT * FROM enforcement_sources WHERE url = $1", &[&url])
        .await?;
    Ok(row.map(|r| row_to_enforcement_source(&r)))
}

fn row_to_enforcement_source(r: &tokio_postgres::Row) -> EnforcementSource {
    EnforcementSource {
        source_id: r.get("source_id"),
        name: r.get("name"),
        url: opt_str(r, "url"),
        source_type: r.get("source_type"),
        description: opt_str(r, "description"),
        has_document: r.get("has_document"),
        s3_key: opt_str(r, "s3_key"),
        doc_id: opt_str(r, "doc_id"),
        summary: opt_str(r, "summary"),
        validation_status: opt_str(r, "validation_status"),
        created_at: r.get("created_at"),
        updated_at: r.get("updated_at"),
        candidate_id: opt_str(r, "candidate_id"),
        feed_id: opt_str(r, "feed_id"),
    }
}

pub async fn delete_enforcement_source(
    client: &Client,
    source_id: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    client
        .execute(
            "DELETE FROM enforcement_sources WHERE source_id = $1",
            &[&source_id],
        )
        .await?;
    Ok(())
}

pub async fn update_enforcement_source_document(
    client: &Client,
    source_id: &str,
    s3_key: &str,
    doc_id: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    client
        .execute(
            "UPDATE enforcement_sources SET has_document = TRUE, s3_key = $1, doc_id = $2, validation_status = 'pending', updated_at = $3 WHERE source_id = $4",
            &[&s3_key, &doc_id, &now(), &source_id],
        )
        .await?;
    Ok(())
}

pub async fn update_enforcement_source_summary(
    client: &Client,
    source_id: &str,
    summary: &str,
    validation_status: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    client
        .execute(
            "UPDATE enforcement_sources SET summary = $1, validation_status = $2, updated_at = $3 WHERE source_id = $4",
            &[&summary, &validation_status, &now(), &source_id],
        )
        .await?;
    Ok(())
}

// ── Dimensions ───────────────────────────────────────────────────────────

pub async fn get_dimensions(
    client: &Client,
) -> Result<Vec<Dimension>, Box<dyn std::error::Error + Send + Sync>> {
    let rows = client
        .query("SELECT * FROM dimension_registry ORDER BY name", &[])
        .await?;
    Ok(rows
        .iter()
        .map(|r| Dimension {
            dimension_id: r.get("dimension_id"),
            name: r.get("name"),
            definition: r.get("definition"),
            probing_questions: opt_str(r, "probing_questions"),
            origin: r.get("origin"),
            related_quality_ids: opt_str(r, "related_quality_ids"),
            created_at: r.get("created_at"),
            created_by: opt_str(r, "created_by"),
        })
        .collect())
}

pub async fn upsert_dimension(
    client: &Client,
    dim: &Dimension,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    client
        .execute(
            "INSERT INTO dimension_registry
            (dimension_id, name, definition, probing_questions, origin,
             related_quality_ids, created_at, created_by)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            ON CONFLICT (dimension_id) DO UPDATE SET
                name = EXCLUDED.name,
                definition = EXCLUDED.definition,
                probing_questions = EXCLUDED.probing_questions,
                origin = EXCLUDED.origin,
                related_quality_ids = EXCLUDED.related_quality_ids,
                created_by = EXCLUDED.created_by",
            &[
                &dim.dimension_id,
                &dim.name,
                &dim.definition,
                &dim.probing_questions,
                &dim.origin,
                &dim.related_quality_ids,
                &now(),
                &dim.created_by,
            ],
        )
        .await?;
    Ok(())
}

// ── Structural Findings ──────────────────────────────────────────────────

pub async fn insert_structural_finding(
    client: &Client,
    run_id: &str,
    finding: &StructuralFinding,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    client
        .execute(
            "INSERT INTO structural_findings
            (finding_id, run_id, policy_id, dimension_id, observation,
             source_type, source_citation, source_text, confidence,
             status, stale_reason, created_at, created_by)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
            ON CONFLICT (finding_id) DO UPDATE SET
                observation = EXCLUDED.observation,
                source_type = EXCLUDED.source_type,
                source_citation = EXCLUDED.source_citation,
                source_text = EXCLUDED.source_text,
                confidence = EXCLUDED.confidence,
                status = EXCLUDED.status,
                stale_reason = EXCLUDED.stale_reason",
            &[
                &finding.finding_id,
                &run_id,
                &finding.policy_id,
                &finding.dimension_id,
                &finding.observation,
                &finding.source_type,
                &finding.source_citation,
                &finding.source_text,
                &finding.confidence,
                &finding.status,
                &finding.stale_reason,
                &now(),
                &finding.created_by,
            ],
        )
        .await?;
    Ok(())
}

pub async fn get_structural_findings(
    client: &Client,
    policy_id: &str,
) -> Result<Vec<StructuralFinding>, Box<dyn std::error::Error + Send + Sync>> {
    let rows = client
        .query(
            "SELECT sf.*, dr.name as dimension_name
             FROM structural_findings sf
             LEFT JOIN dimension_registry dr ON sf.dimension_id = dr.dimension_id
             WHERE sf.policy_id=$1 AND sf.status='active'
             ORDER BY sf.dimension_id, sf.created_at",
            &[&policy_id],
        )
        .await?;
    Ok(rows
        .iter()
        .map(|r| StructuralFinding {
            finding_id: r.get("finding_id"),
            run_id: r.get("run_id"),
            policy_id: r.get("policy_id"),
            dimension_id: opt_str(r, "dimension_id"),
            observation: r.get("observation"),
            source_type: r.get("source_type"),
            source_citation: opt_str(r, "source_citation"),
            source_text: opt_str(r, "source_text"),
            confidence: r.get("confidence"),
            status: r.get("status"),
            stale_reason: opt_str(r, "stale_reason"),
            created_at: r.get("created_at"),
            created_by: opt_str(r, "created_by"),
            dimension_name: opt_str(r, "dimension_name"),
        })
        .collect())
}

// ── Quality Assessments ──────────────────────────────────────────────────

pub async fn upsert_quality_assessment(
    client: &Client,
    run_id: &str,
    assessment: &QualityAssessment,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let finding_ids_str = &assessment.evidence_finding_ids;
    let finding_ids: Vec<String> = finding_ids_str
        .as_ref()
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or_default();

    client.execute("BEGIN", &[]).await?;
    let result = async {
        client.execute(
            "INSERT INTO quality_assessments
            (assessment_id, run_id, policy_id, quality_id, taxonomy_version,
             present, evidence_finding_ids, confidence, rationale, created_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            ON CONFLICT (policy_id, quality_id) DO UPDATE SET
                assessment_id = EXCLUDED.assessment_id,
                run_id = EXCLUDED.run_id,
                taxonomy_version = EXCLUDED.taxonomy_version,
                present = EXCLUDED.present,
                evidence_finding_ids = EXCLUDED.evidence_finding_ids,
                confidence = EXCLUDED.confidence,
                rationale = EXCLUDED.rationale,
                created_at = EXCLUDED.created_at",
            &[
                &assessment.assessment_id,
                &run_id,
                &assessment.policy_id,
                &assessment.quality_id,
                &assessment.taxonomy_version,
                &assessment.present,
                &assessment.evidence_finding_ids,
                &assessment.confidence,
                &assessment.rationale,
                &now(),
            ],
        )
        .await?;
        client.execute(
            "DELETE FROM assessment_findings WHERE assessment_id = $1",
            &[&assessment.assessment_id],
        )
        .await?;
        for fid in &finding_ids {
            client.execute(
                "INSERT INTO assessment_findings (assessment_id, finding_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
                &[&assessment.assessment_id, fid],
            )
            .await?;
        }
        Ok::<_, Box<dyn std::error::Error + Send + Sync>>(())
    }
    .await;
    match result {
        Ok(()) => {
            client.execute("COMMIT", &[]).await?;
            Ok(())
        }
        Err(e) => {
            let _ = client.execute("ROLLBACK", &[]).await;
            Err(e)
        }
    }
}

pub async fn get_quality_assessments(
    client: &Client,
    policy_id: Option<&str>,
) -> Result<Vec<QualityAssessment>, Box<dyn std::error::Error + Send + Sync>> {
    let rows = if let Some(pid) = policy_id {
        client
            .query(
                "SELECT * FROM quality_assessments WHERE policy_id=$1 ORDER BY quality_id",
                &[&pid],
            )
            .await?
    } else {
        client
            .query(
                "SELECT * FROM quality_assessments ORDER BY policy_id, quality_id",
                &[],
            )
            .await?
    };
    Ok(rows
        .iter()
        .map(|r| QualityAssessment {
            assessment_id: r.get("assessment_id"),
            run_id: r.get("run_id"),
            policy_id: r.get("policy_id"),
            quality_id: r.get("quality_id"),
            taxonomy_version: opt_str(r, "taxonomy_version"),
            present: r.get("present"),
            evidence_finding_ids: opt_str(r, "evidence_finding_ids"),
            confidence: r.get("confidence"),
            rationale: opt_str(r, "rationale"),
            created_at: r.get("created_at"),
        })
        .collect())
}

// ── Policy Lifecycle ─────────────────────────────────────────────────────

pub async fn update_policy_lifecycle(
    client: &Client,
    policy_id: &str,
    status: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    client
        .execute(
            "UPDATE policies SET lifecycle_status=$1, lifecycle_updated_at=$2 WHERE policy_id=$3",
            &[&status, &now(), &policy_id],
        )
        .await?;
    Ok(())
}

// ── Source Feeds ─────────────────────────────────────────────────────────

pub async fn upsert_source_feed(
    client: &Client,
    feed: &SourceFeed,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    client
        .execute(
            "INSERT INTO source_feeds
            (feed_id, name, listing_url, content_type, link_selector,
             enabled, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            ON CONFLICT (feed_id) DO UPDATE SET
                name = EXCLUDED.name,
                listing_url = EXCLUDED.listing_url,
                content_type = EXCLUDED.content_type,
                link_selector = EXCLUDED.link_selector,
                enabled = EXCLUDED.enabled,
                updated_at = EXCLUDED.updated_at",
            &[
                &feed.feed_id,
                &feed.name,
                &feed.listing_url,
                &feed.content_type,
                &feed.link_selector,
                &feed.enabled.unwrap_or(true),
                &feed.created_at,
                &now(),
            ],
        )
        .await?;
    Ok(())
}

pub async fn get_source_feeds(
    client: &Client,
    enabled_only: bool,
) -> Result<Vec<SourceFeed>, Box<dyn std::error::Error + Send + Sync>> {
    let rows = if enabled_only {
        client
            .query(
                "SELECT * FROM source_feeds WHERE enabled = TRUE ORDER BY name",
                &[],
            )
            .await?
    } else {
        client
            .query("SELECT * FROM source_feeds ORDER BY name", &[])
            .await?
    };
    Ok(rows.iter().map(row_to_feed).collect())
}

fn row_to_feed(r: &tokio_postgres::Row) -> SourceFeed {
    SourceFeed {
        feed_id: r.get("feed_id"),
        name: r.get("name"),
        listing_url: r.get("listing_url"),
        content_type: r.get("content_type"),
        link_selector: opt_str(r, "link_selector"),
        last_checked_at: opt_str(r, "last_checked_at"),
        last_entry_url: opt_str(r, "last_entry_url"),
        enabled: opt_bool(r, "enabled"),
        created_at: r.get("created_at"),
        updated_at: r.get("updated_at"),
    }
}

pub async fn update_feed_last_checked(
    client: &Client,
    feed_id: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    client
        .execute(
            "UPDATE source_feeds SET last_checked_at=$1, updated_at=$2 WHERE feed_id=$3",
            &[&now(), &now(), &feed_id],
        )
        .await?;
    Ok(())
}

// ── Source Candidates ────────────────────────────────────────────────────

pub async fn insert_candidate(
    client: &Client,
    candidate: &SourceCandidate,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    client
        .execute(
            "INSERT INTO source_candidates
            (candidate_id, feed_id, title, url, discovered_at, published_date,
             status, richness_score, richness_rationale, estimated_cases,
             source_id, doc_id, reviewed_by, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
            ON CONFLICT (candidate_id) DO UPDATE SET
                title = EXCLUDED.title,
                status = EXCLUDED.status,
                richness_score = EXCLUDED.richness_score,
                richness_rationale = EXCLUDED.richness_rationale,
                estimated_cases = EXCLUDED.estimated_cases,
                source_id = EXCLUDED.source_id,
                doc_id = EXCLUDED.doc_id,
                reviewed_by = EXCLUDED.reviewed_by,
                updated_at = EXCLUDED.updated_at",
            &[
                &candidate.candidate_id,
                &candidate.feed_id,
                &candidate.title,
                &candidate.url,
                &candidate.discovered_at,
                &candidate.published_date,
                &candidate.status,
                &candidate.richness_score.map(|v| v as f32),
                &candidate.richness_rationale,
                &candidate.estimated_cases,
                &candidate.source_id,
                &candidate.doc_id,
                &candidate.reviewed_by.as_deref().unwrap_or("auto"),
                &now(),
                &now(),
            ],
        )
        .await?;
    Ok(())
}

pub async fn get_candidate_by_url(
    client: &Client,
    url: &str,
) -> Result<Option<SourceCandidate>, Box<dyn std::error::Error + Send + Sync>> {
    let row = client
        .query_opt("SELECT * FROM source_candidates WHERE url = $1", &[&url])
        .await?;
    Ok(row.map(|r| row_to_candidate(&r)))
}

pub async fn get_candidates(
    client: &Client,
    feed_id: Option<&str>,
    status: Option<&str>,
) -> Result<Vec<SourceCandidate>, Box<dyn std::error::Error + Send + Sync>> {
    let rows = match (feed_id, status) {
        (Some(fid), Some(s)) => {
            client
                .query(
                    "SELECT * FROM source_candidates WHERE feed_id = $1 AND status = $2 ORDER BY discovered_at DESC",
                    &[&fid, &s],
                )
                .await?
        }
        (Some(fid), None) => {
            client
                .query(
                    "SELECT * FROM source_candidates WHERE feed_id = $1 ORDER BY discovered_at DESC",
                    &[&fid],
                )
                .await?
        }
        (None, Some(s)) => {
            client
                .query(
                    "SELECT * FROM source_candidates WHERE status = $1 ORDER BY discovered_at DESC",
                    &[&s],
                )
                .await?
        }
        (None, None) => {
            client
                .query(
                    "SELECT * FROM source_candidates ORDER BY discovered_at DESC",
                    &[],
                )
                .await?
        }
    };
    Ok(rows.iter().map(row_to_candidate).collect())
}

fn row_to_candidate(r: &tokio_postgres::Row) -> SourceCandidate {
    SourceCandidate {
        candidate_id: r.get("candidate_id"),
        feed_id: opt_str(r, "feed_id"),
        title: r.get("title"),
        url: r.get("url"),
        discovered_at: r.get("discovered_at"),
        published_date: opt_str(r, "published_date"),
        status: r.get("status"),
        richness_score: r.try_get::<_, f32>("richness_score").ok().map(|v| v as f64),
        richness_rationale: opt_str(r, "richness_rationale"),
        estimated_cases: opt_i32(r, "estimated_cases"),
        source_id: opt_str(r, "source_id"),
        doc_id: opt_str(r, "doc_id"),
        reviewed_by: opt_str(r, "reviewed_by"),
        created_at: r.get("created_at"),
        updated_at: r.get("updated_at"),
    }
}

pub async fn update_candidate_status(
    client: &Client,
    candidate_id: &str,
    status: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    client
        .execute(
            "UPDATE source_candidates SET status=$1, updated_at=$2 WHERE candidate_id=$3",
            &[&status, &now(), &candidate_id],
        )
        .await?;
    Ok(())
}

pub async fn update_candidate_richness(
    client: &Client,
    candidate_id: &str,
    score: f64,
    rationale: &str,
    estimated_cases: i32,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let score_f32 = score as f32;
    client
        .execute(
            "UPDATE source_candidates SET richness_score=$1, richness_rationale=$2, estimated_cases=$3, status='scored', updated_at=$4 WHERE candidate_id=$5",
            &[&score_f32, &rationale, &estimated_cases, &now(), &candidate_id],
        )
        .await?;
    Ok(())
}

pub async fn update_candidate_ingested(
    client: &Client,
    candidate_id: &str,
    source_id: &str,
    doc_id: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    client
        .execute(
            "UPDATE source_candidates SET source_id=$1, doc_id=$2, status='ingested', updated_at=$3 WHERE candidate_id=$4",
            &[&source_id, &doc_id, &now(), &candidate_id],
        )
        .await?;
    Ok(())
}

// ── Triage Results ───────────────────────────────────────────────────────

pub async fn insert_triage_result(
    client: &Client,
    run_id: &str,
    result: &TriageResult,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let score_f32 = result.triage_score as f32;
    client
        .execute(
            "INSERT INTO triage_results
            (run_id, policy_id, triage_score, rationale, uncertainty, priority_rank, created_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            ON CONFLICT (policy_id) DO UPDATE SET
                run_id = EXCLUDED.run_id,
                triage_score = EXCLUDED.triage_score,
                rationale = EXCLUDED.rationale,
                uncertainty = EXCLUDED.uncertainty,
                priority_rank = EXCLUDED.priority_rank,
                created_at = EXCLUDED.created_at",
            &[
                &run_id,
                &result.policy_id,
                &score_f32,
                &result.rationale,
                &result.uncertainty,
                &result.priority_rank,
                &now(),
            ],
        )
        .await?;
    Ok(())
}

pub async fn get_triage_results(
    client: &Client,
) -> Result<Vec<TriageResult>, Box<dyn std::error::Error + Send + Sync>> {
    let rows = client
        .query(
            "SELECT tr.*, p.name as policy_name
             FROM triage_results tr
             JOIN policies p ON tr.policy_id = p.policy_id
             ORDER BY tr.priority_rank",
            &[],
        )
        .await?;
    Ok(rows
        .iter()
        .map(|r| TriageResult {
            policy_id: r.get("policy_id"),
            triage_score: r.try_get::<_, f32>("triage_score").unwrap_or(0.0) as f64,
            rationale: r.get("rationale"),
            uncertainty: opt_str(r, "uncertainty"),
            priority_rank: r.get("priority_rank"),
            policy_name: opt_str(r, "policy_name"),
            run_id: opt_str(r, "run_id"),
        })
        .collect())
}

// ── Research Sessions ────────────────────────────────────────────────────

pub async fn create_research_session(
    client: &Client,
    run_id: &str,
    policy_id: &str,
    session_id: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    client
        .execute(
            "INSERT INTO research_sessions (session_id, run_id, policy_id, status, trigger, started_at)
             VALUES ($1, $2, $3, 'pending', 'initial', $4) ON CONFLICT (session_id) DO NOTHING",
            &[&session_id, &run_id, &policy_id, &now()],
        )
        .await?;
    Ok(())
}

pub async fn update_research_session(
    client: &Client,
    session_id: &str,
    status: &str,
    error: Option<&str>,
    sources_queried: Option<&serde_json::Value>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let completed = if matches!(
        status,
        "findings_complete" | "assessment_complete" | "failed"
    ) {
        Some(now())
    } else {
        None
    };
    let sources_str = sources_queried.map(|s| serde_json::to_string(s).unwrap_or_default());
    client
        .execute(
            "UPDATE research_sessions
             SET status=$1, error_message=$2, completed_at=COALESCE($3, completed_at),
                 sources_queried=COALESCE($4, sources_queried)
             WHERE session_id=$5",
            &[&status, &error, &completed, &sources_str, &session_id],
        )
        .await?;
    Ok(())
}

pub async fn get_research_sessions(
    client: &Client,
    status: Option<&str>,
) -> Result<Vec<ResearchSession>, Box<dyn std::error::Error + Send + Sync>> {
    let rows = if let Some(s) = status {
        client
            .query(
                "SELECT * FROM research_sessions WHERE status=$1 ORDER BY started_at",
                &[&s],
            )
            .await?
    } else {
        client
            .query("SELECT * FROM research_sessions ORDER BY started_at", &[])
            .await?
    };
    Ok(rows
        .iter()
        .map(|r| ResearchSession {
            session_id: r.get("session_id"),
            run_id: r.get("run_id"),
            policy_id: r.get("policy_id"),
            status: r.get("status"),
            sources_queried: opt_str(r, "sources_queried"),
            started_at: opt_str(r, "started_at"),
            completed_at: opt_str(r, "completed_at"),
            error_message: opt_str(r, "error_message"),
            trigger: opt_str(r, "trigger"),
        })
        .collect())
}

// ── Regulatory Sources ───────────────────────────────────────────────────

pub async fn insert_regulatory_source(
    client: &Client,
    source: &RegulatorySource,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    client
        .execute(
            "INSERT INTO regulatory_sources
            (source_id, source_type, url, title, cfr_reference, full_text, fetched_at, metadata)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            ON CONFLICT (source_id) DO UPDATE SET
                full_text = EXCLUDED.full_text,
                fetched_at = EXCLUDED.fetched_at,
                metadata = EXCLUDED.metadata",
            &[
                &source.source_id,
                &source.source_type,
                &source.url,
                &source.title,
                &source.cfr_reference,
                &source.full_text,
                &now(),
                &source.metadata,
            ],
        )
        .await?;
    Ok(())
}

pub async fn get_regulatory_source(
    client: &Client,
    source_id: &str,
) -> Result<Option<RegulatorySource>, Box<dyn std::error::Error + Send + Sync>> {
    let row = client
        .query_opt(
            "SELECT * FROM regulatory_sources WHERE source_id = $1",
            &[&source_id],
        )
        .await?;
    Ok(row.map(|r| RegulatorySource {
        source_id: r.get("source_id"),
        source_type: r.get("source_type"),
        url: r.get("url"),
        title: opt_str(&r, "title"),
        cfr_reference: opt_str(&r, "cfr_reference"),
        full_text: r.get("full_text"),
        fetched_at: r.get("fetched_at"),
        metadata: opt_str(&r, "metadata"),
    }))
}
