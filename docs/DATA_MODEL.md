# Data Model

All pipeline state is stored in PostgreSQL. The schema is defined in `backend/src/svap/storage.py` and auto-migrates on every Lambda cold start.

## Entity Relationships

```
pipeline_runs
    |
    |-> stage_log (execution tracking)
    |
    |-> convergence_scores (Stage 3)
    |       |
    |       +-- cases (FK)
    |       +-- taxonomy (FK)
    |
    |-> calibration (Stage 3)
    |
    |-> policy_scores (Stage 4)
    |       |
    |       +-- policies (FK)
    |       +-- taxonomy (FK)
    |
    |-> predictions (Stage 5)
    |       |
    |       +-> detection_patterns (Stage 6)
    |
    +-> documents / chunks (RAG store)

Global tables (no run_id):
    cases  <--  taxonomy_case_log
    taxonomy
    policies
    enforcement_sources
    source_feeds / source_candidates
```

## Table Reference

### pipeline_runs
| Column | Type | Description |
|--------|------|-------------|
| run_id | TEXT PK | Unique identifier (e.g., `run_20260227_032611`) |
| created_at | TEXT | ISO 8601 timestamp |
| config_snapshot | TEXT | JSON snapshot of config at run creation |
| notes | TEXT | Optional notes |

### stage_log
| Column | Type | Description |
|--------|------|-------------|
| id | SERIAL PK | Auto-increment |
| run_id | TEXT FK | References pipeline_runs |
| stage | INTEGER | Stage number (0-6, plus sub-stages 40-42) |
| status | TEXT | `running`, `completed`, `failed`, `pending_review`, `approved` |
| started_at | TEXT | ISO timestamp |
| completed_at | TEXT | ISO timestamp |
| error_message | TEXT | Error details if failed |
| metadata | TEXT | JSON with stage-specific metrics |
| task_token | TEXT | Step Functions task token for human gates |

### enforcement_sources (Global)
| Column | Type | Description |
|--------|------|-------------|
| source_id | TEXT PK | Unique identifier |
| name | TEXT | Source name |
| description | TEXT | Brief description |
| url | TEXT | Source URL (nullable) |
| source_type | TEXT | Category of source |
| has_document | BOOLEAN | Whether a document has been uploaded |
| s3_key | TEXT | S3 object key for uploaded document |
| doc_id | TEXT | References documents table |
| summary | TEXT | LLM-generated summary |
| validation_status | TEXT | `pending`, `valid`, `invalid`, `error` |

### cases (Global, Stage 1 output)
| Column | Type | Description |
|--------|------|-------------|
| case_id | TEXT PK | Hash-based ID from case name |
| source_doc_id | TEXT FK | References documents table |
| case_name | TEXT | Short descriptive name |
| scheme_mechanics | TEXT | Step-by-step scheme operation |
| exploited_policy | TEXT | Specific policy exploited |
| enabling_condition | TEXT | Structural property that enabled exploitation |
| scale_dollars | REAL | Dollar amount of losses |
| scale_defendants | INTEGER | Number of defendants |
| scale_duration | TEXT | Duration of scheme |
| detection_method | TEXT | How it was discovered |
| raw_extraction | TEXT | Full JSON from LLM extraction |

### taxonomy (Global, Stage 2 output)
| Column | Type | Description |
|--------|------|-------------|
| quality_id | TEXT PK | 8-char hex from `sha256(name)` |
| name | TEXT | Short memorable label (3-6 words) |
| definition | TEXT | One-sentence domain-agnostic definition |
| recognition_test | TEXT | 3-5 concrete yes/no questions |
| exploitation_logic | TEXT | Causal mechanism: why this quality enables exploitation |
| canonical_examples | TEXT | JSON array of example cases |
| review_status | TEXT | `draft`, `approved`, `rejected`, `revised` |
| reviewer_notes | TEXT | Expert comments |
| created_at | TEXT | ISO timestamp |

### taxonomy_case_log (Global, Stage 2 delta tracking)
| Column | Type | Description |
|--------|------|-------------|
| case_id | TEXT PK FK | References cases -- tracks which cases have been processed for taxonomy |
| processed_at | TEXT | ISO timestamp |

### convergence_scores (Per-run, Stage 3 output)
| Column | Type | Description |
|--------|------|-------------|
| id | SERIAL PK | Auto-increment |
| run_id | TEXT FK | References pipeline_runs |
| case_id | TEXT FK | References cases |
| quality_id | TEXT FK | References taxonomy |
| present | INTEGER | 0 or 1 |
| evidence | TEXT | One-sentence justification |

### calibration (Per-run, Stage 3 output)
| Column | Type | Description |
|--------|------|-------------|
| run_id | TEXT PK | References pipeline_runs |
| threshold | INTEGER | Convergence score above which exploitation is likely large-scale |
| correlation_notes | TEXT | Description of score-severity relationship |
| quality_frequency | TEXT | JSON: how often each quality appears |
| quality_combinations | TEXT | JSON: co-occurrence counts for quality pairs |

### policies (Global)
| Column | Type | Description |
|--------|------|-------------|
| policy_id | TEXT PK | Hash-based or manually assigned ID |
| name | TEXT | Policy/program name |
| description | TEXT | Brief description |
| source_document | TEXT | Source filename if extracted from document |
| structural_characterization | TEXT | Detailed structural analysis |

### policy_scores (Per-run, Stage 4 output)
Same structure as convergence_scores but references policies instead of cases.

### predictions (Per-run, Stage 5 output)
| Column | Type | Description |
|--------|------|-------------|
| prediction_id | TEXT PK | Hash-based ID |
| run_id | TEXT FK | References pipeline_runs |
| policy_id | TEXT FK | References policies |
| convergence_score | INTEGER | Policy's convergence score |
| mechanics | TEXT | Step-by-step exploitation prediction |
| enabling_qualities | TEXT | JSON array of quality IDs |
| actor_profile | TEXT | Who would exploit this |
| lifecycle_stage | TEXT | Exploration / Optimization / Institutionalization |
| detection_difficulty | TEXT | Easy / Medium / Hard |
| review_status | TEXT | `draft`, `approved` |
| reviewer_notes | TEXT | Expert comments |

### detection_patterns (Per-run, Stage 6 output)
| Column | Type | Description |
|--------|------|-------------|
| pattern_id | TEXT PK | Hash-based ID |
| run_id | TEXT FK | References pipeline_runs |
| prediction_id | TEXT FK | References predictions |
| data_source | TEXT | Which system/table to query |
| anomaly_signal | TEXT | Specific queryable condition with thresholds |
| baseline | TEXT | What normal looks like |
| false_positive_risk | TEXT | Legitimate behaviors that could trigger |
| detection_latency | TEXT | How quickly the signal appears |
| priority | TEXT | `critical`, `high`, `medium`, `low` |
| implementation_notes | TEXT | Technical notes for data engineers |

### documents (RAG store)
| Column | Type | Description |
|--------|------|-------------|
| doc_id | TEXT PK | Hash-based ID |
| filename | TEXT | Original filename |
| doc_type | TEXT | `enforcement`, `policy`, `guidance`, `report`, `other` |
| full_text | TEXT | Complete document text |
| metadata | TEXT | Optional JSON metadata |

### chunks (RAG store)
| Column | Type | Description |
|--------|------|-------------|
| chunk_id | TEXT PK | `{doc_id}_c{index}` |
| doc_id | TEXT FK | References documents |
| chunk_index | INTEGER | Position in document |
| text | TEXT | Chunk content |
| token_count | INTEGER | Approximate token count |
