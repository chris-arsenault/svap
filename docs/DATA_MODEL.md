# Data Model

All pipeline state is stored in SQLite. The schema is in `svap/storage.py`.

## Entity Relationship Diagram

```
pipeline_runs
    │
    ├──▶ stage_log (execution tracking)
    │
    ├──▶ cases (Stage 1)
    │       │
    │       └──▶ convergence_scores (Stage 3)
    │
    ├──▶ taxonomy (Stage 2)
    │       │
    │       ├──▶ convergence_scores (Stage 3, FK to quality_id)
    │       └──▶ policy_scores (Stage 4, FK to quality_id)
    │
    ├──▶ calibration (Stage 3)
    │
    ├──▶ policies (Stage 4)
    │       │
    │       ├──▶ policy_scores (Stage 4)
    │       └──▶ predictions (Stage 5)
    │               │
    │               └──▶ detection_patterns (Stage 6)
    │
    └──▶ documents / chunks (RAG store)
```

## Table Reference

### pipeline_runs
| Column | Type | Description |
|--------|------|-------------|
| run_id | TEXT PK | Unique run identifier (e.g., `run_20250225_143022`) |
| created_at | TEXT | ISO 8601 timestamp |
| config_snapshot | TEXT | JSON snapshot of config at run creation |
| notes | TEXT | Optional notes |

### stage_log
| Column | Type | Description |
|--------|------|-------------|
| id | INTEGER PK | Auto-increment |
| run_id | TEXT FK | References pipeline_runs |
| stage | INTEGER | Stage number (1-6) |
| status | TEXT | `running`, `completed`, `failed`, `pending_review`, `approved` |
| started_at | TEXT | ISO timestamp |
| completed_at | TEXT | ISO timestamp |
| error_message | TEXT | Error details if failed |
| metadata | TEXT | JSON with stage-specific metrics |

### cases (Stage 1 output)
| Column | Type | Description |
|--------|------|-------------|
| case_id | TEXT PK | Hash-based unique ID |
| run_id | TEXT FK | References pipeline_runs |
| source_document | TEXT | Filename of source enforcement document |
| case_name | TEXT | Short descriptive name |
| scheme_mechanics | TEXT | How the scheme operated step-by-step |
| exploited_policy | TEXT | Specific policy/program exploited |
| enabling_condition | TEXT | **Key field**: structural property that enabled exploitation |
| scale_dollars | REAL | Dollar amount of losses |
| scale_defendants | INTEGER | Number of defendants |
| scale_duration | TEXT | How long scheme operated |
| detection_method | TEXT | How it was discovered |
| raw_extraction | TEXT | Full JSON from LLM extraction |

### taxonomy (Stage 2 output)
| Column | Type | Description |
|--------|------|-------------|
| quality_id | TEXT PK | `V1`, `V2`, ..., `VN` |
| run_id | TEXT FK | References pipeline_runs |
| name | TEXT | Short memorable label |
| definition | TEXT | One-sentence domain-agnostic definition |
| recognition_test | TEXT | Yes/no questions to identify quality in any policy |
| exploitation_logic | TEXT | Causal mechanism: why this quality enables exploitation |
| canonical_examples | TEXT | JSON array of example cases |
| review_status | TEXT | `draft`, `approved`, `rejected`, `revised` |
| reviewer_notes | TEXT | SME comments |

### convergence_scores (Stage 3 output)
| Column | Type | Description |
|--------|------|-------------|
| id | INTEGER PK | Auto-increment |
| run_id | TEXT FK | |
| case_id | TEXT FK | References cases |
| quality_id | TEXT FK | References taxonomy |
| present | INTEGER | 0 or 1 |
| evidence | TEXT | One-sentence justification |

### calibration (Stage 3 output)
| Column | Type | Description |
|--------|------|-------------|
| run_id | TEXT PK | References pipeline_runs |
| threshold | INTEGER | Convergence score above which exploitation is likely large-scale |
| correlation_notes | TEXT | Description of score-severity relationship |
| quality_frequency | TEXT | JSON: how often each quality appears |
| quality_combinations | TEXT | JSON: co-occurrence counts for quality pairs |

### policies (Stage 4 input/output)
| Column | Type | Description |
|--------|------|-------------|
| policy_id | TEXT PK | Hash-based or manually assigned ID |
| run_id | TEXT FK | |
| name | TEXT | Policy/program name |
| description | TEXT | Brief description |
| source_document | TEXT | Source filename if extracted from document |
| structural_characterization | TEXT | Detailed structural analysis (Stage 4a output) |

### policy_scores (Stage 4 output)
Same structure as convergence_scores but references policies instead of cases.

### predictions (Stage 5 output)
| Column | Type | Description |
|--------|------|-------------|
| prediction_id | TEXT PK | Hash-based ID |
| run_id | TEXT FK | |
| policy_id | TEXT FK | References policies |
| convergence_score | INTEGER | Policy's convergence score |
| mechanics | TEXT | Step-by-step exploitation description |
| enabling_qualities | TEXT | JSON array of quality IDs enabling this prediction |
| actor_profile | TEXT | Who would exploit this |
| lifecycle_stage | TEXT | Exploration / Optimization / Institutionalization |
| detection_difficulty | TEXT | Easy / Medium / Hard |
| review_status | TEXT | `draft`, `approved` |
| reviewer_notes | TEXT | SME comments |

### detection_patterns (Stage 6 output)
| Column | Type | Description |
|--------|------|-------------|
| pattern_id | TEXT PK | Hash-based ID |
| run_id | TEXT FK | |
| prediction_id | TEXT FK | References predictions |
| data_source | TEXT | Which system/table to query |
| anomaly_signal | TEXT | Specific queryable condition with thresholds |
| baseline | TEXT | What normal looks like |
| false_positive_risk | TEXT | Legitimate behaviors that could trigger |
| detection_latency | TEXT | How quickly signal appears |
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
