<!-- drift-generated -->
# ADR-003: Grouped Flow Objects for Workflow State

**Status:** Accepted
**Date:** 2026-02-25
**Deciders:** tsonu
**Drift area:** #3 (State Management Architecture)

## Context

Illuminator has four independent workflow flows: summary revision, chronicle backport, historian annotation/edition, and world dynamics generation. Each flow hook returns 10-20 properties (run state, active flags, callbacks, modal state). Previously, `useIlluminatorFlows` spread all four flows into a single flat object:

```js
return { ...revision, ...backport, ...historian, ...dynamics };
```

This produced ~60 flat properties that were further merged into `buildSharedProps` alongside ~40 other properties, creating a ~100-property bag. Every child component received all ~100 properties via `{...sharedProps}`, regardless of which flow it actually used.

## Decision

Flow hooks return their state as grouped domain objects instead of flat property bags:

```js
return { revisionFlow, backportFlow, historianFlow, dynamicsFlow };
```

Consumer components destructure only the flow groups they need:

```jsx
function ChronicleTab({ backportFlow, historianFlow, ...props }) { ... }
function EntitiesTab({ revisionFlow, historianFlow, ...props }) { ... }
```

Modal sections receive only the flows they render:

```jsx
<RevisionSection revisionFlow={revisionFlow} />
<BackportSection backportFlow={backportFlow} revisionFlow={revisionFlow} />
<HistorianSection historianFlow={historianFlow} revisionFlow={revisionFlow} />
<DynamicsSection dynamicsFlow={dynamicsFlow} />
```

## Flow Group Definitions

| Group | Source Hook | Approx. Properties | Used By |
|-------|-----------|-------------------|---------|
| `revisionFlow` | `useRevisionFlow` | 12 | EntitiesTab, RevisionSection, BackportSection*, HistorianSection* |
| `backportFlow` | `useBackportFlow` | 17 | ChronicleTab, BackportSection |
| `historianFlow` | `useHistorianCallbacks` | 22 | EntitiesTab, ChronicleTab, HistorianSection |
| `dynamicsFlow` | `useDynamicsFlow` | 6 | ContextTab, DynamicsSection |

\* BackportSection and HistorianSection need `revisionFlow.getEntityContextsForRevision` for the shared entity context builder.

## Consequences

- Component interfaces are explicit about which workflows they participate in
- IDE autocomplete on `revisionFlow.` shows only revision-related properties
- Adding a new flow property doesn't silently bloat every component's props
- The flow hooks' return types are preserved as-is — no mapping or renaming at the boundary

## Rules

1. **Flow hooks never flat-spread into parent objects.** Always pass the flow object by name.
2. **Components destructure flow groups, not individual flow properties.** A component receives `revisionFlow` as a unit, not `revisionRun` + `isRevisionActive` + ... as individual props.
3. **Cross-flow dependencies are explicit.** If a modal needs `getEntityContextsForRevision` from the revision flow, it receives both `revisionFlow` and its own flow as separate props.
