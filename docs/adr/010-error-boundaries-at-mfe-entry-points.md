<!-- drift-generated -->
# ADR-010: Error Boundaries at MFE Entry Points

## Status

Accepted (2026-02-27)

## Context

The monorepo uses Module Federation to compose multiple micro-frontend (MFE)
apps into a host shell. Each MFE exposes a Host component that is lazy-loaded
with `React.lazy` and wrapped in `Suspense` for loading states.

When a render error occurred inside a remote MFE, React's default behavior
unmounted the entire component tree up to the nearest error boundary. Since
most Host components had no error boundary, a render crash in any remote would
take down the entire host app with an unrecoverable white screen.

A shared `ErrorBoundary` component existed in `@penguin-tales/shared-components`
but was not consistently used across all MFE entry points.

## Decision

### Every MFE Host wraps Suspense with ErrorBoundary

All 7 Host files use the shared `ErrorBoundary` from
`@penguin-tales/shared-components`:

- IlluminatorHost
- CoherenceEngineHost
- NameForgeHost
- LoreWeaveHost
- CosmographerHost
- ArchivistHost
- ChroniclerHost

The wrapping order is `ErrorBoundary > Suspense > lazy component`:

```jsx
<ErrorBoundary>
  <Suspense fallback={<Loading />}>
    <LazyApp />
  </Suspense>
</ErrorBoundary>
```

### ErrorBoundary provides a retry mechanism

When a render error is caught, ErrorBoundary displays an error message with a
retry button that resets the boundary state and re-attempts rendering. This
keeps the host app functional and lets the user recover without a full page
reload.

### ErrorBoundary lives in shared-components

The canonical `ErrorBoundary` is in `@penguin-tales/shared-components`. MFE
apps must not create local error boundary implementations.

## Consequences

- Render errors in any MFE remote are contained to that remote's boundary
  rather than crashing the host app
- Users see a retry button instead of a white screen on render failures
- New MFE remotes must include `ErrorBoundary` wrapping in their Host
  component
- The shared `ErrorBoundary` is the single implementation; enhancements
  (error reporting, custom fallback UI) are made there and apply to all MFEs
