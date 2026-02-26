.PHONY: lint lint-fix format format-check \
       lint-frontend lint-fix-frontend format-frontend format-check-frontend \
       lint-backend lint-fix-backend format-backend format-check-backend

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
