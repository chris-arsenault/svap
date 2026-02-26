#!/usr/bin/env bash
# Start both backend and frontend for local development
# Requires: local PostgreSQL running (e.g. docker run -d -p 5432:5432 -e POSTGRES_DB=svap -e POSTGRES_USER=svap -e POSTGRES_PASSWORD=password postgres:16)
set -e

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
export DATABASE_URL="${DATABASE_URL:-postgresql://svap:password@localhost:5432/svap}"

echo "Starting SVAP development environment..."
echo "Database: $DATABASE_URL"
echo ""

# Backend
cd "$ROOT_DIR/backend"
echo "Starting backend on http://localhost:8000"
uv run uvicorn svap.api:app --reload --port 8000 &
BACKEND_PID=$!

# Frontend
cd "$ROOT_DIR/frontend"
if [ ! -d node_modules ]; then
    echo "Installing frontend dependencies..."
    npm install --silent
fi
echo "Starting frontend on http://localhost:5173"
npm run dev &
FRONTEND_PID=$!

echo ""
echo "Backend:  http://localhost:8000/api/health"
echo "Frontend: http://localhost:5173"
echo "Press Ctrl+C to stop both."
echo ""

trap "kill $BACKEND_PID $FRONTEND_PID 2>/dev/null; exit 0" INT TERM
wait
