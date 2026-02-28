<!-- drift-generated -->
# ADR 015: Shared Async Error Handling (useAsyncAction + ErrorBanner)

## Status
Accepted (2026-02-28)

## Context
10 of 12 views had no error UI for async operations. Views independently created
local busy state with different types (boolean vs string|null). Only ManagementView
displayed errors; the rest logged to console.error or swallowed silently.

## Decision
Standardize on `useAsyncAction()` hook (from `hooks.ts`) and `<ErrorBanner />`
component (from `SharedUI.tsx`) for all views with async operations.

### useAsyncAction() API
```tsx
const { busy, error, run, clearError } = useAsyncAction();

// busy: string | null — label of the running action, or null
// error: string | null — error message from the last failed action
// run(label, fn): wraps an async fn with busy/error tracking
// clearError(): dismisses the error
```

### ErrorBanner API
```tsx
<ErrorBanner error={error} onDismiss={clearError} />
```

## Rules
- Every view with user-initiated async operations MUST use `useAsyncAction()`
- Every view with `useAsyncAction()` MUST render `<ErrorBanner />` at the top
- Never `console.error` and swallow — surface errors to the user
- Never use raw `useState(false)` for busy tracking — use `useAsyncAction()`

## Consequences
- Users see feedback when operations fail (uploads, pipeline runs, approvals)
- Consistent busy/error UX across all views
- Error banner uses `.error-banner` CSS class (defined in index.css)

## Enforcement
- **ESLint rule:** `local/no-manual-async-state` (in `frontend/eslint-rules/no-manual-async-state.js`)
  warns when views use `useState<boolean>` or `useState(false)` patterns for manual
  busy/loading tracking instead of `useAsyncAction()`
- **Pattern doc:** [async-error-handling](../patterns/async-error-handling.md)
