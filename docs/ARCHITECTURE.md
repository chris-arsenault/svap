# Architecture

## System Overview

SVAP is a seven-stage analytical pipeline where each stage performs a distinct cognitive task. Stages are connected through a PostgreSQL database that stores all intermediate outputs, enabling resumability, auditability, and independent re-execution of any stage.

In production, stages are orchestrated by AWS Step Functions. Locally, the CLI orchestrator runs stages sequentially.

```
+--------------+    +--------------+    +--------------+
|  Documents   |    |   Bedrock    |    |  PostgreSQL  |
|  (RAG Store) |    |   (Claude)   |    |   (State)    |
+------+-------+    +------+-------+    +------+-------+
       |                   |                    |
       +----------+--------+                    |
                  |                             |
         +--------v-----------------------------v------+
         |              Stage Runner                    |
         |  +-----+ +-----+ +-----+ +-----+           |
         |  | S0  |>| S1  |>| S2  |>| S3  |> ...      |
         |  +-----+ +--+--+ +-----+ +-----+           |
         |              |  Human Gate                   |
         +--------------+-------------------------------+
```

## Components

### API Handler (`api.py`)
- Lambda function for all HTTP requests
- Routes on API Gateway V2 `routeKey` strings via a `ROUTES` dict
- No web framework -- plain Python handler
- Returns 202 Accepted for async pipeline operations

### Stage Runner (`stage_runner.py`)
- Lambda function invoked by Step Functions
- Two modes: **stage mode** (runs a pipeline stage) and **gate mode** (registers a task token for human approval)
- Shares the same codebase as the API handler

### Orchestrator (`orchestrator.py`)
- CLI interface for local development
- Commands: `seed`, `run`, `approve`, `status`
- Stage sequencing and prerequisite checking

### Storage (`storage.py`)
- PostgreSQL schema auto-migration via `CREATE TABLE IF NOT EXISTS`
- CRUD operations for all pipeline entities
- Stage execution logging with status tracking
- Database URL resolution: environment variable, Terraform state, or config fallback

### Bedrock Client (`bedrock_client.py`)
- AWS Bedrock API wrapper for Claude
- Prompt template loading from `.txt` files with `{variable}` substitution
- JSON response parsing with markdown fence stripping
- Retry logic with exponential backoff

### RAG Module (`rag.py`)
- Document ingestion with paragraph-boundary chunking
- Keyword-based retrieval (upgradeable to vector search)
- Context assembly for prompt injection

## Data Flow

```
Stage 0                Stage 1                 Stage 2
documents ----fetch--> cases ------LLM-------> taxonomy (delta + dedup)
                       (enabling_condition)    (qualities)

Stage 3                Stage 4                 Stage 5               Stage 6
cases x taxonomy       policies x taxonomy     high-risk policies    predictions
------LLM-------->     ------LLM-------->      ------LLM-------->   ------LLM-------->
convergence_scores     policy_scores           predictions          detection_patterns
+ calibration
```

### Delta Processing

Stages 1 and 2 use iterative delta processing:

- **Stage 1** tracks which documents have been processed. New documents are extracted; existing ones are skipped.
- **Stage 2** tracks which cases have been processed for taxonomy. New cases are clustered and refined, then semantically deduplicated against the existing taxonomy via an LLM comparison. Matching qualities merge their canonical examples; novel qualities are inserted as drafts pending human review.

### Global vs. Per-Run Tables

Some tables are global (shared across runs) and some are per-run:

- **Global:** `enforcement_sources`, `source_feeds`, `documents`, `chunks`, `cases`, `taxonomy`, `policies`, `taxonomy_case_log`
- **Per-run:** `pipeline_runs`, `stage_log`, `convergence_scores`, `calibration`, `policy_scores`, `predictions`, `detection_patterns`

This separation means the corpus (cases, taxonomy, policies) accumulates across runs while analytical outputs are scoped to a specific pipeline execution.

## Human Gates

Stages 2 and 5 are human gates. In Step Functions, these use `waitForTaskToken`:

1. The stage runner stores the task token in the database
2. The stage status is set to `pending_review`
3. Step Functions pauses until `send_task_success()` is called
4. The user approves via the UI or CLI, which sends the callback
5. The state machine resumes with the next stage

Stage 2's gate is conditional: it only fires when novel draft qualities are extracted. If all new qualities merge with existing taxonomy entries, the stage completes automatically.

## Extension Points

### Retrieval backend
Replace `storage.search_chunks()` with a vector similarity search. Configure `rag.embedding_model` in `config.yaml`, add an embedding step to document ingestion, and swap the search query.

### Custom stage logic
Each stage can be modified independently. The `run(storage, client, run_id, config)` signature is the contract. Common customizations include adding extraction fields (Stage 1), changing calibration methodology (Stage 3), or grounding detection patterns in actual data dictionary schemas (Stage 6).

### Per-stage model selection
Different stages have different cognitive demands. Extraction and scoring tasks (Stages 1, 3, 4) can use smaller models. Taxonomy abstraction (Stage 2) and prediction generation (Stage 5) benefit from larger models. Configure per-stage model overrides in `config.yaml`.
