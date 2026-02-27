# SVAP

> Structural Vulnerability Analysis Pipeline

LLM-driven pipeline that analyzes enforcement actions to identify structural policy vulnerabilities, predict exploitation patterns, and design detection strategies.

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](./LICENSE)
[![Python](https://img.shields.io/badge/Python-3.12+-green)](https://www.python.org/)
[![TypeScript](https://img.shields.io/badge/TypeScript-5.7-blue)](https://www.typescriptlang.org/)

## Table of Contents

- [Overview](#overview)
- [Pipeline](#pipeline)
- [Architecture](#architecture)
- [Quick Start](#quick-start)
- [Project Structure](#project-structure)
- [Documentation](#documentation)
- [License](#license)

## Overview

SVAP takes a corpus of enforcement documents and runs a seven-stage analytical pipeline. Each stage performs a distinct cognitive task using Claude on AWS Bedrock, with all intermediate outputs stored in PostgreSQL for auditability and resumability.

The pipeline identifies abstract structural properties that make policies exploitable, scores known cases against those properties, scans unevaluated policies for the same patterns, and generates actionable detection strategies.

Two stages (taxonomy extraction and prediction generation) include human gates that pause the pipeline for expert review before proceeding.

## Pipeline

| Stage | Name | Description |
|-------|------|-------------|
| 0 | Source Fetch | Fetches enforcement documents, ingests into RAG store |
| 1 | Case Assembly | Extracts structured case data: scheme mechanics, exploited policy, enabling conditions |
| 2 | Taxonomy Extraction | Clusters enabling conditions into reusable vulnerability qualities. Delta processing with semantic deduplication. **Human gate** |
| 3 | Convergence Scoring | Scores each case against each quality, calibrates the convergence threshold |
| 4 | Policy Scanning | Characterizes policies structurally, scores against the vulnerability taxonomy |
| 5 | Prediction | Generates exploitation predictions for high-scoring policies. **Human gate** |
| 6 | Detection Patterns | Designs anomaly signals, baselines, and false-positive assessments |

Stages 1 and 2 use delta processing: they track which documents and cases have already been processed and only run LLM extraction on new inputs. Stage 2 additionally performs semantic deduplication, merging newly extracted qualities with existing taxonomy entries when they describe the same structural property.

## Architecture

```
Frontend (React + Vite)  ->  API Gateway V2  ->  Lambda (Python)  ->  PostgreSQL (RDS)
         |                        |                    |
    CloudFront + S3         Cognito JWT          Bedrock (Claude)
                                                       |
                                              Step Functions (orchestration)
```

**Backend:** Plain Python Lambda handler routing on API Gateway V2 `routeKey` strings. No web framework. Two Lambda functions share the same codebase: `svap-api` (HTTP) and `svap-stage-runner` (Step Functions).

**Frontend:** React 18 + TypeScript + Vite SPA with Zustand state management and path-based routing. Live pipeline status via polling subscription.

**Infrastructure:** Terraform on AWS. VPC, RDS, Lambda, API Gateway, Step Functions, S3, CloudFront, Cognito.

For detailed system design, see [Architecture](docs/ARCHITECTURE.md).

## Quick Start

### Local Development

```bash
# Backend
cd backend
uv sync
python -m svap.dev_server          # localhost:5000

# Frontend
cd frontend
npm install
npm run dev                        # localhost:5173
```

### Pipeline Operations

```bash
make seed                          # Reset corpus + load seed data
make reset                         # Full corpus reset
make runs                          # List pipeline runs

# Or run stages directly
cd backend
uv run -m svap.orchestrator seed
uv run -m svap.orchestrator run --stage all
uv run -m svap.orchestrator status
```

### Deployment

```bash
cd infrastructure/terraform
terraform init
terraform apply
```

Requires an AWS account with Bedrock model access (Claude Sonnet) and a Cognito user pool.

## Project Structure

```
backend/
  src/svap/
    api.py                Lambda handler (API Gateway V2, no framework)
    stage_runner.py       Lambda handler (Step Functions invocation)
    orchestrator.py       CLI entry point + stage sequencing
    storage.py            PostgreSQL schema + all CRUD
    bedrock_client.py     Bedrock API wrapper with retry + JSON parsing
    rag.py                Document ingestion + keyword retrieval
    hhs_data.py           Policy catalog + static reference data
    stages/               Stage 0-6 implementations
    prompts/              LLM prompt templates (.txt with {variable} placeholders)
    seed/                 Bootstrap data for taxonomy + policies
  config.yaml             Pipeline configuration

frontend/
  src/
    App.tsx               Auth flow + route definitions
    auth.ts               Cognito SDK integration
    data/
      pipelineStore.ts    Zustand store for all pipeline state
      useStatusSubscription.ts  Live polling for pipeline progress
    views/                Dashboard, Sources, Cases, Taxonomy, etc.
    components/           Sidebar, SharedUI

infrastructure/terraform/
  svap.tf                 VPC, RDS, Lambda, API Gateway, Step Functions, S3
  modules/api-http/       API Gateway + Lambda module
  modules/spa-website/    CloudFront + S3 module
```

## Documentation

| Document | Description |
|----------|-------------|
| [Architecture](docs/ARCHITECTURE.md) | System design, component responsibilities, data flow, extension points |
| [Data Model](docs/DATA_MODEL.md) | Database schema, entity relationships, table reference |
| [Prompt Engineering](docs/PROMPT_ENGINEERING.md) | LLM prompt design patterns, stage-specific guidance, testing |
| [Replication Guide](docs/REPLICATION_GUIDE.md) | Step-by-step guide to reproducing the HHS OIG analysis |

## License

[MIT](./LICENSE) -- Copyright (c) 2026 @tsonu
