<!-- drift-generated -->
# ADR-002: Store-First Data Access in Illuminator Components

**Status:** Accepted
**Date:** 2026-02-25
**Deciders:** tsonu
**Drift area:** #3 (State Management Architecture)

## Context

IlluminatorRemote acts as the top-level orchestrator for the illuminator MFE. After its decomposition into hooks and components, a `buildSharedProps` function assembled ~100 properties into a single bag that was spread to `IlluminatorTabContent` and `IlluminatorModals`. Many of these properties — `projectId`, `simulationRunId`, `worldContext`, `historianConfig`, `entityGuidance`, `cultureIdentities`, `queue`, `stats`, `eraTemporalInfo`, `navEntities`, `entityNavMap` — were already available via existing Zustand store selectors.

This created a redundant data path: the parent reads from stores, passes values as props, and children receive the same data they could read directly.

## Decision

Children read store-available values directly from Zustand stores instead of receiving them as props. The parent (`IlluminatorRemote`) continues to write to stores via `useEffect` (one-way sync), but no longer passes the read side through the props chain.

**Stores used by children:**

| Store | Values | Selector hooks |
|-------|--------|----------------|
| `useIlluminatorConfigStore` | projectId, simulationRunId, worldContext, historianConfig, entityGuidance, cultureIdentities | Direct selectors |
| `useEnrichmentQueueStore` | queue, stats | Direct selectors |
| `useEraTemporalInfo()` | eraTemporalInfo | Index selector |
| `useEntityNavList()` / `useEntityNavItems()` | navEntities, entityNavMap | Entity selectors |

**Mutation callbacks remain as props.** Setters like `updateWorldContext`, `updateHistorianConfig` etc. are parent-owned (they sync back to external consumers via Module Federation) and cannot be moved to stores without changing the ownership model.

**Tab renderers converted to React components.** The previous `render*Tab` plain functions (called via `TAB_RENDERERS[activeTab](props)`) were replaced with proper React components (`TAB_COMPONENTS[activeTab]`) rendered via JSX. This was required because plain functions cannot call hooks.

## Consequences

- ~12 props eliminated from the `buildSharedProps` chain
- Tab components are now proper React components with hook access
- Store reads are colocated with the components that use the data
- The `illuminatorConfigStore` becomes the canonical read path for project-level configuration
- Children are coupled to store shapes (acceptable — these stores are app-internal, not cross-MFE)

## Rules

1. **Never drill a value that exists in a store.** If a Zustand store or selector already provides the value, the consumer calls the hook.
2. **Mutation callbacks stay as props.** Parent-to-child callback props are the correct pattern for actions that need to propagate to external consumers.
3. **The config store is one-way.** `IlluminatorRemote` writes via `setConfig()`. Children read only. No child modifies the config store directly.
