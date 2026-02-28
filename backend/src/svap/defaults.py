"""Shared default configuration for all Lambda entry points."""


def default_config() -> dict:
    """Return a fresh default config dict.

    Both ``api.py`` and ``stage_runner.py`` fall back to this when
    ``config.yaml`` cannot be loaded from S3 or the local filesystem.
    """
    return {
        "bedrock": {
            "region": "us-east-1",
            "model_id": "us.anthropic.claude-sonnet-4-6",
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
