"""
SVAP API Server

FastAPI application that wraps the SVAP pipeline backend and serves
structured data to the React UI. This is the bridge between the Python
pipeline (SQLite storage, Bedrock client, stage modules) and the
browser-based workstation.

Run:
    uvicorn svap.api:app --reload --port 8000

The React dev server proxies /api/* to this server.
"""

import json
import os
from pathlib import Path
from typing import Optional

from fastapi import FastAPI, HTTPException, BackgroundTasks
from fastapi.middleware.cors import CORSMiddleware
from pydantic import BaseModel

from svap.storage import SVAPStorage
from svap.hhs_extension import (
    get_dashboard_data,
    flatten_policy_catalog,
    get_policy_context,
    get_data_sources_for_policy,
    HHS_POLICY_CATALOG,
    ENFORCEMENT_SOURCES,
    HHS_DATA_SOURCES,
)

# ── App setup ─────────────────────────────────────────────────────────

app = FastAPI(
    title="SVAP API",
    description="Structural Vulnerability Analysis Pipeline — HHS OIG Workstation Backend",
    version="1.0.0",
)

app.add_middleware(
    CORSMiddleware,
    allow_origins=["http://localhost:5173", "http://localhost:3000"],
    allow_methods=["*"],
    allow_headers=["*"],
)

# ── Configuration ─────────────────────────────────────────────────────

CONFIG_PATH = os.environ.get("SVAP_CONFIG", "config.yaml")
DB_PATH = os.environ.get("SVAP_DB", "svap_data.db")


def get_storage() -> SVAPStorage:
    """Get a storage instance. Creates the DB if it doesn't exist."""
    return SVAPStorage(DB_PATH)


def get_active_run_id(storage: SVAPStorage) -> str:
    """Get the most recent run_id, or raise 404."""
    run_id = storage.get_latest_run_id()
    if not run_id:
        raise HTTPException(404, "No pipeline runs found. Run 'python -m svap.orchestrator seed' first.")
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
    return storage.get_cases(run_id)


@app.get("/api/cases/{case_id}")
def get_case(case_id: str):
    storage = get_storage()
    run_id = get_active_run_id(storage)
    cases = storage.get_cases(run_id)
    case = next((c for c in cases if c["case_id"] == case_id), None)
    if not case:
        raise HTTPException(404, f"Case {case_id} not found")
    return case


# ── Taxonomy ──────────────────────────────────────────────────────────

@app.get("/api/taxonomy")
def list_taxonomy():
    storage = get_storage()
    run_id = get_active_run_id(storage)
    return storage.get_taxonomy(run_id)


@app.get("/api/taxonomy/{quality_id}")
def get_quality(quality_id: str):
    storage = get_storage()
    run_id = get_active_run_id(storage)
    taxonomy = storage.get_taxonomy(run_id)
    quality = next((q for q in taxonomy if q["quality_id"] == quality_id), None)
    if not quality:
        raise HTTPException(404, f"Quality {quality_id} not found")
    return quality


# ── Convergence ───────────────────────────────────────────────────────

@app.get("/api/convergence/cases")
def convergence_cases():
    """Convergence matrix: cases × qualities."""
    storage = get_storage()
    run_id = get_active_run_id(storage)
    return {
        "matrix": storage.get_convergence_matrix(run_id),
        "calibration": storage.get_calibration(run_id),
    }


@app.get("/api/convergence/policies")
def convergence_policies():
    """Convergence matrix: policies × qualities."""
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
    return storage.get_policies(run_id)


@app.get("/api/policies/{policy_id}")
def get_policy(policy_id: str):
    storage = get_storage()
    run_id = get_active_run_id(storage)
    policies = storage.get_policies(run_id)
    policy = next((p for p in policies if p["policy_id"] == policy_id), None)
    if not policy:
        raise HTTPException(404, f"Policy {policy_id} not found")

    # Enrich with scores and predictions
    scores = storage.get_policy_scores(run_id)
    predictions = storage.get_predictions(run_id)
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
    return storage.get_predictions(run_id)


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

class RunStageRequest(BaseModel):
    stage: int
    config_overrides: Optional[dict] = None


@app.post("/api/pipeline/run")
def run_stage(req: RunStageRequest, background_tasks: BackgroundTasks):
    """
    Trigger a pipeline stage to run in the background.

    The React UI calls this, then polls /api/status to track progress.
    Stages 2 and 5 will complete in 'pending_review' status.
    """
    storage = get_storage()
    run_id = get_active_run_id(storage)

    # Import here to avoid circular deps and allow lazy Bedrock init
    from svap.orchestrator import _run_stage, _load_config

    config = _load_config(CONFIG_PATH)
    if req.config_overrides:
        config.update(req.config_overrides)

    background_tasks.add_task(_run_stage, storage, run_id, req.stage, config)

    return {"status": "started", "run_id": run_id, "stage": req.stage}


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
    return {"status": "ok", "db_path": DB_PATH}
