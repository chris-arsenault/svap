<!-- drift-generated -->
# ADR-001: Custom ESLint Rule for Maximum JSX Props

**Status:** Accepted
**Date:** 2026-02-25
**Deciders:** tsonu
**Drift area:** #5 (ESLint Configuration Fragmentation)

## Context

After decomposing IlluminatorRemote.jsx (2505 lines) into 17 files, the resulting components pass large "props bags" of 20-100+ properties through `{...sharedProps}` spreading. No standard ESLint rule enforces a maximum total prop count on JSX elements — `react/jsx-max-props-per-line` only controls formatting.

High prop counts indicate one or more of:
- A component doing too much
- Store-available data being drilled unnecessarily
- Workflow state that should be grouped into domain objects

## Decision

Add a custom `local/max-jsx-props` ESLint rule at the root level. The rule visits `JSXOpeningElement` nodes and counts all attributes (both `JSXAttribute` and `JSXSpreadAttribute`). Spreads count as 1 each to discourage blind `{...props}` spreading.

**Configuration:**
- Threshold: **12 props** (warn level)
- Scope: all frontend files (`apps/**/webui/src/**/*.{js,jsx,ts,tsx}`)
- Level: `warn` (shows in editor, doesn't block CI)

**Rule location:** `eslint-rules/max-jsx-props.js` (ESM, alongside existing `no-escape-hatches.js`)

## Consequences

- Components with >12 props emit a warning, prompting developers to consider grouping or store access
- The threshold is intentionally lenient — 12 allows well-structured components with clear interfaces
- Leaf components like ChroniclePanel (25 props) are flagged, providing a roadmap for future refactoring
- The rule applies monorepo-wide, not just illuminator

## Alternatives Considered

- **eslint-plugin-local-rules** — adds a dependency for something achievable with a 45-line file
- **TypeScript-level enforcement** — no mechanism exists to cap prop count at the type level
- **Code review only** — too easy to miss during rapid iteration
