<!-- drift-generated -->
# Store Access Pattern Guide

**ADR:** [013-unified-store-access](../adr/013-unified-store-access.md)
**ESLint rule:** `local/no-direct-store-import` (warn)

## Quick Rules

1. **Never import `usePipelineStore` in views** — use hooks from `usePipelineSelectors`
2. **Actions**: individual hooks (`useRunPipeline()`, `useFetchDiscovery()`, etc.)
3. **Data**: individual hooks (`useCases()`, `usePolicies()`, `useThreshold()`, etc.)
4. Only `pipelineStore.ts` and `api.ts` import the store directly

## Pattern

```tsx
// GOOD — individual selector hooks
import { useRunPipeline, useCases, useTaxonomy } from "../data/usePipelineSelectors";

export default function MyView() {
  const runPipeline = useRunPipeline();  // stable ref, no re-renders
  const cases = useCases();              // subscribes to exactly one slice
  const taxonomy = useTaxonomy();
  // ...
}
```

```tsx
// BAD — direct store import
import { usePipelineStore } from "../data/pipelineStore";

export default function MyView() {
  const cases = usePipelineStore((s) => s.cases);  // works but bypasses the pattern
  // ...
}
```

## Available Hooks

### Action Hooks (stable references, never cause re-renders)

| Hook | Store action |
|------|-------------|
| `useRefresh()` | `refresh` |
| `useRunPipeline()` | `runPipeline` |
| `useSeedPipeline()` | `seedPipeline` |
| `useFetchDiscovery()` | `fetchDiscovery` |
| `useRunDiscoveryFeeds()` | `runDiscoveryFeeds` |
| `useReviewCandidate()` | `reviewCandidate` |
| `useFetchDimensions()` | `fetchDimensions` |
| `useFetchTriageResults()` | `fetchTriageResults` |
| `useRunTriage()` | `runTriage` |
| `useRunDeepResearch()` | `runDeepResearch` |
| `useFetchResearchSessions()` | `fetchResearchSessions` |
| `useFetchFindings()` | `fetchFindings` |
| `useFetchAssessments()` | `fetchAssessments` |
| `useUpdatePipelineStatus()` | `updatePipelineStatus` |

### Data Hooks (subscribe to one slice each)

| Hook | Store field |
|------|------------|
| `useRunId()` | `runId` |
| `useCases()` | `cases` |
| `useTaxonomy()` | `taxonomy` |
| `usePolicies()` | `policies` |
| `useThreshold()` | `threshold` |
| `useExploitationTrees()` | `exploitationTrees` |
| `useDetectionPatterns()` | `detectionPatterns` |
| `useEnforcementSources()` | `enforcementSources` |
| `useSourceFeeds()` | `sourceFeeds` |
| `useSourceCandidates()` | `sourceCandidates` |
| `useDimensions()` | `dimensions` |
| `useTriageResults()` | `triageResults` |
| `useResearchSessions()` | `researchSessions` |
| `usePolicyCatalog()` | `policyCatalog` |
| `useScannedPrograms()` | `scannedPrograms` |
| `useDataSources()` | `dataSources` |

## Exception: Dashboard useShallow

Dashboard subscribes to 5+ data fields for its metrics display. Using individual hooks
would cause 5+ separate subscriptions with potential render cascading. A single `useShallow`
call with all needed fields is acceptable for render-heavy components:

```tsx
const { cases, taxonomy, policies, threshold, exploitationTrees } = usePipelineStore(
  useShallow((s) => ({
    cases: s.cases,
    taxonomy: s.taxonomy,
    policies: s.policies,
    threshold: s.threshold,
    exploitationTrees: s.exploitationTrees,
  })),
);
```

This exception is limited to Dashboard. Other views should use individual hooks.
