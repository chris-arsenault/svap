"""
Lambda handler for running individual pipeline stages.

Invoked by AWS Step Functions in two modes:

1. **Gate mode** (``"gate": true`` + ``"task_token"`` in payload):
   Stores the Step Functions task token in the database and marks the
   stage as pending_review, then returns immediately so the execution
   pauses until a human approves or rejects the gate.

2. **Stage mode** (``"stage": N``, no gate flag):
   Runs the actual LLM pipeline stage via ``_run_stage`` and returns
   the result so the state machine can advance.

Step Functions ``arn:aws:states:::lambda:invoke`` wraps the original
payload under a ``Payload`` key -- this handler unwraps it
transparently.

Event shape (after unwrapping):
    {
        "run_id": "seed_20240101_120000",
        "stage": 3,
        "config_overrides": {},       // optional

        // gate mode only:
        "gate": true,
        "task_token": "AQC..."
    }
"""

import logging
import os
import traceback

import boto3
import yaml

from svap.orchestrator import _load_config, _run_stage
from svap.storage import SVAPStorage

logger = logging.getLogger(__name__)
logger.setLevel(os.environ.get("LOG_LEVEL", "INFO"))

DATABASE_URL = os.environ.get("DATABASE_URL", "")
CONFIG_BUCKET = os.environ.get("SVAP_CONFIG_BUCKET", "")


def handler(event, context):
    """Lambda entry point for pipeline stage execution."""

    # Step Functions wraps the payload under a 'Payload' key when using
    # arn:aws:states:::lambda:invoke -- unwrap it.
    payload = event.get("Payload", event)

    run_id = payload["run_id"]
    stage = payload["stage"]
    config_overrides = payload.get("config_overrides") or {}

    storage = SVAPStorage(DATABASE_URL)

    # -- Gate mode ----------------------------------------------------------
    if payload.get("gate") and "task_token" in payload:
        task_token = payload["task_token"]
        try:
            # Insert a new 'running' log entry so pending_review transition works
            storage.log_stage_start(run_id, stage)
            storage.log_stage_pending_review(run_id, stage)
            storage.store_task_token(run_id, stage, task_token)
            logger.info(
                "Gate registered for run_id=%s stage=%s; waiting for approval",
                run_id,
                stage,
            )
            return {
                "status": "waiting_for_approval",
                "run_id": run_id,
                "stage": stage,
            }
        except Exception as e:
            logger.error(
                "Failed to register gate for run_id=%s stage=%s: %s",
                run_id,
                stage,
                e,
            )
            traceback.print_exc()
            raise

    # -- Stage mode ---------------------------------------------------------
    try:
        config = _get_config(config_overrides)
        result = _run_stage(storage, run_id, stage, config)
        return {"status": "completed", "run_id": run_id, "stage": stage, **result}
    except Exception as e:
        logger.error(
            "Stage %s failed for run_id=%s: %s",
            stage,
            run_id,
            e,
        )
        traceback.print_exc()
        storage.log_stage_failed(run_id, stage, str(e))
        raise


def _get_config(overrides: dict | None = None) -> dict:
    """Load pipeline config from S3 (deployed) or local file (dev)."""
    config = {}

    if CONFIG_BUCKET:
        try:
            s3 = boto3.client("s3")
            obj = s3.get_object(Bucket=CONFIG_BUCKET, Key="config.yaml")
            config = yaml.safe_load(obj["Body"].read())
        except Exception:
            config = _default_config()
    else:
        try:
            config = _load_config("config.yaml")
        except FileNotFoundError:
            config = _default_config()

    if overrides:
        config.update(overrides)

    return config


def _default_config() -> dict:
    """Minimal default config when no config file is available."""
    return {
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
