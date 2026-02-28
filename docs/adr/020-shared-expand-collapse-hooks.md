<!-- drift-generated -->
# ADR 020: Shared Expand/Collapse Hooks

## Status

Accepted

## Context

The expand/collapse UI pattern appeared in 10+ views with three independent
implementations:

1. **Set-based multi-expand** (DetectionView) — `Set<string>` state with manual
   `toggleSet()` helper, duplicated onClick/onKeyDown handlers.
2. **Single-expand with inline state** (PredictionView, TaxonomyView, CaseSourcing,
   DimensionRegistryView, DiscoveryView) — `useState<string | null>(null)` with
   inline `(prev) => prev === id ? null : id` toggle callbacks.
3. **Local boolean state** (PolicyExplorer TreeNode) — per-node `useState(bool)`
   with inline toggle.

Every expandable element duplicated the keyboard accessibility pattern:
`role="button"`, `tabIndex={0}`, `onClick`, `onKeyDown` with Enter/Space handling.

Cluster-004 (similarity 0.43) identified DetectionView, PolicyExplorer, and
PredictionView as structurally similar. Cluster-002 identified the data display
views with similar patterns.

## Decision

Extract three shared primitives to `hooks.ts`:

- **`useExpandSet()`** — returns `{ expanded, toggle, set, reset }` for
  multi-expand scenarios (Set<string>-based).
- **`useExpandSingle()`** — returns `{ expandedId, toggle }` for single-expand
  scenarios (one item at a time).
- **`expandableProps(onToggle)`** — returns `{ role, tabIndex, onClick, onKeyDown }`
  for keyboard-accessible expandable elements.

Also exported: **`toggleSet(prev, id)`** utility for direct Set manipulation.

## Consequences

- Expand/collapse logic is defined once and tested once
- Keyboard accessibility is guaranteed by using `expandableProps()`
- New views pick up the pattern by importing from hooks.ts

## Enforcement

- **`local/no-manual-expand-state`** ESLint rule warns on `useState` with
  `expanded*` variable names in view/component files
- **`no-manual-expand-state.js`** in `frontend/eslint-rules/`
- Pattern guide: `docs/patterns/expand-collapse.md`
