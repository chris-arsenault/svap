<!-- drift-generated -->
# ADR 014: Shared API Utility (data/api.ts)

## Status
Accepted (2026-02-28)

## Context
ManagementView built its own `authHeaders()` helper and called `fetch()` directly,
while all other views used the Zustand store's internal `apiGet`/`apiPost` functions.
This created two auth code paths that could diverge.

## Decision
Extract `apiGet()` and `apiPost()` from `pipelineStore.ts` into a standalone
`data/api.ts` module. Both the store and any view making direct API calls import
from this shared module.

## Rules
- All HTTP calls to the backend MUST use `apiGet()` or `apiPost()` from `data/api.ts`
- Never call `fetch()` directly with manual auth headers
- Never import `getToken()` in view files — the API utility handles auth
- `API_BASE` is exported from `data/api.ts` — no local `config.apiBaseUrl` references

## Consequences
- Single auth code path for token refresh, header construction, and error handling
- New API endpoints only need to be added in one place
- Views that need non-store API calls (e.g., ManagementView) get auth for free
