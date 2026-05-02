.PHONY: ci lint lint-fix format format-check typecheck terraform-fmt-check \
       lint-frontend lint-fix-frontend format-frontend format-check-frontend \
       lint-rust format-rust format-check-rust \
       seed reset reset-corpus reset-runs reset-dry-run reset-trees runs

ci: format-check-rust lint-rust lint-frontend typecheck terraform-fmt-check

lint: lint-rust lint-frontend
lint-fix: lint-fix-frontend
format: format-rust format-frontend
format-check: format-check-rust format-check-frontend

lint-frontend:
	cd frontend && pnpm exec eslint .

lint-fix-frontend:
	cd frontend && pnpm exec eslint . --fix

format-frontend:
	cd frontend && pnpm exec prettier --write .

format-check-frontend:
	cd frontend && pnpm exec prettier --check .

lint-rust:
	cd backend && CARGO_TARGET_DIR=target-clippy cargo clippy --release -- -D warnings -W clippy::cognitive_complexity

format-rust:
	cd backend && cargo fmt

format-check-rust:
	cd backend && cargo fmt -- --check

typecheck:
	cd frontend && pnpm exec tsc --noEmit

terraform-fmt-check:
	terraform fmt -check -recursive infrastructure/terraform/

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

reset-trees:
	cd backend && uv run scripts/reset_runs.py --stages56 $(ARGS)

runs:
	cd backend && uv run scripts/reset_runs.py --list
