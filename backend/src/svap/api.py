"""
SVAP API Server

FastAPI application that wraps the SVAP pipeline backend and serves
structured data to the React UI. This is the bridge between the Python
pipeline (PostgreSQL storage, Bedrock client, stage modules) and the
browser-based workstation.

Run locally:
    uvicorn svap.api:app --reload --port 8000

Deployed as Lambda:
    handler = Mangum(app) (at bottom of file)

The React dev server proxies /api/* to this server.
"""

import json
import os
from datetime import UTC, datetime

from fastapi import BackgroundTasks, FastAPI, HTTPException
from pydantic import BaseModel

from svap.api_schemas import (
    enrich_cases,
    enrich_policies,
    enrich_predictions,
    enrich_taxonomy,
)
from svap.hhs_data import (
    ENFORCEMENT_SOURCES,
    HHS_DATA_SOURCES,
    HHS_POLICY_CATALOG,
    flatten_policy_catalog,
    get_dashboard_data,
    get_data_sources_for_policy,
    get_policy_context,
)
from svap.storage import SVAPStorage

# ── App setup ─────────────────────────────────────────────────────────

app = FastAPI(
    title="SVAP API",
    description="Structural Vulnerability Analysis Pipeline — HHS OIG Workstation Backend",
    version="0.2.0",
)

# CORS is handled by API Gateway (cors_configuration block).
# Do NOT add CORSMiddleware here — it conflicts with the gateway headers.

# ── Configuration ─────────────────────────────────────────────────────

CONFIG_PATH = os.environ.get("SVAP_CONFIG", "config.yaml")
DATABASE_URL = os.environ.get("DATABASE_URL", "postgresql://svap:password@localhost:5432/svap")
IS_LAMBDA = bool(os.environ.get("AWS_LAMBDA_FUNCTION_NAME"))
PIPELINE_STATE_MACHINE_ARN = os.environ.get("PIPELINE_STATE_MACHINE_ARN", "")


def get_storage() -> SVAPStorage:
    """Get a storage instance. Creates the DB if it doesn't exist."""
    return SVAPStorage(DATABASE_URL)


def get_active_run_id(storage: SVAPStorage) -> str:
    """Get the most recent run_id, or raise 404."""
    run_id = storage.get_latest_run()
    if not run_id:
        raise HTTPException(
            404, "No pipeline runs found. Run 'python -m svap.orchestrator seed' first."
        )
    return run_id


# ── Dashboard / Overview ──────────────────────────────────────────────


@app.get("/api/dashboard")
def dashboard():
    """
    Full dashboard payload: pipeline status, counts, cases, taxonomy,
    policies, predictions, detection patterns, convergence data.

    This is the primary data endpoint — the React app calls this on load
    and populates all views from the response.
    """
    storage = get_storage()
    run_id = get_active_run_id(storage)
    return get_dashboard_data(storage, run_id)


@app.get("/api/status")
def pipeline_status():
    """Lightweight status check — stage completion states only."""
    storage = get_storage()
    run_id = get_active_run_id(storage)
    return {
        "run_id": run_id,
        "stages": storage.get_pipeline_status(run_id),
    }


# ── Cases ─────────────────────────────────────────────────────────────


@app.get("/api/cases")
def list_cases():
    storage = get_storage()
    run_id = get_active_run_id(storage)
    cases = storage.get_cases(run_id)
    matrix = storage.get_convergence_matrix(run_id)
    return enrich_cases(cases, matrix)


@app.get("/api/cases/{case_id}")
def get_case(case_id: str):
    storage = get_storage()
    run_id = get_active_run_id(storage)
    cases = storage.get_cases(run_id)
    matrix = storage.get_convergence_matrix(run_id)
    enriched = enrich_cases(cases, matrix)
    case = next((c for c in enriched if c["case_id"] == case_id), None)
    if not case:
        raise HTTPException(404, f"Case {case_id} not found")
    return case


# ── Taxonomy ──────────────────────────────────────────────────────────


@app.get("/api/taxonomy")
def list_taxonomy():
    storage = get_storage()
    run_id = get_active_run_id(storage)
    taxonomy = storage.get_taxonomy(run_id)
    matrix = storage.get_convergence_matrix(run_id)
    return enrich_taxonomy(taxonomy, matrix)


@app.get("/api/taxonomy/{quality_id}")
def get_quality(quality_id: str):
    storage = get_storage()
    run_id = get_active_run_id(storage)
    taxonomy = storage.get_taxonomy(run_id)
    matrix = storage.get_convergence_matrix(run_id)
    enriched = enrich_taxonomy(taxonomy, matrix)
    quality = next((q for q in enriched if q["quality_id"] == quality_id), None)
    if not quality:
        raise HTTPException(404, f"Quality {quality_id} not found")
    return quality


# ── Convergence ───────────────────────────────────────────────────────


@app.get("/api/convergence/cases")
def convergence_cases():
    """Convergence matrix: cases x qualities."""
    storage = get_storage()
    run_id = get_active_run_id(storage)
    return {
        "matrix": storage.get_convergence_matrix(run_id),
        "calibration": storage.get_calibration(run_id),
    }


@app.get("/api/convergence/policies")
def convergence_policies():
    """Convergence matrix: policies x qualities."""
    storage = get_storage()
    run_id = get_active_run_id(storage)
    return {
        "scores": storage.get_policy_scores(run_id),
        "calibration": storage.get_calibration(run_id),
    }


# ── Policies ──────────────────────────────────────────────────────────


@app.get("/api/policies")
def list_policies():
    storage = get_storage()
    run_id = get_active_run_id(storage)
    policies = storage.get_policies(run_id)
    scores = storage.get_policy_scores(run_id)
    calibration = storage.get_calibration(run_id)
    return enrich_policies(policies, scores, calibration)


@app.get("/api/policies/{policy_id}")
def get_policy(policy_id: str):
    storage = get_storage()
    run_id = get_active_run_id(storage)
    policies = storage.get_policies(run_id)
    scores = storage.get_policy_scores(run_id)
    calibration = storage.get_calibration(run_id)
    enriched = enrich_policies(policies, scores, calibration)

    policy = next((p for p in enriched if p["policy_id"] == policy_id), None)
    if not policy:
        raise HTTPException(404, f"Policy {policy_id} not found")

    # Add detail-level data: per-score rows, predictions, patterns
    predictions = enrich_predictions(storage.get_predictions(run_id))
    patterns = storage.get_detection_patterns(run_id)

    policy_scores = [s for s in scores if s["policy_id"] == policy_id]
    policy_predictions = [p for p in predictions if p["policy_id"] == policy_id]
    pred_ids = {p["prediction_id"] for p in policy_predictions}
    policy_patterns = [d for d in patterns if d["prediction_id"] in pred_ids]

    return {
        **policy,
        "scores": policy_scores,
        "predictions": policy_predictions,
        "detection_patterns": policy_patterns,
        "context": get_policy_context(policy.get("name", "")),
        "data_sources": get_data_sources_for_policy(policy.get("name", "")),
    }


# ── Predictions ───────────────────────────────────────────────────────


@app.get("/api/predictions")
def list_predictions():
    storage = get_storage()
    run_id = get_active_run_id(storage)
    return enrich_predictions(storage.get_predictions(run_id))


# ── Detection Patterns ────────────────────────────────────────────────


@app.get("/api/detection-patterns")
def list_detection_patterns():
    storage = get_storage()
    run_id = get_active_run_id(storage)
    return storage.get_detection_patterns(run_id)


# ── HHS Reference Data (static, no pipeline run needed) ──────────────


@app.get("/api/hhs/policy-catalog")
def policy_catalog():
    return HHS_POLICY_CATALOG


@app.get("/api/hhs/policy-catalog/flat")
def policy_catalog_flat():
    return flatten_policy_catalog()


@app.get("/api/hhs/enforcement-sources")
def enforcement_sources():
    return ENFORCEMENT_SOURCES


@app.get("/api/hhs/data-sources")
def data_sources():
    return HHS_DATA_SOURCES


# ── Pipeline Operations (trigger stages) ──────────────────────────────


class RunPipelineRequest(BaseModel):
    config_overrides: dict | None = None
    notes: str = ""


@app.post("/api/pipeline/run")
def run_pipeline(req: RunPipelineRequest, background_tasks: BackgroundTasks):
    """
    Start a full pipeline run.

    In Lambda: starts a Step Functions execution that drives all stages.
    Locally: creates the run and kicks off stage 1 via BackgroundTasks.

    The React UI calls this, then polls /api/status to track progress.
    """
    storage = get_storage()
    run_id = f"run_{datetime.now(UTC).strftime('%Y%m%d_%H%M%S')}"

    from svap.orchestrator import _load_config

    config = _load_config(CONFIG_PATH)
    if req.config_overrides:
        config.update(req.config_overrides)
    storage.create_run(run_id, config=config, notes=req.notes or "")

    if IS_LAMBDA and PIPELINE_STATE_MACHINE_ARN:
        # In Lambda: start Step Functions execution
        import boto3

        sfn = boto3.client("stepfunctions")
        response = sfn.start_execution(
            stateMachineArn=PIPELINE_STATE_MACHINE_ARN,
            name=run_id,
            input=json.dumps({"run_id": run_id}),
        )
        return {
            "status": "started",
            "run_id": run_id,
            "execution_arn": response["executionArn"],
        }
    else:
        # Local dev: run stage 1 as a BackgroundTask
        from svap.orchestrator import _run_stage

        background_tasks.add_task(_run_stage, storage, run_id, 1, config)

    return {"status": "started", "run_id": run_id}


class ApproveRequest(BaseModel):
    stage: int


@app.post("/api/pipeline/approve")
def approve_stage(req: ApproveRequest):
    """Approve a human-gated stage (2 or 5) to allow downstream stages to proceed."""
    if req.stage not in (2, 5):
        raise HTTPException(400, "Only stages 2 and 5 have human review gates.")

    storage = get_storage()
    run_id = get_active_run_id(storage)
    status = storage.get_stage_status(run_id, req.stage)

    if status != "pending_review":
        raise HTTPException(400, f"Stage {req.stage} is '{status}', not pending review.")

    storage.approve_stage(run_id, req.stage)

    if IS_LAMBDA and PIPELINE_STATE_MACHINE_ARN:
        # Resume the Step Functions execution waiting on this approval
        import boto3

        task_token = storage.get_task_token(run_id, req.stage)
        if task_token:
            sfn = boto3.client("stepfunctions")
            sfn.send_task_success(
                taskToken=task_token,
                output=json.dumps({"approved": True, "stage": req.stage}),
            )

    return {"status": "approved", "stage": req.stage}


@app.post("/api/pipeline/seed")
def seed_pipeline():
    """Load seed data (HHS OIG enforcement cases, taxonomy, policies)."""
    from svap.orchestrator import _seed

    storage = get_storage()
    result = _seed(storage)
    return {"status": "seeded", **result}


# ── Health check ──────────────────────────────────────────────────────


@app.get("/api/health")
def health():
    return {"status": "ok", "database": "postgresql", "lambda": IS_LAMBDA}


# ── Lambda entry point ────────────────────────────────────────────────

try:
    from mangum import Mangum

    handler = Mangum(app, lifespan="off")
except ImportError:
    # Mangum not installed (local dev without Lambda deps)
    pass
