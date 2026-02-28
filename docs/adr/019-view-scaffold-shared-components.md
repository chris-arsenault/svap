<!-- drift-generated -->
# ADR 019: View Scaffold Shared Components

## Status

Accepted

## Context

All 12 views in the frontend repeated identical markup for their header section:
a `<div className="view-header stagger-in">` containing an `<h2>` title and a
`<div className="view-desc">` description. Five views (Dashboard, DiscoveryView,
ManagementView, ResearchView, SourcesView) additionally duplicated a metrics row
pattern with identical `metric-card` markup — each card being 4-5 lines of nested
divs with `metric-label`, `metric-value`, and `metric-sub` classes.

This was identified as cluster-001 in the semantic drift analysis (similarity 0.42,
5 members) with dominant signals: neighborhood 1.0, typeSignature 1.0, behavior 0.84.

## Decision

Extract two shared components to `components/SharedUI.tsx`:

- **`ViewHeader`** — accepts `title: string` and `description: React.ReactNode`.
  Renders the standard view-header with stagger-in animation.
- **`MetricCard`** — accepts `label`, `value`, optional `sub` and `className`.
  Renders a single metric card with stagger-in animation.

All views import and use these components instead of raw markup.

## Consequences

- View files are shorter and more focused on their unique content
- Header styling changes propagate automatically to all views
- New views get the standard scaffold by importing two components

## Enforcement

- **`local/no-manual-view-header`** ESLint rule warns on raw `view-header`
  className usage in view files
- **`no-manual-view-header.js`** in `frontend/eslint-rules/`
- Pattern guide: `docs/patterns/view-scaffold.md`
