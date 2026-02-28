<!-- drift-generated -->
# ADR 013: Unified Store Access via usePipelineSelectors

## Status
Accepted (2026-02-28)

## Context
Views used 4 different patterns to access the Zustand store: useShallow for data,
direct inline selectors, pre-defined selector hooks (usePipelineSelectors.ts), and
mixed data+actions in useShallow. Only 1 of 12 views used the selector hooks despite
14 being defined.

## Decision
All views access the store exclusively through `usePipelineSelectors.ts`.

- **Actions**: individual hooks (`useRunPipeline()`, `useFetchDiscovery()`, etc.)
- **Data**: individual hooks (`useCases()`, `usePolicies()`, `useThreshold()`, etc.)
- **No view imports `usePipelineStore` directly** (except Dashboard for a
  multi-field useShallow, which is acceptable for render-heavy components)

## Consequences
- New views follow a single clear pattern: import from `usePipelineSelectors`
- Actions are stable references that never trigger re-renders
- Data selectors subscribe to exactly one slice, minimizing unnecessary re-renders
- `usePipelineStore` is an implementation detail of the data layer, not a view-level API
