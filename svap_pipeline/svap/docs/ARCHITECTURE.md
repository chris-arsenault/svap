# Architecture

## System Overview

SVAP is a six-stage analytical pipeline where each stage performs a distinct cognitive task. Stages are connected through a SQLite database that stores all intermediate outputs, enabling resumability, auditability, and independent re-execution of any stage.

```
┌─────────────┐    ┌─────────────┐    ┌─────────────┐
│  Documents   │    │   Bedrock    │    │   SQLite    │
│  (RAG Store) │    │   (Claude)   │    │  (State)    │
└──────┬───────┘    └──────┬───────┘    └──────┬──────┘
       │                   │                    │
       └───────────┬───────┘                    │
                   │                            │
          ┌────────▼────────────────────────────▼────┐
          │              Orchestrator                  │
          │  ┌─────┐ ┌─────┐ ┌─────┐ ┌─────┐        │
          │  │ S1  │→│ S2  │→│ S3  │→│ S4  │→ ...   │
          │  └─────┘ └──┬──┘ └─────┘ └─────┘        │
          │             │ Human Gate                   │
          └─────────────┴──────────────────────────────┘
```

## Component Responsibilities

### Orchestrator (`orchestrator.py`)
- CLI interface and command routing
- Stage sequencing and prerequisite checking
- Human gate enforcement
- Data seeding and export

### Storage (`storage.py`)
- SQLite schema management
- CRUD operations for all pipeline entities
- Stage execution logging
- Keyword-based document retrieval

### Bedrock Client (`bedrock_client.py`)
- AWS Bedrock API wrapper
- Prompt template loading and variable substitution
- JSON response parsing with fence-stripping
- Retry logic with exponential backoff

### RAG Module (`rag.py`)
- Document ingestion and paragraph-boundary chunking
- Keyword-based retrieval (upgradeable to vector search)
- Context assembly for prompt injection
- Structured formatting of pipeline entities for context

### Stages (`stages/`)
Each stage module exports a `run(storage, client, run_id, config)` function and optionally a `load_seed_*()` function for seeding.

## Data Flow

```
Stage 1                    Stage 2                    Stage 3
enforcement docs ──LLM──▶ cases table ──LLM──▶ taxonomy table ──LLM──▶ convergence_scores
                           (enabling_condition)  (qualities V1-VN)     + calibration

Stage 4                    Stage 5                    Stage 6
policies ──LLM──▶ policy_scores ──LLM──▶ predictions ──LLM──▶ detection_patterns
+ taxonomy                 (ranked list)    (mechanics)         (anomaly signals)
```

## Extension Points

### Replacing the retrieval backend
The `storage.search_chunks()` method uses keyword scoring. To use vector search:
1. Configure `rag.embedding_model` in `config.yaml`
2. Add an embedding step to `DocumentIngester.ingest_file()`
3. Replace `search_chunks()` with a cosine-similarity query

### Adding structured data sources
For claims databases or enrollment systems:
1. Write a data extraction function that queries the source and produces text summaries
2. Ingest the summaries via `DocumentIngester.ingest_text()`
3. The RAG module will retrieve relevant summaries into prompt context

### Custom stage logic
Each stage can be modified independently. Common customizations:
- Stage 1: Add domain-specific extraction fields
- Stage 3: Change the calibration methodology (e.g., use a statistical threshold instead of LLM judgment)
- Stage 6: Ground detection patterns in your actual data dictionary schemas

### Multi-model strategies
Different stages have different cognitive demands:
- Stages 1, 3, 4 (extraction/scoring): Claude Haiku may suffice — these are structured extraction tasks
- Stage 2 (taxonomy): Claude Sonnet or Opus recommended — requires abstract reasoning
- Stages 5, 6 (prediction/detection): Claude Sonnet or Opus recommended — requires creative structured reasoning

Configure per-stage model overrides if needed.
