.PHONY: lint lint-fix format format-check \
       lint-frontend lint-fix-frontend format-frontend format-check-frontend \
       lint-backend lint-fix-backend format-backend format-check-backend \
       seed reset reset-corpus reset-runs reset-dry-run runs

lint: lint-frontend lint-backend
lint-fix: lint-fix-frontend lint-fix-backend
format: format-frontend format-backend
format-check: format-check-frontend format-check-backend

lint-frontend:
	cd frontend && npx eslint .

lint-fix-frontend:
	cd frontend && npx eslint . --fix

format-frontend:
	cd frontend && npx prettier --write .

format-check-frontend:
	cd frontend && npx prettier --check .

lint-backend:
	cd backend && uv run --extra dev ruff check src/

lint-fix-backend:
	cd backend && uv run --extra dev ruff check --fix src/

format-backend:
	cd backend && uv run --extra dev ruff format src/

format-check-backend:
	cd backend && uv run --extra dev ruff format --check src/

# ── Pipeline data ────────────────────────────────────────────

seed:
	cd backend && uv run scripts/reset_runs.py --corpus
	cd backend && uv run -m svap.orchestrator seed

reset: reset-corpus

reset-corpus:
	cd backend && uv run scripts/reset_runs.py --corpus

reset-runs:
	cd backend && uv run scripts/reset_runs.py --all

reset-dry-run:
	cd backend && uv run scripts/reset_runs.py --corpus --dry-run

runs:
	cd backend && uv run scripts/reset_runs.py --list
