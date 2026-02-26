#!/usr/bin/env bash
# Seed local PostgreSQL database with example data
# Requires: local PostgreSQL running (e.g. docker run -d -p 5432:5432 -e POSTGRES_DB=svap -e POSTGRES_USER=svap -e POSTGRES_PASSWORD=password postgres:16)
set -e

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
export DATABASE_URL="${DATABASE_URL:-postgresql://svap:password@localhost:5432/svap}"

cd "$ROOT_DIR/backend"
uv run python -m svap.orchestrator seed
echo ""
echo "Database seeded at: $DATABASE_URL"
