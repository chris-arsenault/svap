"""
Storage layer for SVAP pipeline.

All intermediate outputs are stored in PostgreSQL. Each stage reads its inputs
from the database and writes its outputs back. This enables:
  - Resumability: re-run any stage without re-running predecessors
  - Auditability: every intermediate result is preserved
  - Scalability: PostgreSQL handles concurrent access and large datasets
"""

import json
from datetime import UTC, datetime

import psycopg2
import psycopg2.extras

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
    # Stage 1: Enforcement cases (extracted from documents)
    """CREATE TABLE IF NOT EXISTS cases (
        case_id             TEXT PRIMARY KEY,
        run_id              TEXT NOT NULL,
        source_document     TEXT,
        case_name           TEXT NOT NULL,
        scheme_mechanics    TEXT NOT NULL,
        exploited_policy    TEXT NOT NULL,
        enabling_condition  TEXT NOT NULL,
        scale_dollars       REAL,
        scale_defendants    INTEGER,
        scale_duration      TEXT,
        detection_method    TEXT,
        raw_extraction      TEXT,
        created_at          TEXT NOT NULL,
        FOREIGN KEY (run_id) REFERENCES pipeline_runs(run_id)
    )""",
    # Stage 2: Vulnerability taxonomy
    """CREATE TABLE IF NOT EXISTS taxonomy (
        quality_id          TEXT PRIMARY KEY,
        run_id              TEXT NOT NULL,
        name                TEXT NOT NULL,
        definition          TEXT NOT NULL,
        recognition_test    TEXT NOT NULL,
        exploitation_logic  TEXT NOT NULL,
        canonical_examples  TEXT,
        review_status       TEXT DEFAULT 'draft' CHECK(review_status IN ('draft','approved','rejected','revised')),
        reviewer_notes      TEXT,
        created_at          TEXT NOT NULL,
        FOREIGN KEY (run_id) REFERENCES pipeline_runs(run_id)
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
    # Stage 4: Policies to scan
    """CREATE TABLE IF NOT EXISTS policies (
        policy_id           TEXT PRIMARY KEY,
        run_id              TEXT NOT NULL,
        name                TEXT NOT NULL,
        description         TEXT,
        source_document     TEXT,
        structural_characterization TEXT,
        created_at          TEXT NOT NULL,
        FOREIGN KEY (run_id) REFERENCES pipeline_runs(run_id)
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
    # Unique indexes for upsert conflict targets
    "CREATE UNIQUE INDEX IF NOT EXISTS uq_convergence ON convergence_scores(run_id, case_id, quality_id)",
    "CREATE UNIQUE INDEX IF NOT EXISTS uq_policy_score ON policy_scores(run_id, policy_id, quality_id)",
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
                "SELECT stage, status, started_at, completed_at, error_message "
                "FROM stage_log WHERE run_id=%s ORDER BY stage, id DESC",
                (run_id,),
            )
            rows = cur.fetchall()
        return [dict(r) for r in rows]

    # ── Stage 1: Cases ──────────────────────────────────────────────

    def insert_case(self, run_id: str, case: dict):
        with self.conn.cursor() as cur:
            cur.execute(
                """INSERT INTO cases
                (case_id, run_id, source_document, case_name, scheme_mechanics,
                 exploited_policy, enabling_condition, scale_dollars, scale_defendants,
                 scale_duration, detection_method, raw_extraction, created_at)
                VALUES (%s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s)
                ON CONFLICT (case_id) DO UPDATE SET
                    run_id = EXCLUDED.run_id,
                    source_document = EXCLUDED.source_document,
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
                    run_id,
                    case.get("source_document"),
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

    def get_cases(self, run_id: str) -> list[dict]:
        with self.conn.cursor(cursor_factory=psycopg2.extras.RealDictCursor) as cur:
            cur.execute("SELECT * FROM cases WHERE run_id=%s", (run_id,))
            rows = cur.fetchall()
        return [dict(r) for r in rows]

    # ── Stage 2: Taxonomy ───────────────────────────────────────────

    def insert_quality(self, run_id: str, quality: dict):
        with self.conn.cursor() as cur:
            cur.execute(
                """INSERT INTO taxonomy
                (quality_id, run_id, name, definition, recognition_test,
                 exploitation_logic, canonical_examples, review_status, created_at)
                VALUES (%s, %s, %s, %s, %s, %s, %s, %s, %s)
                ON CONFLICT (quality_id) DO UPDATE SET
                    run_id = EXCLUDED.run_id,
                    name = EXCLUDED.name,
                    definition = EXCLUDED.definition,
                    recognition_test = EXCLUDED.recognition_test,
                    exploitation_logic = EXCLUDED.exploitation_logic,
                    canonical_examples = EXCLUDED.canonical_examples,
                    review_status = EXCLUDED.review_status,
                    created_at = EXCLUDED.created_at""",
                (
                    quality["quality_id"],
                    run_id,
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

    def get_taxonomy(self, run_id: str) -> list[dict]:
        with self.conn.cursor(cursor_factory=psycopg2.extras.RealDictCursor) as cur:
            cur.execute("SELECT * FROM taxonomy WHERE run_id=%s ORDER BY quality_id", (run_id,))
            rows = cur.fetchall()
        return [dict(r) for r in rows]

    def update_quality_review(self, quality_id: str, status: str, notes: str = ""):
        with self.conn.cursor() as cur:
            cur.execute(
                "UPDATE taxonomy SET review_status=%s, reviewer_notes=%s WHERE quality_id=%s",
                (status, notes, quality_id),
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

    def insert_policy(self, run_id: str, policy: dict):
        with self.conn.cursor() as cur:
            cur.execute(
                """INSERT INTO policies
                (policy_id, run_id, name, description, source_document,
                 structural_characterization, created_at)
                VALUES (%s, %s, %s, %s, %s, %s, %s)
                ON CONFLICT (policy_id) DO UPDATE SET
                    run_id = EXCLUDED.run_id,
                    name = EXCLUDED.name,
                    description = EXCLUDED.description,
                    source_document = EXCLUDED.source_document,
                    structural_characterization = EXCLUDED.structural_characterization,
                    created_at = EXCLUDED.created_at""",
                (
                    policy["policy_id"],
                    run_id,
                    policy["name"],
                    policy.get("description"),
                    policy.get("source_document"),
                    policy.get("structural_characterization"),
                    _now(),
                ),
            )
        self._safe_commit()

    def get_policies(self, run_id: str) -> list[dict]:
        with self.conn.cursor(cursor_factory=psycopg2.extras.RealDictCursor) as cur:
            cur.execute("SELECT * FROM policies WHERE run_id=%s", (run_id,))
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
                """INSERT INTO documents (doc_id, filename, doc_type, full_text, metadata, created_at)
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


def _now() -> str:
    return datetime.now(UTC).isoformat()
