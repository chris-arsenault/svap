# SVAP — Structural Vulnerability Analysis Pipeline

LLM-driven pipeline that analyzes healthcare enforcement actions to identify structural policy vulnerabilities, predict exploitation patterns, and design detection strategies.

## Architecture

```
Frontend (React + Vite)  →  API Gateway V2  →  Lambda (Python)  →  PostgreSQL (RDS)
         ↓                        ↓                    ↓
    CloudFront + S3         Cognito JWT          Bedrock (Claude)
                                                       ↓
                                              Step Functions (orchestration)
```

## Pipeline Stages

| Stage | Name | What it does |
|-------|------|-------------|
| 0 | Source Fetch | Fetches enforcement documents from URLs, ingests into RAG store, validates via LLM |
| 1 | Case Assembly | Extracts structured case data from enforcement documents (scheme mechanics, exploited policy, enabling conditions) |
| 2 | Taxonomy | Clusters enabling conditions into reusable vulnerability qualities (V1-VN). **Human gate** |
| 3 | Convergence Scoring | Scores each case against each quality, calibrates threshold |
| 4 | Policy Scanning | Characterizes HHS policies structurally, scores against vulnerability qualities |
| 5 | Prediction | Generates exploitation predictions for high-scoring policies. **Human gate** |
| 6 | Detection Patterns | Designs anomaly signals, baselines, and false-positive assessments |

## Project Structure

```
backend/                    Python Lambda + pipeline stages
  src/svap/
    api.py                  Lambda handler (API Gateway V2 HTTP API, no framework)
    orchestrator.py         CLI entry point + stage sequencing
    bedrock_client.py       AWS Bedrock wrapper with retry + JSON parsing
    storage.py              PostgreSQL schema + CRUD
    rag.py                  Document ingestion + keyword retrieval
    hhs_data.py             HHS policy catalog + static data
    stages/                 Stage 0-6 implementations
    prompts/                LLM prompt templates (.txt)
    seed/                   Initial data for bootstrapping
  config.yaml              Pipeline configuration

frontend/                   React 18 + TypeScript + Vite SPA
  src/
    App.tsx                 Auth flow + view routing
    auth.ts                 Cognito SDK integration
    config.ts               Runtime config injection
    data/usePipelineData.tsx  Single data hook (Context provider)
    views/                  Dashboard, Sources, Cases, Taxonomy, Matrix, Policies, Predictions, Detection
    components/             Sidebar, SharedUI

infrastructure/terraform/   AWS deployment
  svap.tf                   VPC, RDS, Lambda, API Gateway, Step Functions, S3
  modules/api-http/         API Gateway + Lambda module
  modules/spa-website/      CloudFront + S3 module
```

## Documentation

- [Architecture](docs/ARCHITECTURE.md) — System design, component responsibilities, data flow
- [Data Model](docs/DATA_MODEL.md) — Database schema, entity relationships
- [Prompt Engineering](docs/PROMPT_ENGINEERING.md) — LLM prompt design patterns
- [Replication Guide](docs/REPLICATION_GUIDE.md) — Reproducing the HHS OIG analysis

## Quick Start

### Local Development

```bash
# Backend
cd backend
uv sync
cp config.yaml config.local.yaml  # edit DATABASE_URL
python -m svap.dev_server          # localhost:5000

# Frontend
cd frontend
npm install
npm run dev                        # localhost:5173

# Run pipeline locally
python -m svap.orchestrator seed   # load seed data
python -m svap.orchestrator run --stage all
python -m svap.orchestrator status
```

### Deployment

```bash
cd infrastructure/terraform
terraform init
terraform apply
```

Requires: AWS account with Bedrock model access (Claude Sonnet 4.6), Cognito user pool (shared from `websites` repo).

## Key Environment Variables

| Variable | Where | Purpose |
|----------|-------|---------|
| `DATABASE_URL` | Lambda | PostgreSQL connection string |
| `SVAP_CONFIG_BUCKET` | Lambda | S3 bucket for config.yaml |
| `PIPELINE_STATE_MACHINE_ARN` | Lambda | Step Functions ARN |
| `VITE_API_BASE_URL` | Frontend build | API endpoint |
| `VITE_COGNITO_USER_POOL_ID` | Frontend build | Cognito pool |
| `VITE_COGNITO_CLIENT_ID` | Frontend build | Cognito app client |
