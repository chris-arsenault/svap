# CLAUDE.md — Instructions for AI Agents

## Critical Rules

- **NEVER** run `terraform` commands (init, plan, apply, destroy, import, state) unless the user explicitly asks
- **NEVER** source `.env` files or set AWS credentials in the shell unless the user explicitly asks
- **NEVER** run `aws` CLI commands or interact with AWS services unless the user explicitly asks
- **NEVER** run commands that cost money or modify cloud infrastructure
- These operations are expensive and dangerous — only the user initiates them

## Project Overview

SVAP is a 7-stage LLM pipeline (stages 0-6) that analyzes healthcare enforcement actions to predict policy exploitation. Backend is a plain Python Lambda handler (no web framework). Frontend is React + TypeScript + Vite. Infrastructure is Terraform on AWS.

## Code Layout

| Path | What |
|------|------|
| `backend/src/svap/api.py` | Lambda handler — routes on `event["routeKey"]` via `ROUTES` dict |
| `backend/src/svap/stage_runner.py` | Lambda handler for Step Functions stage execution |
| `backend/src/svap/orchestrator.py` | CLI entry point + `_run_stage()` |
| `backend/src/svap/storage.py` | PostgreSQL schema (`SCHEMA_STATEMENTS`) + all CRUD |
| `backend/src/svap/bedrock_client.py` | Bedrock API wrapper with retry, JSON parsing |
| `backend/src/svap/rag.py` | Document ingestion + keyword retrieval |
| `backend/src/svap/stages/` | Stage 0-6 implementations |
| `backend/src/svap/prompts/` | LLM prompt templates (`.txt` with `{variable}` placeholders) |
| `backend/src/svap/seed/` | Bootstrap JSON data |
| `backend/config.yaml` | Pipeline config (model ID, RAG settings, human gates) |
| `frontend/src/data/usePipelineData.tsx` | Single data hook — Context provider for all views |
| `frontend/src/views/` | One file per view (Dashboard, Sources, Cases, etc.) |
| `infrastructure/terraform/svap.tf` | All AWS resources (VPC, RDS, Lambda, API GW, Step Functions) |

## Architecture Patterns

- **No web framework**: `api.py` routes directly on API Gateway V2 `routeKey` strings. Add new routes to the `ROUTES` dict + terraform route list.
- **Schema auto-migration**: `storage.py` `SCHEMA_STATEMENTS` runs `CREATE TABLE IF NOT EXISTS` on every Lambda cold start.
- **Two Lambda functions**: `svap-api` (HTTP API) and `svap-stage-runner` (Step Functions invocation). Both share the same codebase.
- **Default config in two places**: `api.py:_DEFAULT_CONFIG` and `stage_runner.py:_default_config()`. Both must stay in sync.
- **Bedrock model ID**: Currently `us.anthropic.claude-sonnet-4-6` (inference profile format, not raw model ID).
- **Human gates**: Stages 2 and 5 require approval via Step Functions task tokens.
- **Auth**: Cognito JWT from shared user pool in `../websites/` repo. Tokens passed as `Authorization: Bearer {jwt}`.

## Development Commands

```bash
# Backend lint
cd backend && uv run ruff check src/

# Frontend typecheck + build
cd frontend && npx tsc --noEmit && npm run build

# Local dev servers
cd backend && python -m svap.dev_server    # :5000
cd frontend && npm run dev                  # :5173
```

## Common Gotchas

- Adding an API route requires: handler function in `api.py`, entry in `ROUTES` dict, route string in `svap.tf` API Gateway routes list
- The `scale_dollars` column is `REAL` — `_parse_dollars()` in `stage1_case_assembly.py` handles messy LLM output
- `enforcement_sources` table is not tied to a run — it's a global registry with document tracking
- S3 uploads go to `SVAP_CONFIG_BUCKET` under `enforcement-sources/{source_id}/`
- Frontend `usePipelineData` fetches `/api/dashboard` on mount — this is the single data source for all views

## Documentation Index

- [Architecture](docs/ARCHITECTURE.md) — System design, data flow, extension points
- [Data Model](docs/DATA_MODEL.md) — Database schema, entity relationships
- [Prompt Engineering](docs/PROMPT_ENGINEERING.md) — Prompt design patterns, template format
- [Replication Guide](docs/REPLICATION_GUIDE.md) — Reproducing the HHS OIG analysis
- [Cognito Auth Design](docs/plans/2026-02-25-cognito-auth-design.md) — Authentication architecture
- [Cognito Auth Plan](docs/plans/2026-02-25-cognito-auth-plan.md) — Auth implementation plan
