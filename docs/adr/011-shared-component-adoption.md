<!-- drift-generated -->
# ADR-011: Shared Component Adoption

## Status

Accepted (2026-02-27)

## Context

The monorepo's `@penguin-tales/shared-components` package provides reusable UI
components (ModalShell, ErrorBoundary, input components, hooks, etc.) intended
to be shared across all MFE apps. However, adoption was inconsistent: some apps
imported from the shared package while others had local duplicates of the same
components. The shared package was also not consistently configured as a Module
Federation singleton, meaning multiple instances could coexist at runtime,
breaking shared state and increasing bundle size.

## Decision

### shared-components is a federation singleton

`@penguin-tales/shared-components` is listed in `sharedDepsExtended` in the
Module Federation configuration (`config/federation.js`). This ensures a single
instance is shared across all MFE remotes at runtime, which is required for
components that use React context or shared stores.

### All apps declare it as a workspace dependency

Every app in the monorepo has `@penguin-tales/shared-components` as a workspace
dependency in its `package.json`. This provides consistent type resolution and
ensures the package is available in all development and build contexts.

### New shared UI patterns go in shared-components

When a UI pattern is needed by more than one app, it must be added to
`@penguin-tales/shared-components` rather than duplicated locally. The shared
package is the canonical location for cross-app UI components, hooks, and
utilities.

### Local duplicates are migrated incrementally

Existing local copies of components that have canonical versions in
shared-components should be replaced with imports from the shared package.
This migration happens incrementally as code is touched for other work.

## Consequences

- New apps must add `@penguin-tales/shared-components` as a workspace
  dependency and include it in their federation shared config
- Developers should check shared-components before creating a new local
  component; if a similar component exists, extend it rather than duplicate it
- Local component copies that duplicate shared-components functionality should
  be replaced when the surrounding code is modified
- The shared package is the single source of truth for cross-app UI;
  enhancements and bug fixes are made there and propagate to all consumers
