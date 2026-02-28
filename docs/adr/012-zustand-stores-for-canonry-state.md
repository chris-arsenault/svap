<!-- drift-generated -->
# ADR-012: Zustand Stores for Canonry State Management

## Status

Accepted (2026-02-27)

## Context

The Canonry app (visual domain configuration editor) managed its state via
`useState` hooks in `App.jsx`, passing state and setters down through props.
As the app grew, App.jsx accumulated a large number of state variables spanning
unrelated concerns: UI navigation (active tab, active section, help modal),
AWS integration (config, tokens, credentials, sync progress), and editor state
(browse state, upload plan, snapshot status).

This made App.jsx difficult to maintain, created deep prop drilling, and
coupled unrelated state changes -- a tab navigation update would re-render
components that only cared about AWS sync status.

## Decision

### Canonry state is managed via zustand stores

State is split into domain-specific zustand stores rather than held in
App.jsx useState hooks:

- **`useCanonryUiStore`**: UI navigation state -- `activeTab`,
  `activeSectionByTab`, `showHome`, `helpModalOpen`,
  `chroniclerRequestedPage`
- **`useCanonryAwsStore`**: AWS integration state -- `config`, `tokens`,
  `status`, `browseState`, `credentials`, `syncProgress`, `uploadPlan`,
  `snapshotStatus`

### Stores handle their own persistence

Each store manages its own persistence strategy:

- `useCanonryUiStore` persists to localStorage via zustand's `subscribe`
  middleware where appropriate
- `useCanonryAwsStore` persists credentials and config via `awsConfigStorage`

Components do not need to know about persistence -- they read and write
through the store API and persistence happens transparently.

### App.jsx consumes stores via selectors

App.jsx imports store hooks with selectors to subscribe to only the state
slices it needs. This eliminates prop drilling and ensures components
re-render only when their subscribed state changes.

```jsx
const activeTab = useCanonryUiStore((s) => s.activeTab);
const syncProgress = useCanonryAwsStore((s) => s.syncProgress);
```

### New state domains get their own store

When a new state concern is added to Canonry that does not fit into the
existing stores, it should get a new store file in the `stores/` directory
rather than being added to App.jsx or an unrelated store.

## Consequences

- App.jsx is significantly simpler, serving as a layout/routing component
  rather than a state container
- Components access state directly via store hooks, eliminating prop drilling
- State changes are scoped -- a tab navigation change does not trigger
  re-renders in AWS-related components
- New Canonry features should define their state in a zustand store, not in
  component-level useState in App.jsx
- Store files live in the `stores/` directory within the Canonry app
