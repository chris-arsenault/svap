<!-- drift-generated -->
# Code Review Checklist — Drift Prevention

Use this checklist when reviewing PRs. Each item corresponds to a unified drift area.

## Frontend

### Store Access (ADR 013)
- [ ] Views import from `usePipelineSelectors`, not `pipelineStore` directly
- [ ] Actions use individual hooks (`useRunPipeline()`, etc.)
- [ ] Data uses individual hooks (`useCases()`, etc.)
- [ ] No new `usePipelineStore()` calls in view files

### API Access (ADR 014)
- [ ] HTTP calls use `apiGet()`/`apiPost()` from `data/api.ts`
- [ ] No raw `fetch()` calls with manual auth headers
- [ ] No `getToken()` imports in view files
- [ ] No local `API_BASE` definitions

### Async Error Handling (ADR 015)
- [ ] Views with async operations use `useAsyncAction()` from `hooks.ts`
- [ ] `<ErrorBanner />` rendered at top of views with async operations
- [ ] No `console.error` + swallow pattern
- [ ] No raw `useState(false)` for busy tracking

### CSS (ADR 009)
- [ ] No inline `style={{}}` attributes (except CSS custom properties for dynamic values)
- [ ] New styles added to `index.css` utility classes, not inline
- [ ] `eslint-disable` comment present for any legitimate `style={}` use

## Backend

### Config (ADR 016)
- [ ] Default config values come from `svap.defaults.default_config()`
- [ ] No config values defined in `api.py` or `stage_runner.py`

### Parallel Execution (ADR 016)
- [ ] Parallel LLM calls use `run_parallel_llm()` from `svap.parallel`
- [ ] No new `ThreadPoolExecutor` usage in stage files

### Route Parameters (ADR 016)
- [ ] Path parameters accessed via `_path_param(event, name)`
- [ ] No raw `event["pathParameters"]` indexing
