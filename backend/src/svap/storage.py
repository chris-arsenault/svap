"""
Storage layer for SVAP pipeline.

All intermediate outputs are stored in PostgreSQL. Each stage reads its inputs
from the database and writes its outputs back. This enables:
  - Resumability: re-run any stage without re-running predecessors
  - Auditability: every intermediate result is preserved
  - Scalability: PostgreSQL handles concurrent access and large datasets
"""

import json
import os
import subprocess
import sys
from datetime import UTC, datetime
from pathlib import Path

import psycopg2
import psycopg2.extras

_TERRAFORM_DIR = Path(__file__).resolve().parent.parent.parent.parent / "infrastructure" / "terraform"


def resolve_database_url(config: dict | None = None) -> str:
    """Resolve DATABASE_URL from env, terraform state, or config.

    Resolution order:
      1. DATABASE_URL environment variable
      2. Terraform state output ``database_url``
      3. ``config["storage"]["database_url"]`` (if config provided)
    """
    url = os.environ.get("DATABASE_URL")
    if url:
        return url

    # Try terraform state
    if (_TERRAFORM_DIR / ".terraform").exists():
        try:
            result = subprocess.run(
                ["terraform", "output", "-raw", "database_url"],
                cwd=_TERRAFORM_DIR,
                capture_output=True,
                text=True,
                timeout=15,
            )
            if result.returncode == 0 and result.stdout.strip():
                return result.stdout.strip()
        except (FileNotFoundError, subprocess.TimeoutExpired):
            pass

    # Fall back to config
    if config:
        url = config.get("storage", {}).get("database_url")
        if url:
            return url

    print(
        "Error: Could not resolve database URL.\n"
        "  Set DATABASE_URL or ensure terraform is initialized in\n"
        f"  {_TERRAFORM_DIR}",
        file=sys.stderr,
    )
    sys.exit(1)

_conn = None


def _get_connection(database_url: str):
    global _conn
    if _conn is None or _conn.closed:
        _conn = psycopg2.connect(database_url)
    return _conn


SCHEMA_STATEMENTS = [
    # Pipeline run metadata
    """CREATE TABLE IF NOT EXISTS pipeline_runs (
        run_id          TEXT PRIMARY KEY,
        created_at      TEXT NOT NULL,
        config_snapshot TEXT NOT NULL,
        notes           TEXT
    )""",
    # Stage execution log
    """CREATE TABLE IF NOT EXISTS stage_log (
        id              SERIAL PRIMARY KEY,
        run_id          TEXT NOT NULL,
        stage           INTEGER NOT NULL,
        status          TEXT NOT NULL CHECK(status IN ('running','completed','failed','pending_review','approved')),
        started_at      TEXT,
        completed_at    TEXT,
        error_message   TEXT,
        metadata        TEXT,
        task_token       TEXT,
        FOREIGN KEY (run_id) REFERENCES pipeline_runs(run_id)
    )""",
    # Stage 1: Enforcement cases (extracted from documents) — GLOBAL, no run_id
    """CREATE TABLE IF NOT EXISTS cases (
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
    )""",
    # Stage 2: Vulnerability taxonomy — GLOBAL, no run_id
    """CREATE TABLE IF NOT EXISTS taxonomy (
        quality_id          TEXT PRIMARY KEY,
        name                TEXT NOT NULL,
        definition          TEXT NOT NULL,
        recognition_test    TEXT NOT NULL,
        exploitation_logic  TEXT NOT NULL,
        canonical_examples  TEXT,
        review_status       TEXT DEFAULT 'draft' CHECK(review_status IN ('draft','approved','rejected','revised')),
        reviewer_notes      TEXT,
        created_at          TEXT NOT NULL
    )""",
    # Stage 2: Taxonomy case processing log — tracks which cases contributed to taxonomy
    """CREATE TABLE IF NOT EXISTS taxonomy_case_log (
        case_id             TEXT PRIMARY KEY,
        processed_at        TEXT NOT NULL,
        FOREIGN KEY (case_id) REFERENCES cases(case_id)
    )""",
    # Stage 3: Convergence scores for known cases
    """CREATE TABLE IF NOT EXISTS convergence_scores (
        id                  SERIAL PRIMARY KEY,
        run_id              TEXT NOT NULL,
        case_id             TEXT NOT NULL,
        quality_id          TEXT NOT NULL,
        present             INTEGER NOT NULL CHECK(present IN (0, 1)),
        evidence            TEXT,
        created_at          TEXT NOT NULL,
        FOREIGN KEY (run_id) REFERENCES pipeline_runs(run_id),
        FOREIGN KEY (case_id) REFERENCES cases(case_id),
        FOREIGN KEY (quality_id) REFERENCES taxonomy(quality_id)
    )""",
    # Stage 3: Calibration results
    """CREATE TABLE IF NOT EXISTS calibration (
        run_id              TEXT PRIMARY KEY,
        threshold           INTEGER NOT NULL,
        correlation_notes   TEXT,
        quality_frequency   TEXT,
        quality_combinations TEXT,
        created_at          TEXT NOT NULL,
        FOREIGN KEY (run_id) REFERENCES pipeline_runs(run_id)
    )""",
    # Stage 4: Policies to scan — GLOBAL, no run_id
    """CREATE TABLE IF NOT EXISTS policies (
        policy_id           TEXT PRIMARY KEY,
        name                TEXT NOT NULL,
        description         TEXT,
        source_document     TEXT,
        structural_characterization TEXT,
        created_at          TEXT NOT NULL
    )""",
    # Stage 4: Policy convergence scores
    """CREATE TABLE IF NOT EXISTS policy_scores (
        id                  SERIAL PRIMARY KEY,
        run_id              TEXT NOT NULL,
        policy_id           TEXT NOT NULL,
        quality_id          TEXT NOT NULL,
        present             INTEGER NOT NULL CHECK(present IN (0, 1)),
        evidence            TEXT,
        created_at          TEXT NOT NULL,
        FOREIGN KEY (run_id) REFERENCES pipeline_runs(run_id),
        FOREIGN KEY (policy_id) REFERENCES policies(policy_id),
        FOREIGN KEY (quality_id) REFERENCES taxonomy(quality_id)
    )""",
    # Stage 5: Exploitation predictions
    """CREATE TABLE IF NOT EXISTS predictions (
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
        FOREIGN KEY (run_id) REFERENCES pipeline_runs(run_id),
        FOREIGN KEY (policy_id) REFERENCES policies(policy_id)
    )""",
    # Stage 6: Detection patterns
    """CREATE TABLE IF NOT EXISTS detection_patterns (
        pattern_id          TEXT PRIMARY KEY,
        run_id              TEXT NOT NULL,
        prediction_id       TEXT NOT NULL,
        data_source         TEXT NOT NULL,
        anomaly_signal      TEXT NOT NULL,
        baseline            TEXT,
        false_positive_risk TEXT,
        detection_latency   TEXT,
        priority            TEXT CHECK(priority IN ('critical','high','medium','low')),
        implementation_notes TEXT,
        created_at          TEXT NOT NULL,
        FOREIGN KEY (run_id) REFERENCES pipeline_runs(run_id),
        FOREIGN KEY (prediction_id) REFERENCES predictions(prediction_id)
    )""",
    # RAG: Document store for source documents
    """CREATE TABLE IF NOT EXISTS documents (
        doc_id              TEXT PRIMARY KEY,
        filename            TEXT,
        doc_type            TEXT CHECK(doc_type IN ('enforcement','policy','guidance','report','other')),
        full_text           TEXT NOT NULL,
        metadata            TEXT,
        created_at          TEXT NOT NULL
    )""",
    # RAG: Document chunks for retrieval
    """CREATE TABLE IF NOT EXISTS chunks (
        chunk_id            TEXT PRIMARY KEY,
        doc_id              TEXT NOT NULL,
        chunk_index         INTEGER NOT NULL,
        text                TEXT NOT NULL,
        token_count         INTEGER,
        FOREIGN KEY (doc_id) REFERENCES documents(doc_id)
    )""",
    # Enforcement source registry (mutable, not tied to a run)
    """CREATE TABLE IF NOT EXISTS enforcement_sources (
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
        updated_at        TEXT NOT NULL
    )""",
    # Unique indexes for upsert conflict targets
    "CREATE UNIQUE INDEX IF NOT EXISTS uq_convergence ON convergence_scores(run_id, case_id, quality_id)",
    "CREATE UNIQUE INDEX IF NOT EXISTS uq_policy_score ON policy_scores(run_id, policy_id, quality_id)",
    # ── Sourcing Agents: Dimension Registry ──────────────────────
    """CREATE TABLE IF NOT EXISTS dimension_registry (
        dimension_id        TEXT PRIMARY KEY,
        name                TEXT NOT NULL,
        definition          TEXT NOT NULL,
        probing_questions   TEXT,
        origin              TEXT NOT NULL CHECK(origin IN ('case_derived','policy_derived','manual','seed')),
        related_quality_ids TEXT,
        created_at          TEXT NOT NULL,
        created_by          TEXT
    )""",
    # ── Sourcing Agents: Structural Findings ─────────────────────
    """CREATE TABLE IF NOT EXISTS structural_findings (
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
        FOREIGN KEY (run_id) REFERENCES pipeline_runs(run_id),
        FOREIGN KEY (policy_id) REFERENCES policies(policy_id),
        FOREIGN KEY (dimension_id) REFERENCES dimension_registry(dimension_id)
    )""",
    # ── Sourcing Agents: Quality Assessments ─────────────────────
    """CREATE TABLE IF NOT EXISTS quality_assessments (
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
        FOREIGN KEY (run_id) REFERENCES pipeline_runs(run_id),
        FOREIGN KEY (policy_id) REFERENCES policies(policy_id),
        FOREIGN KEY (quality_id) REFERENCES taxonomy(quality_id)
    )""",
    "CREATE UNIQUE INDEX IF NOT EXISTS uq_quality_assessment ON quality_assessments(run_id, policy_id, quality_id)",
    # ── Sourcing Agents: Source Feeds ────────────────────────────
    """CREATE TABLE IF NOT EXISTS source_feeds (
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
    )""",
    # ── Sourcing Agents: Source Candidates ────────────────────────
    """CREATE TABLE IF NOT EXISTS source_candidates (
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
    )""",
    # ── Sourcing Agents: Triage Results ──────────────────────────
    """CREATE TABLE IF NOT EXISTS triage_results (
        id                  SERIAL PRIMARY KEY,
        run_id              TEXT NOT NULL,
        policy_id           TEXT NOT NULL,
        triage_score        REAL NOT NULL,
        rationale           TEXT NOT NULL,
        uncertainty         TEXT,
        priority_rank       INTEGER NOT NULL,
        created_at          TEXT NOT NULL,
        FOREIGN KEY (run_id) REFERENCES pipeline_runs(run_id),
        FOREIGN KEY (policy_id) REFERENCES policies(policy_id)
    )""",
    "CREATE UNIQUE INDEX IF NOT EXISTS uq_triage ON triage_results(run_id, policy_id)",
    # ── Sourcing Agents: Research Sessions ───────────────────────
    """CREATE TABLE IF NOT EXISTS research_sessions (
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
        FOREIGN KEY (run_id) REFERENCES pipeline_runs(run_id),
        FOREIGN KEY (policy_id) REFERENCES policies(policy_id)
    )""",
    # ── Sourcing Agents: Regulatory Source Cache ─────────────────
    """CREATE TABLE IF NOT EXISTS regulatory_sources (
        source_id           TEXT PRIMARY KEY,
        source_type         TEXT NOT NULL,
        url                 TEXT NOT NULL,
        title               TEXT,
        cfr_reference       TEXT,
        full_text           TEXT NOT NULL,
        fetched_at          TEXT NOT NULL,
        metadata            TEXT
    )""",
    # ── ALTER existing tables for sourcing agents ────────────────
    "ALTER TABLE policies ADD COLUMN IF NOT EXISTS lifecycle_status TEXT DEFAULT 'cataloged'",
    "ALTER TABLE policies ADD COLUMN IF NOT EXISTS lifecycle_updated_at TEXT",
    "ALTER TABLE enforcement_sources ADD COLUMN IF NOT EXISTS candidate_id TEXT",
    "ALTER TABLE enforcement_sources ADD COLUMN IF NOT EXISTS feed_id TEXT",
    # ── Data integrity ──────────────────────────────────────────
    "CREATE UNIQUE INDEX IF NOT EXISTS uq_enforcement_source_url ON enforcement_sources(url) WHERE url IS NOT NULL",
    # ── Migration: drop run_id from global tables ────────────
    "ALTER TABLE cases DROP COLUMN IF EXISTS run_id",
    "ALTER TABLE cases ADD COLUMN IF NOT EXISTS source_doc_id TEXT",
    "ALTER TABLE taxonomy DROP COLUMN IF EXISTS run_id",
    "ALTER TABLE policies DROP COLUMN IF EXISTS run_id",
    # ── Migration: drop dedup/traceability columns from documents ──
    "ALTER TABLE documents DROP COLUMN IF EXISTS content_hash",
    "ALTER TABLE documents DROP COLUMN IF EXISTS source_id",
    "ALTER TABLE documents DROP COLUMN IF EXISTS last_processed_run",
]


class SVAPStorage:
    """PostgreSQL-backed storage for all pipeline state."""

    def __init__(self, database_url: str):
        self.database_url = database_url
        self.conn = _get_connection(database_url)
        self._init_schema()

    def _safe_commit(self):
        """Commit, or rollback on error to avoid 'aborted transaction' state."""
        try:
            self.conn.commit()
        except Exception:
            self.conn.rollback()
            raise

    def _init_schema(self):
        with self.conn.cursor() as cur:
            for stmt in SCHEMA_STATEMENTS:
                cur.execute(stmt)
        self.conn.commit()

    def close(self):
        # No-op: keep module-level connection alive for Lambda warm starts
        pass

    # ── Run management ──────────────────────────────────────────────

    def create_run(self, run_id: str, config: dict, notes: str = "") -> str:
        with self.conn.cursor() as cur:
            cur.execute(
                "INSERT INTO pipeline_runs (run_id, created_at, config_snapshot, notes) VALUES (%s, %s, %s, %s)",
                (run_id, _now(), json.dumps(config), notes),
            )
        self._safe_commit()
        return run_id

    def get_latest_run(self) -> str | None:
        with self.conn.cursor(cursor_factory=psycopg2.extras.RealDictCursor) as cur:
            cur.execute("SELECT run_id FROM pipeline_runs ORDER BY created_at DESC LIMIT 1")
            row = cur.fetchone()
        return row["run_id"] if row else None

    # ── Stage log ───────────────────────────────────────────────────

    def log_stage_start(self, run_id: str, stage: int):
        with self.conn.cursor() as cur:
            cur.execute(
                "INSERT INTO stage_log (run_id, stage, status, started_at) VALUES (%s, %s, 'running', %s)",
                (run_id, stage, _now()),
            )
        self._safe_commit()

    def log_stage_complete(self, run_id: str, stage: int, metadata: dict | None = None):
        with self.conn.cursor() as cur:
            cur.execute(
                "UPDATE stage_log SET status='completed', completed_at=%s, metadata=%s "
                "WHERE run_id=%s AND stage=%s AND status='running'",
                (_now(), json.dumps(metadata) if metadata else None, run_id, stage),
            )
        self._safe_commit()

    def log_stage_failed(self, run_id: str, stage: int, error: str):
        with self.conn.cursor() as cur:
            cur.execute(
                "UPDATE stage_log SET status='failed', completed_at=%s, error_message=%s "
                "WHERE run_id=%s AND stage=%s AND status='running'",
                (_now(), error, run_id, stage),
            )
        self._safe_commit()

    def log_stage_pending_review(self, run_id: str, stage: int):
        with self.conn.cursor() as cur:
            cur.execute(
                "UPDATE stage_log SET status='pending_review', completed_at=%s "
                "WHERE run_id=%s AND stage=%s AND status='running'",
                (_now(), run_id, stage),
            )
        self._safe_commit()

    def approve_stage(self, run_id: str, stage: int):
        with self.conn.cursor() as cur:
            cur.execute(
                "UPDATE stage_log SET status='approved' WHERE run_id=%s AND stage=%s AND status='pending_review'",
                (run_id, stage),
            )
        self._safe_commit()

    def get_stage_status(self, run_id: str, stage: int) -> str | None:
        with self.conn.cursor(cursor_factory=psycopg2.extras.RealDictCursor) as cur:
            cur.execute(
                "SELECT status FROM stage_log WHERE run_id=%s AND stage=%s ORDER BY id DESC LIMIT 1",
                (run_id, stage),
            )
            row = cur.fetchone()
        return row["status"] if row else None

    def get_pipeline_status(self, run_id: str) -> list[dict]:
        with self.conn.cursor(cursor_factory=psycopg2.extras.RealDictCursor) as cur:
            cur.execute(
                "SELECT DISTINCT ON (stage) stage, status, started_at, completed_at, error_message "
                "FROM stage_log WHERE run_id=%s ORDER BY stage, id DESC",
                (run_id,),
            )
            rows = cur.fetchall()
        return [dict(r) for r in rows]

    # ── Stage 1: Cases ──────────────────────────────────────────────

    def insert_case(self, case: dict):
        with self.conn.cursor() as cur:
            cur.execute(
                """INSERT INTO cases
                (case_id, source_doc_id, case_name, scheme_mechanics,
                 exploited_policy, enabling_condition, scale_dollars, scale_defendants,
                 scale_duration, detection_method, raw_extraction, created_at)
                VALUES (%s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s)
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
                    created_at = EXCLUDED.created_at""",
                (
                    case["case_id"],
                    case.get("source_doc_id"),
                    case["case_name"],
                    case["scheme_mechanics"],
                    case["exploited_policy"],
                    case["enabling_condition"],
                    case.get("scale_dollars"),
                    case.get("scale_defendants"),
                    case.get("scale_duration"),
                    case.get("detection_method"),
                    json.dumps(case.get("raw_extraction")),
                    _now(),
                ),
            )
        self._safe_commit()

    def cases_exist_for_document(self, doc_id: str) -> bool:
        """Check if cases have already been extracted from this document."""
        with self.conn.cursor() as cur:
            cur.execute("SELECT 1 FROM cases WHERE source_doc_id = %s LIMIT 1", (doc_id,))
            return cur.fetchone() is not None

    def get_cases(self) -> list[dict]:
        with self.conn.cursor(cursor_factory=psycopg2.extras.RealDictCursor) as cur:
            cur.execute("SELECT * FROM cases")
            rows = cur.fetchall()
        return [dict(r) for r in rows]

    # ── Stage 2: Taxonomy ───────────────────────────────────────────

    def insert_quality(self, quality: dict):
        with self.conn.cursor() as cur:
            cur.execute(
                """INSERT INTO taxonomy
                (quality_id, name, definition, recognition_test,
                 exploitation_logic, canonical_examples, review_status, created_at)
                VALUES (%s, %s, %s, %s, %s, %s, %s, %s)
                ON CONFLICT (quality_id) DO UPDATE SET
                    name = EXCLUDED.name,
                    definition = EXCLUDED.definition,
                    recognition_test = EXCLUDED.recognition_test,
                    exploitation_logic = EXCLUDED.exploitation_logic,
                    canonical_examples = EXCLUDED.canonical_examples,
                    review_status = EXCLUDED.review_status,
                    created_at = EXCLUDED.created_at""",
                (
                    quality["quality_id"],
                    quality["name"],
                    quality["definition"],
                    quality["recognition_test"],
                    quality["exploitation_logic"],
                    json.dumps(quality.get("canonical_examples", [])),
                    quality.get("review_status", "draft"),
                    _now(),
                ),
            )
        self._safe_commit()

    def get_taxonomy(self) -> list[dict]:
        with self.conn.cursor(cursor_factory=psycopg2.extras.RealDictCursor) as cur:
            cur.execute("SELECT * FROM taxonomy ORDER BY quality_id")
            rows = cur.fetchall()
        return [dict(r) for r in rows]

    def has_approved_taxonomy(self) -> bool:
        """Check if an approved taxonomy exists."""
        with self.conn.cursor() as cur:
            cur.execute("SELECT COUNT(*) FROM taxonomy WHERE review_status = 'approved'")
            return cur.fetchone()[0] > 0

    def update_quality_review(self, quality_id: str, status: str, notes: str = ""):
        with self.conn.cursor() as cur:
            cur.execute(
                "UPDATE taxonomy SET review_status=%s, reviewer_notes=%s WHERE quality_id=%s",
                (status, notes, quality_id),
            )
        self._safe_commit()

    def get_approved_taxonomy(self) -> list[dict]:
        """Return only approved qualities for use by downstream pipeline stages."""
        with self.conn.cursor(cursor_factory=psycopg2.extras.RealDictCursor) as cur:
            cur.execute("SELECT * FROM taxonomy WHERE review_status = 'approved' ORDER BY quality_id")
            rows = cur.fetchall()
        return [dict(r) for r in rows]

    def get_taxonomy_processed_case_ids(self) -> set[str]:
        """Return the set of case_ids already processed for taxonomy extraction."""
        with self.conn.cursor() as cur:
            cur.execute("SELECT case_id FROM taxonomy_case_log")
            rows = cur.fetchall()
        return {r[0] for r in rows}

    def record_taxonomy_case_processed(self, case_id: str):
        """Record that a case has been processed for taxonomy extraction."""
        with self.conn.cursor() as cur:
            cur.execute(
                "INSERT INTO taxonomy_case_log (case_id, processed_at) VALUES (%s, %s) "
                "ON CONFLICT (case_id) DO NOTHING",
                (case_id, _now()),
            )
        self._safe_commit()

    def merge_quality_examples(self, quality_id: str, new_examples: list[str]):
        """Append new canonical examples to an existing taxonomy quality."""
        with self.conn.cursor() as cur:
            cur.execute(
                "SELECT canonical_examples FROM taxonomy WHERE quality_id = %s",
                (quality_id,),
            )
            row = cur.fetchone()
            if not row:
                return
            existing = json.loads(row[0]) if row[0] else []
            for ex in new_examples:
                if ex not in existing:
                    existing.append(ex)
            cur.execute(
                "UPDATE taxonomy SET canonical_examples = %s WHERE quality_id = %s",
                (json.dumps(existing), quality_id),
            )
        self._safe_commit()

    # ── Stage 3: Convergence Scores ─────────────────────────────────

    def insert_convergence_score(
        self, run_id: str, case_id: str, quality_id: str, present: bool, evidence: str
    ):
        with self.conn.cursor() as cur:
            cur.execute(
                """INSERT INTO convergence_scores
                (run_id, case_id, quality_id, present, evidence, created_at)
                VALUES (%s, %s, %s, %s, %s, %s)
                ON CONFLICT (run_id, case_id, quality_id) DO UPDATE SET
                    present = EXCLUDED.present,
                    evidence = EXCLUDED.evidence,
                    created_at = EXCLUDED.created_at""",
                (run_id, case_id, quality_id, int(present), evidence, _now()),
            )
        self._safe_commit()

    def get_convergence_matrix(self, run_id: str) -> list[dict]:
        with self.conn.cursor(cursor_factory=psycopg2.extras.RealDictCursor) as cur:
            cur.execute(
                """SELECT c.case_name, c.case_id, c.scale_dollars,
                          cs.quality_id, cs.present, cs.evidence
                   FROM convergence_scores cs
                   JOIN cases c ON cs.case_id = c.case_id
                   WHERE cs.run_id=%s
                   ORDER BY c.case_id, cs.quality_id""",
                (run_id,),
            )
            rows = cur.fetchall()
        return [dict(r) for r in rows]

    def insert_calibration(self, run_id: str, threshold: int, notes: str, freq: dict, combos: dict):
        with self.conn.cursor() as cur:
            cur.execute(
                """INSERT INTO calibration
                (run_id, threshold, correlation_notes, quality_frequency, quality_combinations, created_at)
                VALUES (%s, %s, %s, %s, %s, %s)
                ON CONFLICT (run_id) DO UPDATE SET
                    threshold = EXCLUDED.threshold,
                    correlation_notes = EXCLUDED.correlation_notes,
                    quality_frequency = EXCLUDED.quality_frequency,
                    quality_combinations = EXCLUDED.quality_combinations,
                    created_at = EXCLUDED.created_at""",
                (run_id, threshold, notes, json.dumps(freq), json.dumps(combos), _now()),
            )
        self._safe_commit()

    def get_calibration(self, run_id: str) -> dict | None:
        with self.conn.cursor(cursor_factory=psycopg2.extras.RealDictCursor) as cur:
            cur.execute("SELECT * FROM calibration WHERE run_id=%s", (run_id,))
            row = cur.fetchone()
        return dict(row) if row else None

    # ── Stage 4: Policies ───────────────────────────────────────────

    def insert_policy(self, policy: dict):
        with self.conn.cursor() as cur:
            cur.execute(
                """INSERT INTO policies
                (policy_id, name, description, source_document,
                 structural_characterization, created_at)
                VALUES (%s, %s, %s, %s, %s, %s)
                ON CONFLICT (policy_id) DO UPDATE SET
                    name = EXCLUDED.name,
                    description = EXCLUDED.description,
                    source_document = EXCLUDED.source_document,
                    structural_characterization = EXCLUDED.structural_characterization,
                    created_at = EXCLUDED.created_at""",
                (
                    policy["policy_id"],
                    policy["name"],
                    policy.get("description"),
                    policy.get("source_document"),
                    policy.get("structural_characterization"),
                    _now(),
                ),
            )
        self._safe_commit()

    def get_policies(self) -> list[dict]:
        with self.conn.cursor(cursor_factory=psycopg2.extras.RealDictCursor) as cur:
            cur.execute("SELECT * FROM policies")
            rows = cur.fetchall()
        return [dict(r) for r in rows]

    def insert_policy_score(
        self, run_id: str, policy_id: str, quality_id: str, present: bool, evidence: str
    ):
        with self.conn.cursor() as cur:
            cur.execute(
                """INSERT INTO policy_scores
                (run_id, policy_id, quality_id, present, evidence, created_at)
                VALUES (%s, %s, %s, %s, %s, %s)
                ON CONFLICT (run_id, policy_id, quality_id) DO UPDATE SET
                    present = EXCLUDED.present,
                    evidence = EXCLUDED.evidence,
                    created_at = EXCLUDED.created_at""",
                (run_id, policy_id, quality_id, int(present), evidence, _now()),
            )
        self._safe_commit()

    def get_policy_scores(self, run_id: str) -> list[dict]:
        with self.conn.cursor(cursor_factory=psycopg2.extras.RealDictCursor) as cur:
            cur.execute(
                """SELECT p.name, p.policy_id, ps.quality_id, ps.present, ps.evidence
                   FROM policy_scores ps
                   JOIN policies p ON ps.policy_id = p.policy_id
                   WHERE ps.run_id=%s
                   ORDER BY p.policy_id, ps.quality_id""",
                (run_id,),
            )
            rows = cur.fetchall()
        return [dict(r) for r in rows]

    # ── Stage 5: Predictions ────────────────────────────────────────

    def insert_prediction(self, run_id: str, pred: dict):
        with self.conn.cursor() as cur:
            cur.execute(
                """INSERT INTO predictions
                (prediction_id, run_id, policy_id, convergence_score, mechanics,
                 enabling_qualities, actor_profile, lifecycle_stage,
                 detection_difficulty, review_status, created_at)
                VALUES (%s, %s, %s, %s, %s, %s, %s, %s, %s, 'draft', %s)
                ON CONFLICT (prediction_id) DO UPDATE SET
                    run_id = EXCLUDED.run_id,
                    policy_id = EXCLUDED.policy_id,
                    convergence_score = EXCLUDED.convergence_score,
                    mechanics = EXCLUDED.mechanics,
                    enabling_qualities = EXCLUDED.enabling_qualities,
                    actor_profile = EXCLUDED.actor_profile,
                    lifecycle_stage = EXCLUDED.lifecycle_stage,
                    detection_difficulty = EXCLUDED.detection_difficulty,
                    review_status = EXCLUDED.review_status,
                    created_at = EXCLUDED.created_at""",
                (
                    pred["prediction_id"],
                    run_id,
                    pred["policy_id"],
                    pred["convergence_score"],
                    pred["mechanics"],
                    json.dumps(pred["enabling_qualities"]),
                    pred.get("actor_profile"),
                    pred.get("lifecycle_stage"),
                    pred.get("detection_difficulty"),
                    _now(),
                ),
            )
        self._safe_commit()

    def get_predictions(self, run_id: str) -> list[dict]:
        with self.conn.cursor(cursor_factory=psycopg2.extras.RealDictCursor) as cur:
            cur.execute(
                """SELECT pr.*, p.name as policy_name
                   FROM predictions pr JOIN policies p ON pr.policy_id = p.policy_id
                   WHERE pr.run_id=%s ORDER BY pr.convergence_score DESC""",
                (run_id,),
            )
            rows = cur.fetchall()
        return [dict(r) for r in rows]

    # ── Stage 6: Detection Patterns ─────────────────────────────────

    def insert_detection_pattern(self, run_id: str, pattern: dict):
        with self.conn.cursor() as cur:
            cur.execute(
                """INSERT INTO detection_patterns
                (pattern_id, run_id, prediction_id, data_source, anomaly_signal,
                 baseline, false_positive_risk, detection_latency, priority,
                 implementation_notes, created_at)
                VALUES (%s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s)
                ON CONFLICT (pattern_id) DO UPDATE SET
                    run_id = EXCLUDED.run_id,
                    prediction_id = EXCLUDED.prediction_id,
                    data_source = EXCLUDED.data_source,
                    anomaly_signal = EXCLUDED.anomaly_signal,
                    baseline = EXCLUDED.baseline,
                    false_positive_risk = EXCLUDED.false_positive_risk,
                    detection_latency = EXCLUDED.detection_latency,
                    priority = EXCLUDED.priority,
                    implementation_notes = EXCLUDED.implementation_notes,
                    created_at = EXCLUDED.created_at""",
                (
                    pattern["pattern_id"],
                    run_id,
                    pattern["prediction_id"],
                    pattern["data_source"],
                    pattern["anomaly_signal"],
                    pattern.get("baseline"),
                    pattern.get("false_positive_risk"),
                    pattern.get("detection_latency"),
                    pattern.get("priority"),
                    pattern.get("implementation_notes"),
                    _now(),
                ),
            )
        self._safe_commit()

    def get_detection_patterns(self, run_id: str) -> list[dict]:
        with self.conn.cursor(cursor_factory=psycopg2.extras.RealDictCursor) as cur:
            cur.execute(
                """SELECT dp.*, pr.mechanics as prediction_mechanics, p.name as policy_name
                   FROM detection_patterns dp
                   JOIN predictions pr ON dp.prediction_id = pr.prediction_id
                   JOIN policies p ON pr.policy_id = p.policy_id
                   WHERE dp.run_id=%s
                   ORDER BY dp.priority, dp.detection_latency""",
                (run_id,),
            )
            rows = cur.fetchall()
        return [dict(r) for r in rows]

    # ── RAG: Documents ──────────────────────────────────────────────

    def insert_document(
        self,
        doc_id: str,
        filename: str,
        doc_type: str,
        full_text: str,
        metadata: dict | None = None,
    ):
        with self.conn.cursor() as cur:
            cur.execute(
                """INSERT INTO documents
                (doc_id, filename, doc_type, full_text, metadata, created_at)
                VALUES (%s, %s, %s, %s, %s, %s)
                ON CONFLICT (doc_id) DO UPDATE SET
                    filename = EXCLUDED.filename,
                    doc_type = EXCLUDED.doc_type,
                    full_text = EXCLUDED.full_text,
                    metadata = EXCLUDED.metadata,
                    created_at = EXCLUDED.created_at""",
                (
                    doc_id,
                    filename,
                    doc_type,
                    full_text,
                    json.dumps(metadata) if metadata else None,
                    _now(),
                ),
            )
        self._safe_commit()

    def insert_chunk(
        self, chunk_id: str, doc_id: str, chunk_index: int, text: str, token_count: int
    ):
        with self.conn.cursor() as cur:
            cur.execute(
                """INSERT INTO chunks (chunk_id, doc_id, chunk_index, text, token_count)
                VALUES (%s, %s, %s, %s, %s)
                ON CONFLICT (chunk_id) DO UPDATE SET
                    doc_id = EXCLUDED.doc_id,
                    chunk_index = EXCLUDED.chunk_index,
                    text = EXCLUDED.text,
                    token_count = EXCLUDED.token_count""",
                (chunk_id, doc_id, chunk_index, text, token_count),
            )
        self._safe_commit()

    def search_chunks(self, query: str, doc_type: str | None = None, limit: int = 10) -> list[dict]:
        """Keyword-based chunk retrieval. Replace with vector search if embedding model is configured."""
        keywords = query.lower().split()
        where_clauses = ["1=1"]
        params = []
        if doc_type:
            where_clauses.append("d.doc_type = %s")
            params.append(doc_type)

        # Simple keyword scoring: count matching keywords per chunk
        with self.conn.cursor(cursor_factory=psycopg2.extras.RealDictCursor) as cur:
            cur.execute(
                f"""SELECT c.chunk_id, c.text, c.doc_id, d.filename, d.doc_type
                    FROM chunks c JOIN documents d ON c.doc_id = d.doc_id
                    WHERE {" AND ".join(where_clauses)}
                    ORDER BY c.chunk_index
                    LIMIT %s""",
                [*params, limit * 5],  # over-fetch then score
            )
            rows = cur.fetchall()

        scored = []
        for row in rows:
            text_lower = row["text"].lower()
            score = sum(1 for kw in keywords if kw in text_lower)
            if score > 0:
                scored.append((score, dict(row)))
        scored.sort(key=lambda x: -x[0])
        return [item for _, item in scored[:limit]]

    def get_all_documents(self, doc_type: str | None = None) -> list[dict]:
        with self.conn.cursor(cursor_factory=psycopg2.extras.RealDictCursor) as cur:
            if doc_type:
                cur.execute("SELECT * FROM documents WHERE doc_type=%s", (doc_type,))
            else:
                cur.execute("SELECT * FROM documents")
            rows = cur.fetchall()
        return [dict(r) for r in rows]

    # ── Step Functions: Task Tokens ─────────────────────────────────

    def store_task_token(self, run_id: str, stage: int, task_token: str):
        """Store a Step Functions task token for a human gate stage."""
        with self.conn.cursor() as cur:
            cur.execute(
                """UPDATE stage_log SET task_token = %s
                   WHERE id = (
                       SELECT id FROM stage_log
                       WHERE run_id = %s AND stage = %s
                       ORDER BY id DESC LIMIT 1
                   )""",
                (task_token, run_id, stage),
            )
        self._safe_commit()

    def get_task_token(self, run_id: str, stage: int) -> str | None:
        """Retrieve the task token for a human gate stage."""
        with self.conn.cursor(cursor_factory=psycopg2.extras.RealDictCursor) as cur:
            cur.execute(
                "SELECT task_token FROM stage_log "
                "WHERE run_id = %s AND stage = %s AND task_token IS NOT NULL "
                "ORDER BY id DESC LIMIT 1",
                (run_id, stage),
            )
            row = cur.fetchone()
        return row["task_token"] if row else None


    # ── Enforcement Sources ────────────────────────────────────────

    def upsert_enforcement_source(self, source: dict):
        with self.conn.cursor() as cur:
            cur.execute(
                """INSERT INTO enforcement_sources
                (source_id, name, url, source_type, description,
                 has_document, s3_key, doc_id, summary, validation_status,
                 created_at, updated_at)
                VALUES (%s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s)
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
                    updated_at = EXCLUDED.updated_at""",
                (
                    source["source_id"],
                    source["name"],
                    source.get("url"),
                    source.get("source_type", "press_release"),
                    source.get("description"),
                    source.get("has_document", False),
                    source.get("s3_key"),
                    source.get("doc_id"),
                    source.get("summary"),
                    source.get("validation_status", "pending"),
                    source.get("created_at", _now()),
                    _now(),
                ),
            )
        self._safe_commit()

    def get_enforcement_sources(self) -> list[dict]:
        with self.conn.cursor(cursor_factory=psycopg2.extras.RealDictCursor) as cur:
            cur.execute("SELECT * FROM enforcement_sources ORDER BY created_at")
            rows = cur.fetchall()
        return [dict(r) for r in rows]

    def get_enforcement_source(self, source_id: str) -> dict | None:
        with self.conn.cursor(cursor_factory=psycopg2.extras.RealDictCursor) as cur:
            cur.execute(
                "SELECT * FROM enforcement_sources WHERE source_id = %s",
                (source_id,),
            )
            row = cur.fetchone()
        return dict(row) if row else None

    def get_enforcement_source_by_url(self, url: str) -> dict | None:
        """Find an enforcement source by its URL."""
        with self.conn.cursor(cursor_factory=psycopg2.extras.RealDictCursor) as cur:
            cur.execute(
                "SELECT * FROM enforcement_sources WHERE url = %s",
                (url,),
            )
            row = cur.fetchone()
        return dict(row) if row else None

    def delete_enforcement_source(self, source_id: str):
        with self.conn.cursor() as cur:
            cur.execute(
                "DELETE FROM enforcement_sources WHERE source_id = %s",
                (source_id,),
            )
        self._safe_commit()

    def update_enforcement_source_document(
        self, source_id: str, s3_key: str, doc_id: str
    ):
        with self.conn.cursor() as cur:
            cur.execute(
                """UPDATE enforcement_sources
                   SET has_document = TRUE, s3_key = %s, doc_id = %s,
                       validation_status = 'pending', updated_at = %s
                   WHERE source_id = %s""",
                (s3_key, doc_id, _now(), source_id),
            )
        self._safe_commit()

    def update_enforcement_source_summary(
        self, source_id: str, summary: str, validation_status: str
    ):
        with self.conn.cursor() as cur:
            cur.execute(
                """UPDATE enforcement_sources
                   SET summary = %s, validation_status = %s, updated_at = %s
                   WHERE source_id = %s""",
                (summary, validation_status, _now(), source_id),
            )
        self._safe_commit()

    def seed_enforcement_sources_if_empty(self):
        with self.conn.cursor() as cur:
            cur.execute("SELECT COUNT(*) FROM enforcement_sources")
            count = cur.fetchone()[0]
        if count > 0:
            return False
        from pathlib import Path

        seed_path = Path(__file__).parent / "seed" / "enforcement_sources.json"
        with open(seed_path) as f:
            sources = json.load(f)
        for s in sources:
            self.upsert_enforcement_source(
                {
                    "source_id": s["id"],
                    "name": s["name"],
                    "url": s["url"],
                    "source_type": s.get("type", "press_release"),
                    "description": s.get("description", ""),
                }
            )
        return True

    # ── Dimension Registry ─────────────────────────────────────────

    def upsert_dimension(self, dim: dict):
        with self.conn.cursor() as cur:
            cur.execute(
                """INSERT INTO dimension_registry
                (dimension_id, name, definition, probing_questions, origin,
                 related_quality_ids, created_at, created_by)
                VALUES (%s, %s, %s, %s, %s, %s, %s, %s)
                ON CONFLICT (dimension_id) DO UPDATE SET
                    name = EXCLUDED.name,
                    definition = EXCLUDED.definition,
                    probing_questions = EXCLUDED.probing_questions,
                    origin = EXCLUDED.origin,
                    related_quality_ids = EXCLUDED.related_quality_ids,
                    created_by = EXCLUDED.created_by""",
                (
                    dim["dimension_id"],
                    dim["name"],
                    dim["definition"],
                    json.dumps(dim.get("probing_questions", [])),
                    dim.get("origin", "manual"),
                    json.dumps(dim.get("related_quality_ids", [])),
                    dim.get("created_at", _now()),
                    dim.get("created_by"),
                ),
            )
        self._safe_commit()

    def get_dimensions(self) -> list[dict]:
        with self.conn.cursor(cursor_factory=psycopg2.extras.RealDictCursor) as cur:
            cur.execute("SELECT * FROM dimension_registry ORDER BY name")
            rows = cur.fetchall()
        return [dict(r) for r in rows]

    def seed_dimensions_if_empty(self):
        with self.conn.cursor() as cur:
            cur.execute("SELECT COUNT(*) FROM dimension_registry")
            count = cur.fetchone()[0]
        if count > 0:
            return False
        from pathlib import Path

        seed_path = Path(__file__).parent / "seed" / "dimension_registry.json"
        if not seed_path.exists():
            return False
        with open(seed_path) as f:
            dims = json.load(f)
        for d in dims:
            self.upsert_dimension(d)
        return True

    # ── Structural Findings ────────────────────────────────────────

    def insert_structural_finding(self, run_id: str, finding: dict):
        with self.conn.cursor() as cur:
            cur.execute(
                """INSERT INTO structural_findings
                (finding_id, run_id, policy_id, dimension_id, observation,
                 source_type, source_citation, source_text, confidence,
                 status, stale_reason, created_at, created_by)
                VALUES (%s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s)
                ON CONFLICT (finding_id) DO UPDATE SET
                    observation = EXCLUDED.observation,
                    source_type = EXCLUDED.source_type,
                    source_citation = EXCLUDED.source_citation,
                    source_text = EXCLUDED.source_text,
                    confidence = EXCLUDED.confidence,
                    status = EXCLUDED.status,
                    stale_reason = EXCLUDED.stale_reason""",
                (
                    finding["finding_id"],
                    run_id,
                    finding["policy_id"],
                    finding.get("dimension_id"),
                    finding["observation"],
                    finding.get("source_type", "llm_knowledge"),
                    finding.get("source_citation"),
                    finding.get("source_text"),
                    finding.get("confidence", "medium"),
                    finding.get("status", "active"),
                    finding.get("stale_reason"),
                    _now(),
                    finding.get("created_by"),
                ),
            )
        self._safe_commit()

    def get_structural_findings(
        self, run_id: str, policy_id: str, status: str = "active"
    ) -> list[dict]:
        with self.conn.cursor(cursor_factory=psycopg2.extras.RealDictCursor) as cur:
            cur.execute(
                """SELECT sf.*, dr.name as dimension_name
                   FROM structural_findings sf
                   LEFT JOIN dimension_registry dr ON sf.dimension_id = dr.dimension_id
                   WHERE sf.run_id=%s AND sf.policy_id=%s AND sf.status=%s
                   ORDER BY sf.dimension_id, sf.created_at""",
                (run_id, policy_id, status),
            )
            rows = cur.fetchall()
        return [dict(r) for r in rows]

    def mark_finding_stale(self, finding_id: str, reason: str):
        with self.conn.cursor() as cur:
            cur.execute(
                "UPDATE structural_findings SET status='stale', stale_reason=%s WHERE finding_id=%s",
                (reason, finding_id),
            )
        self._safe_commit()

    # ── Quality Assessments ────────────────────────────────────────

    def upsert_quality_assessment(self, run_id: str, assessment: dict):
        with self.conn.cursor() as cur:
            cur.execute(
                """INSERT INTO quality_assessments
                (assessment_id, run_id, policy_id, quality_id, taxonomy_version,
                 present, evidence_finding_ids, confidence, rationale, created_at)
                VALUES (%s, %s, %s, %s, %s, %s, %s, %s, %s, %s)
                ON CONFLICT (run_id, policy_id, quality_id) DO UPDATE SET
                    assessment_id = EXCLUDED.assessment_id,
                    taxonomy_version = EXCLUDED.taxonomy_version,
                    present = EXCLUDED.present,
                    evidence_finding_ids = EXCLUDED.evidence_finding_ids,
                    confidence = EXCLUDED.confidence,
                    rationale = EXCLUDED.rationale,
                    created_at = EXCLUDED.created_at""",
                (
                    assessment["assessment_id"],
                    run_id,
                    assessment["policy_id"],
                    assessment["quality_id"],
                    assessment.get("taxonomy_version"),
                    assessment.get("present", "uncertain"),
                    json.dumps(assessment.get("evidence_finding_ids", [])),
                    assessment.get("confidence", "medium"),
                    assessment.get("rationale"),
                    _now(),
                ),
            )
        self._safe_commit()

    def get_quality_assessments(
        self, run_id: str, policy_id: str | None = None
    ) -> list[dict]:
        with self.conn.cursor(cursor_factory=psycopg2.extras.RealDictCursor) as cur:
            if policy_id:
                cur.execute(
                    "SELECT * FROM quality_assessments WHERE run_id=%s AND policy_id=%s ORDER BY quality_id",
                    (run_id, policy_id),
                )
            else:
                cur.execute(
                    "SELECT * FROM quality_assessments WHERE run_id=%s ORDER BY policy_id, quality_id",
                    (run_id,),
                )
            rows = cur.fetchall()
        return [dict(r) for r in rows]

    # ── Policy Lifecycle ───────────────────────────────────────────

    def update_policy_lifecycle(self, policy_id: str, status: str):
        with self.conn.cursor() as cur:
            cur.execute(
                "UPDATE policies SET lifecycle_status=%s, lifecycle_updated_at=%s WHERE policy_id=%s",
                (status, _now(), policy_id),
            )
        self._safe_commit()

    # ── Source Feeds ───────────────────────────────────────────────

    def upsert_source_feed(self, feed: dict):
        with self.conn.cursor() as cur:
            cur.execute(
                """INSERT INTO source_feeds
                (feed_id, name, listing_url, content_type, link_selector,
                 enabled, created_at, updated_at)
                VALUES (%s, %s, %s, %s, %s, %s, %s, %s)
                ON CONFLICT (feed_id) DO UPDATE SET
                    name = EXCLUDED.name,
                    listing_url = EXCLUDED.listing_url,
                    content_type = EXCLUDED.content_type,
                    link_selector = EXCLUDED.link_selector,
                    enabled = EXCLUDED.enabled,
                    updated_at = EXCLUDED.updated_at""",
                (
                    feed["feed_id"],
                    feed["name"],
                    feed["listing_url"],
                    feed.get("content_type", "press_release"),
                    feed.get("link_selector"),
                    feed.get("enabled", True),
                    feed.get("created_at", _now()),
                    _now(),
                ),
            )
        self._safe_commit()

    def get_source_feeds(self, enabled_only: bool = False) -> list[dict]:
        with self.conn.cursor(cursor_factory=psycopg2.extras.RealDictCursor) as cur:
            if enabled_only:
                cur.execute("SELECT * FROM source_feeds WHERE enabled = TRUE ORDER BY name")
            else:
                cur.execute("SELECT * FROM source_feeds ORDER BY name")
            rows = cur.fetchall()
        return [dict(r) for r in rows]

    def update_feed_last_checked(self, feed_id: str, last_entry_url: str | None = None):
        with self.conn.cursor() as cur:
            cur.execute(
                "UPDATE source_feeds SET last_checked_at=%s, last_entry_url=COALESCE(%s, last_entry_url), updated_at=%s WHERE feed_id=%s",
                (_now(), last_entry_url, _now(), feed_id),
            )
        self._safe_commit()

    def seed_source_feeds_if_empty(self):
        with self.conn.cursor() as cur:
            cur.execute("SELECT COUNT(*) FROM source_feeds")
            count = cur.fetchone()[0]
        if count > 0:
            return False
        from pathlib import Path

        seed_path = Path(__file__).parent / "seed" / "source_feeds.json"
        if not seed_path.exists():
            return False
        with open(seed_path) as f:
            feeds = json.load(f)
        for feed in feeds:
            self.upsert_source_feed(feed)
        return True

    # ── Source Candidates ──────────────────────────────────────────

    def insert_candidate(self, candidate: dict):
        with self.conn.cursor() as cur:
            cur.execute(
                """INSERT INTO source_candidates
                (candidate_id, feed_id, title, url, discovered_at, published_date,
                 status, richness_score, richness_rationale, estimated_cases,
                 source_id, doc_id, reviewed_by, created_at, updated_at)
                VALUES (%s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s)
                ON CONFLICT (candidate_id) DO UPDATE SET
                    title = EXCLUDED.title,
                    status = EXCLUDED.status,
                    richness_score = EXCLUDED.richness_score,
                    richness_rationale = EXCLUDED.richness_rationale,
                    estimated_cases = EXCLUDED.estimated_cases,
                    source_id = EXCLUDED.source_id,
                    doc_id = EXCLUDED.doc_id,
                    reviewed_by = EXCLUDED.reviewed_by,
                    updated_at = EXCLUDED.updated_at""",
                (
                    candidate["candidate_id"],
                    candidate.get("feed_id"),
                    candidate["title"],
                    candidate["url"],
                    candidate.get("discovered_at", _now()),
                    candidate.get("published_date"),
                    candidate.get("status", "discovered"),
                    candidate.get("richness_score"),
                    candidate.get("richness_rationale"),
                    candidate.get("estimated_cases"),
                    candidate.get("source_id"),
                    candidate.get("doc_id"),
                    candidate.get("reviewed_by", "auto"),
                    _now(),
                    _now(),
                ),
            )
        self._safe_commit()

    def get_candidate_by_url(self, url: str) -> dict | None:
        with self.conn.cursor(cursor_factory=psycopg2.extras.RealDictCursor) as cur:
            cur.execute("SELECT * FROM source_candidates WHERE url = %s", (url,))
            row = cur.fetchone()
        return dict(row) if row else None

    def get_candidates(
        self, feed_id: str | None = None, status: str | None = None
    ) -> list[dict]:
        clauses, params = ["1=1"], []
        if feed_id:
            clauses.append("feed_id = %s")
            params.append(feed_id)
        if status:
            clauses.append("status = %s")
            params.append(status)
        with self.conn.cursor(cursor_factory=psycopg2.extras.RealDictCursor) as cur:
            cur.execute(
                f"SELECT * FROM source_candidates WHERE {' AND '.join(clauses)} ORDER BY discovered_at DESC",
                params,
            )
            rows = cur.fetchall()
        return [dict(r) for r in rows]

    def update_candidate_status(self, candidate_id: str, status: str):
        with self.conn.cursor() as cur:
            cur.execute(
                "UPDATE source_candidates SET status=%s, updated_at=%s WHERE candidate_id=%s",
                (status, _now(), candidate_id),
            )
        self._safe_commit()

    def update_candidate_richness(
        self, candidate_id: str, score: float, rationale: str, estimated_cases: int
    ):
        with self.conn.cursor() as cur:
            cur.execute(
                """UPDATE source_candidates
                   SET richness_score=%s, richness_rationale=%s, estimated_cases=%s,
                       status='scored', updated_at=%s
                   WHERE candidate_id=%s""",
                (score, rationale, estimated_cases, _now(), candidate_id),
            )
        self._safe_commit()

    def update_candidate_ingested(self, candidate_id: str, source_id: str, doc_id: str):
        with self.conn.cursor() as cur:
            cur.execute(
                """UPDATE source_candidates
                   SET source_id=%s, doc_id=%s, status='ingested', updated_at=%s
                   WHERE candidate_id=%s""",
                (source_id, doc_id, _now(), candidate_id),
            )
        self._safe_commit()

    # ── Triage Results ─────────────────────────────────────────────

    def insert_triage_result(self, run_id: str, result: dict):
        with self.conn.cursor() as cur:
            cur.execute(
                """INSERT INTO triage_results
                (run_id, policy_id, triage_score, rationale, uncertainty, priority_rank, created_at)
                VALUES (%s, %s, %s, %s, %s, %s, %s)
                ON CONFLICT (run_id, policy_id) DO UPDATE SET
                    triage_score = EXCLUDED.triage_score,
                    rationale = EXCLUDED.rationale,
                    uncertainty = EXCLUDED.uncertainty,
                    priority_rank = EXCLUDED.priority_rank,
                    created_at = EXCLUDED.created_at""",
                (
                    run_id,
                    result["policy_id"],
                    result["triage_score"],
                    result["rationale"],
                    result.get("uncertainty"),
                    result["priority_rank"],
                    _now(),
                ),
            )
        self._safe_commit()

    def get_triage_results(self, run_id: str) -> list[dict]:
        with self.conn.cursor(cursor_factory=psycopg2.extras.RealDictCursor) as cur:
            cur.execute(
                """SELECT tr.*, p.name as policy_name
                   FROM triage_results tr
                   JOIN policies p ON tr.policy_id = p.policy_id
                   WHERE tr.run_id=%s
                   ORDER BY tr.priority_rank""",
                (run_id,),
            )
            rows = cur.fetchall()
        return [dict(r) for r in rows]

    # ── Research Sessions ──────────────────────────────────────────

    def create_research_session(
        self, run_id: str, policy_id: str, session_id: str, trigger: str = "initial"
    ) -> dict:
        session = {
            "session_id": session_id,
            "run_id": run_id,
            "policy_id": policy_id,
            "status": "pending",
            "trigger": trigger,
            "started_at": _now(),
        }
        with self.conn.cursor() as cur:
            cur.execute(
                """INSERT INTO research_sessions
                (session_id, run_id, policy_id, status, trigger, started_at)
                VALUES (%s, %s, %s, %s, %s, %s)
                ON CONFLICT (session_id) DO NOTHING""",
                (session_id, run_id, policy_id, "pending", trigger, _now()),
            )
        self._safe_commit()
        return session

    def update_research_session(
        self,
        session_id: str,
        status: str,
        error: str | None = None,
        sources_queried: list | None = None,
    ):
        with self.conn.cursor() as cur:
            completed = _now() if status in ("findings_complete", "assessment_complete", "failed") else None
            cur.execute(
                """UPDATE research_sessions
                   SET status=%s, error_message=%s, completed_at=COALESCE(%s, completed_at),
                       sources_queried=COALESCE(%s, sources_queried)
                   WHERE session_id=%s""",
                (
                    status,
                    error,
                    completed,
                    json.dumps(sources_queried) if sources_queried else None,
                    session_id,
                ),
            )
        self._safe_commit()

    def get_research_sessions(
        self, run_id: str, status: str | None = None
    ) -> list[dict]:
        with self.conn.cursor(cursor_factory=psycopg2.extras.RealDictCursor) as cur:
            if status:
                cur.execute(
                    "SELECT * FROM research_sessions WHERE run_id=%s AND status=%s ORDER BY started_at",
                    (run_id, status),
                )
            else:
                cur.execute(
                    "SELECT * FROM research_sessions WHERE run_id=%s ORDER BY started_at",
                    (run_id,),
                )
            rows = cur.fetchall()
        return [dict(r) for r in rows]

    # ── Regulatory Source Cache ─────────────────────────────────────

    def insert_regulatory_source(self, source: dict):
        with self.conn.cursor() as cur:
            cur.execute(
                """INSERT INTO regulatory_sources
                (source_id, source_type, url, title, cfr_reference, full_text, fetched_at, metadata)
                VALUES (%s, %s, %s, %s, %s, %s, %s, %s)
                ON CONFLICT (source_id) DO UPDATE SET
                    full_text = EXCLUDED.full_text,
                    fetched_at = EXCLUDED.fetched_at,
                    metadata = EXCLUDED.metadata""",
                (
                    source["source_id"],
                    source["source_type"],
                    source["url"],
                    source.get("title"),
                    source.get("cfr_reference"),
                    source["full_text"],
                    _now(),
                    json.dumps(source.get("metadata")) if source.get("metadata") else None,
                ),
            )
        self._safe_commit()

    def get_regulatory_source(self, source_id: str) -> dict | None:
        with self.conn.cursor(cursor_factory=psycopg2.extras.RealDictCursor) as cur:
            cur.execute("SELECT * FROM regulatory_sources WHERE source_id = %s", (source_id,))
            row = cur.fetchone()
        return dict(row) if row else None


def _now() -> str:
    return datetime.now(UTC).isoformat()
