<!-- drift-generated -->
# API Access Pattern Guide

**ADR:** [014-shared-api-utility](../adr/014-shared-api-utility.md)
**ESLint rule:** `local/no-direct-fetch` (warn)

## Quick Rules

1. All HTTP calls MUST use `apiGet()` or `apiPost()` from `data/api.ts`
2. Never call `fetch()` directly with manual auth headers
3. Never import `getToken()` in view files — the API utility handles auth
4. `API_BASE` is exported from `data/api.ts` — no local config references

## Pattern

```tsx
import { apiGet, apiPost } from "../data/api";

// GET request
const res = await apiGet("/management/executions");
if (!res.ok) throw new Error(`${res.status}`);
const data = await res.json();

// POST request (body is JSON-serialized automatically)
await apiPost("/management/runs/delete", { run_id: runId });
```

## Where API Calls Happen

| Caller | How it calls the API |
|--------|---------------------|
| `pipelineStore.ts` | Imports `apiGet`/`apiPost` from `data/api.ts` |
| `ManagementView.tsx` | Imports `apiGet`/`apiPost` from `data/api.ts` |
| Other views | Call store actions (which internally use `data/api.ts`) |

Views that need direct API calls (outside the store) import from `data/api.ts`.
Most views use store action hooks and never touch the API layer directly.

## What NOT To Do

```tsx
// BAD: manual fetch with auth headers
import { getToken } from "../auth";
const token = await getToken();
const res = await fetch(`${config.apiBaseUrl}/cases`, {
  headers: { Authorization: `Bearer ${token}` },
});

// BAD: local API_BASE
const API_BASE = config.apiBaseUrl || "/api";

// GOOD: shared utility
import { apiGet } from "../data/api";
const res = await apiGet("/cases");
```
