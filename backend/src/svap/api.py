"""
SVAP API — Lambda handler for API Gateway V2 HTTP API.

Routes directly on event["routeKey"] without a web framework.
API Gateway handles CORS, JWT auth, and path parameter extraction.

Run locally:
    python -m svap.dev_server

Deployed as Lambda:
    handler = svap.api.handler
"""

import base64
import json
import logging
import os
import re
import traceback
from datetime import UTC, datetime

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
    SCANNED_PROGRAMS,
    flatten_policy_catalog,
    get_dashboard_data,
    get_data_sources_for_policy,
    get_policy_context,
)
from svap.storage import SVAPStorage

logger = logging.getLogger(__name__)
logger.setLevel(os.environ.get("LOG_LEVEL", "INFO"))

# ── Configuration ────────────────────────────────────────────────────────

DATABASE_URL = os.environ.get("DATABASE_URL", "postgresql://svap:password@localhost:5432/svap")
IS_LAMBDA = bool(os.environ.get("AWS_LAMBDA_FUNCTION_NAME"))
PIPELINE_STATE_MACHINE_ARN = os.environ.get("PIPELINE_STATE_MACHINE_ARN", "")
CONFIG_BUCKET = os.environ.get("SVAP_CONFIG_BUCKET", "")


# ── Helpers ──────────────────────────────────────────────────────────────

_DEFAULT_CONFIG = {
    "bedrock": {
        "region": "us-east-1",
        "model_id": "anthropic.claude-sonnet-4-20250514-v1:0",
        "max_tokens": 4096,
        "temperature": 0.2,
        "retry_attempts": 3,
        "retry_delay_seconds": 5,
    },
    "rag": {
        "chunk_size": 1500,
        "chunk_overlap": 200,
        "max_context_chunks": 10,
        "embedding_model": None,
    },
    "pipeline": {
        "human_gates": [2, 5],
        "max_concurrency": 5,
        "export_dir": "/tmp/results",
    },
}


def _get_config(overrides: dict | None = None) -> dict:
    """Load pipeline config from S3 (Lambda) or local file (dev)."""
    config = {}
    if CONFIG_BUCKET:
        try:
            import boto3
            import yaml

            obj = boto3.client("s3").get_object(Bucket=CONFIG_BUCKET, Key="config.yaml")
            config = yaml.safe_load(obj["Body"].read())
        except Exception:
            config = dict(_DEFAULT_CONFIG)
    else:
        try:
            from svap.orchestrator import _load_config

            config = _load_config("config.yaml")
        except FileNotFoundError:
            config = dict(_DEFAULT_CONFIG)

    if overrides:
        config.update(overrides)
    return config


class ApiError(Exception):
    """Raised by route handlers to return an error response."""

    def __init__(self, status_code: int, message: str):
        self.status_code = status_code
        self.message = message


def get_storage() -> SVAPStorage:
    """Get a storage instance."""
    return SVAPStorage(DATABASE_URL)


def get_active_run_id(storage: SVAPStorage) -> str:
    """Get the most recent run_id, or raise 404."""
    run_id = storage.get_latest_run()
    if not run_id:
        raise ApiError(404, "No pipeline runs found. Seed the pipeline first.")
    return run_id


def _json_body(event: dict) -> dict:
    """Parse JSON body from API Gateway V2 event."""
    body = event.get("body", "")
    if not body:
        return {}
    if event.get("isBase64Encoded"):
        import base64

        body = base64.b64decode(body).decode()
    return json.loads(body)


def _ok(body) -> dict:
    """200 response in API Gateway V2 format."""
    return {
        "statusCode": 200,
        "headers": {"Content-Type": "application/json"},
        "body": json.dumps(body, default=str),
    }


def _error(status_code: int, message: str) -> dict:
    """Error response in API Gateway V2 format."""
    return {
        "statusCode": status_code,
        "headers": {"Content-Type": "application/json"},
        "body": json.dumps({"detail": message}),
    }


# ── Route handlers ──────────────────────────────────────────────────────
# Each takes (event) and returns a Python object.
# The top-level handler() wraps the return with _ok().
# Raise ApiError for error responses.


def _dashboard(event):
    storage = get_storage()
    storage.seed_enforcement_sources_if_empty()
    run_id = storage.get_latest_run()
    if not run_id:
        return {
            "run_id": "",
            "source": "api",
            "pipeline_status": [],
            "counts": {
                "cases": 0,
                "taxonomy_qualities": 0,
                "policies": 0,
                "predictions": 0,
                "detection_patterns": 0,
            },
            "calibration": {"threshold": 3},
            "cases": [],
            "taxonomy": [],
            "policies": [],
            "predictions": [],
            "detection_patterns": [],
            "case_convergence": [],
            "policy_convergence": [],
            "policy_catalog": HHS_POLICY_CATALOG,
            "enforcement_sources": storage.get_enforcement_sources(),
            "data_sources": HHS_DATA_SOURCES,
            "scanned_programs": SCANNED_PROGRAMS,
        }
    return get_dashboard_data(storage, run_id)


def _status(event):
    storage = get_storage()
    run_id = get_active_run_id(storage)
    return {"run_id": run_id, "stages": storage.get_pipeline_status(run_id)}


def _list_cases(event):
    storage = get_storage()
    run_id = get_active_run_id(storage)
    return enrich_cases(storage.get_cases(run_id), storage.get_convergence_matrix(run_id))


def _get_case(event):
    case_id = event["pathParameters"]["case_id"]
    storage = get_storage()
    run_id = get_active_run_id(storage)
    enriched = enrich_cases(storage.get_cases(run_id), storage.get_convergence_matrix(run_id))
    case = next((c for c in enriched if c["case_id"] == case_id), None)
    if not case:
        raise ApiError(404, f"Case {case_id} not found")
    return case


def _list_taxonomy(event):
    storage = get_storage()
    run_id = get_active_run_id(storage)
    return enrich_taxonomy(storage.get_taxonomy(run_id), storage.get_convergence_matrix(run_id))


def _get_quality(event):
    quality_id = event["pathParameters"]["quality_id"]
    storage = get_storage()
    run_id = get_active_run_id(storage)
    enriched = enrich_taxonomy(storage.get_taxonomy(run_id), storage.get_convergence_matrix(run_id))
    quality = next((q for q in enriched if q["quality_id"] == quality_id), None)
    if not quality:
        raise ApiError(404, f"Quality {quality_id} not found")
    return quality


def _convergence_cases(event):
    storage = get_storage()
    run_id = get_active_run_id(storage)
    return {
        "matrix": storage.get_convergence_matrix(run_id),
        "calibration": storage.get_calibration(run_id),
    }


def _convergence_policies(event):
    storage = get_storage()
    run_id = get_active_run_id(storage)
    return {
        "scores": storage.get_policy_scores(run_id),
        "calibration": storage.get_calibration(run_id),
    }


def _list_policies(event):
    storage = get_storage()
    run_id = get_active_run_id(storage)
    return enrich_policies(
        storage.get_policies(run_id),
        storage.get_policy_scores(run_id),
        storage.get_calibration(run_id),
    )


def _get_policy(event):
    policy_id = event["pathParameters"]["policy_id"]
    storage = get_storage()
    run_id = get_active_run_id(storage)

    policies = storage.get_policies(run_id)
    scores = storage.get_policy_scores(run_id)
    calibration = storage.get_calibration(run_id)
    enriched = enrich_policies(policies, scores, calibration)

    policy = next((p for p in enriched if p["policy_id"] == policy_id), None)
    if not policy:
        raise ApiError(404, f"Policy {policy_id} not found")

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


def _list_predictions(event):
    storage = get_storage()
    run_id = get_active_run_id(storage)
    return enrich_predictions(storage.get_predictions(run_id))


def _list_detection_patterns(event):
    storage = get_storage()
    run_id = get_active_run_id(storage)
    return storage.get_detection_patterns(run_id)


def _policy_catalog(event):
    return HHS_POLICY_CATALOG


def _policy_catalog_flat(event):
    return flatten_policy_catalog()


def _enforcement_sources(event):
    return ENFORCEMENT_SOURCES


def _data_sources(event):
    return HHS_DATA_SOURCES


def _run_pipeline(event):
    body = _json_body(event)
    storage = get_storage()
    run_id = f"run_{datetime.now(UTC).strftime('%Y%m%d_%H%M%S')}"

    config = _get_config(body.get("config_overrides"))
    storage.create_run(run_id, config=config, notes=body.get("notes", ""))

    if IS_LAMBDA and PIPELINE_STATE_MACHINE_ARN:
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

    return {"status": "started", "run_id": run_id}


def _approve_stage(event):
    body = _json_body(event)
    stage = body.get("stage")
    if stage not in (2, 5):
        raise ApiError(400, "Only stages 2 and 5 have human review gates.")

    storage = get_storage()
    run_id = get_active_run_id(storage)
    status = storage.get_stage_status(run_id, stage)

    if status != "pending_review":
        raise ApiError(400, f"Stage {stage} is '{status}', not pending review.")

    storage.approve_stage(run_id, stage)

    if IS_LAMBDA and PIPELINE_STATE_MACHINE_ARN:
        import boto3

        task_token = storage.get_task_token(run_id, stage)
        if task_token:
            sfn = boto3.client("stepfunctions")
            sfn.send_task_success(
                taskToken=task_token,
                output=json.dumps({"approved": True, "stage": stage}),
            )

    return {"status": "approved", "stage": stage}


def _seed_pipeline(event):
    from svap.orchestrator import _seed

    storage = get_storage()
    result = _seed(storage)
    return {"status": "seeded", **result}


def _health(event):
    return {"status": "ok", "database": "postgresql", "lambda": IS_LAMBDA}


# ── Enforcement source management ────────────────────────────────────────


def _list_enforcement_sources(event):
    storage = get_storage()
    storage.seed_enforcement_sources_if_empty()
    return storage.get_enforcement_sources()


def _create_enforcement_source(event):
    body = _json_body(event)
    name = body.get("name", "").strip()
    if not name:
        raise ApiError(400, "Missing required field: name")

    source_id = body.get("source_id") or re.sub(r"[^a-z0-9_]", "", name.lower().replace(" ", "_"))[:50]
    storage = get_storage()
    if storage.get_enforcement_source(source_id):
        raise ApiError(409, f"Source '{source_id}' already exists")

    storage.upsert_enforcement_source(
        {
            "source_id": source_id,
            "name": name,
            "url": body.get("url"),
            "source_type": body.get("source_type", "press_release"),
            "description": body.get("description", ""),
        }
    )
    return storage.get_enforcement_source(source_id)


def _upload_enforcement_document(event):
    body = _json_body(event)
    source_id = body.get("source_id")
    filename = body.get("filename")
    content_b64 = body.get("content")

    if not all([source_id, filename, content_b64]):
        raise ApiError(400, "Missing source_id, filename, or content")

    storage = get_storage()
    source = storage.get_enforcement_source(source_id)
    if not source:
        raise ApiError(404, f"Source '{source_id}' not found")

    file_bytes = base64.b64decode(content_b64)

    # Extract text based on file type
    from svap.stages.stage0_source_fetch import _extract_text

    lower_name = filename.lower()
    if lower_name.endswith((".html", ".htm")):
        text = _extract_text(file_bytes.decode("utf-8", errors="replace"))
    else:
        text = file_bytes.decode("utf-8", errors="replace")

    if len(text) < 100:
        raise ApiError(400, "Extracted text too short — document may be empty or unreadable")

    # Store to S3
    s3_key = f"enforcement-sources/{source_id}/{filename}"
    _upload_to_s3(s3_key, file_bytes, "application/octet-stream")

    # Ingest into RAG store
    from svap.rag import DocumentIngester

    config = _get_config()
    ingester = DocumentIngester(storage, config)
    doc_id, n_chunks = ingester.ingest_text(
        text=text,
        filename=source_id,
        doc_type="enforcement",
        metadata={"source_id": source_id, "original_filename": filename, "s3_key": s3_key},
    )

    storage.update_enforcement_source_document(source_id, s3_key=s3_key, doc_id=doc_id)

    return {
        "source_id": source_id,
        "doc_id": doc_id,
        "chunks": n_chunks,
        "s3_key": s3_key,
        "text_length": len(text),
    }


def _delete_enforcement_source(event):
    body = _json_body(event)
    source_id = body.get("source_id")
    if not source_id:
        raise ApiError(400, "Missing source_id")

    storage = get_storage()
    if not storage.get_enforcement_source(source_id):
        raise ApiError(404, f"Source '{source_id}' not found")

    storage.delete_enforcement_source(source_id)
    return {"status": "deleted", "source_id": source_id}


def _upload_to_s3(key: str, body: bytes, content_type: str):
    import boto3

    bucket = CONFIG_BUCKET
    if not bucket:
        logger.warning("SVAP_CONFIG_BUCKET not set, skipping S3 upload")
        return
    boto3.client("s3").put_object(Bucket=bucket, Key=key, Body=body, ContentType=content_type)


# ── Route table ─────────────────────────────────────────────────────────
# Keys are the exact routeKey strings from API Gateway V2, matching the
# routes list in infrastructure/terraform/svap.tf.

ROUTES = {
    "GET /api/dashboard": _dashboard,
    "GET /api/status": _status,
    "GET /api/cases": _list_cases,
    "GET /api/cases/{case_id}": _get_case,
    "GET /api/taxonomy": _list_taxonomy,
    "GET /api/taxonomy/{quality_id}": _get_quality,
    "GET /api/convergence/cases": _convergence_cases,
    "GET /api/convergence/policies": _convergence_policies,
    "GET /api/policies": _list_policies,
    "GET /api/policies/{policy_id}": _get_policy,
    "GET /api/predictions": _list_predictions,
    "GET /api/detection-patterns": _list_detection_patterns,
    "GET /api/hhs/policy-catalog": _policy_catalog,
    "GET /api/hhs/policy-catalog/flat": _policy_catalog_flat,
    "GET /api/hhs/enforcement-sources": _enforcement_sources,
    "GET /api/hhs/data-sources": _data_sources,
    "GET /api/enforcement-sources": _list_enforcement_sources,
    "POST /api/enforcement-sources": _create_enforcement_source,
    "POST /api/enforcement-sources/upload": _upload_enforcement_document,
    "POST /api/enforcement-sources/delete": _delete_enforcement_source,
    "POST /api/pipeline/run": _run_pipeline,
    "POST /api/pipeline/approve": _approve_stage,
    "POST /api/pipeline/seed": _seed_pipeline,
    "GET /api/health": _health,
}


# ── Lambda entry point ──────────────────────────────────────────────────


def handler(event, context):
    """Lambda handler for API Gateway V2 HTTP API (payload format 2.0)."""
    route_key = event.get("routeKey", "")
    logger.info("Request: %s", route_key)

    route_fn = ROUTES.get(route_key)
    if not route_fn:
        return _error(404, f"Not found: {route_key}")

    try:
        return _ok(route_fn(event))
    except ApiError as e:
        return _error(e.status_code, e.message)
    except Exception:
        logger.error("Unhandled error on %s:\n%s", route_key, traceback.format_exc())
        return _error(500, "Internal server error")
