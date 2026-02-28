# Semantic Drift Report
Generated: 2026-02-28 09:17 UTC
Units analyzed: 97
Clusters found: 4
CSS intra-file duplicates: 6

## Preliminary Clusters

> These clusters are based on structural and behavioral similarity signals. Semantic verification by Claude is pending.

### cluster-001: default, default, default, default, +3 more
**Members:** 7 | **Avg Similarity:** 0.40 | **Spread:** 1 directories
**Dominant Signal:** neighborhood

| Unit | Kind | File | Key Signals |
|------|------|------|-------------|
| default | function | frontend/src/App.tsx | neighborhood:1.00, typeSignature:1.00, behavior:0.86, imports:0.55 |
| default | function | frontend/src/views/DetectionView.tsx | neighborhood:1.00, typeSignature:1.00, behavior:0.86, imports:0.55 |
| default | function | frontend/src/views/DimensionRegistryView.tsx | neighborhood:1.00, typeSignature:1.00, behavior:0.86, imports:0.55 |
| default | function | frontend/src/views/DiscoveryView.tsx | neighborhood:1.00, typeSignature:1.00, behavior:0.86, imports:0.55 |
| default | function | frontend/src/views/ManagementView.tsx | neighborhood:1.00, typeSignature:1.00, behavior:0.86, imports:0.55 |
| default | function | frontend/src/views/ResearchView.tsx | neighborhood:1.00, typeSignature:1.00, behavior:0.86, imports:0.55 |
| default | function | frontend/src/views/TaxonomyView.tsx | neighborhood:1.00, typeSignature:1.00, behavior:0.86, imports:0.55 |

*Pending semantic verification*

### cluster-002: default, default, default, default, +2 more
**Members:** 6 | **Avg Similarity:** 0.40 | **Spread:** 1 directories
**Dominant Signal:** neighborhood

| Unit | Kind | File | Key Signals |
|------|------|------|-------------|
| default | function | frontend/src/views/CaseSourcing.tsx | neighborhood:1.00, typeSignature:1.00, behavior:0.82, imports:0.55 |
| default | function | frontend/src/views/ConvergenceMatrix.tsx | neighborhood:1.00, typeSignature:1.00, behavior:0.82, imports:0.55 |
| default | function | frontend/src/views/Dashboard.tsx | neighborhood:1.00, typeSignature:1.00, behavior:0.82, imports:0.55 |
| default | function | frontend/src/views/PolicyExplorer.tsx | neighborhood:1.00, typeSignature:1.00, behavior:0.82, imports:0.55 |
| default | function | frontend/src/views/PredictionView.tsx | neighborhood:1.00, typeSignature:1.00, behavior:0.82, imports:0.55 |
| default | function | frontend/src/views/SourcesView.tsx | neighborhood:1.00, typeSignature:1.00, behavior:0.82, imports:0.55 |

*Pending semantic verification*

### cluster-003: getSession, useError, useLoading
**Members:** 3 | **Avg Similarity:** 0.46 | **Spread:** 1 directories
**Dominant Signal:** consumerSet

| Unit | Kind | File | Key Signals |
|------|------|------|-------------|
| getSession | function | frontend/src/auth.ts | consumerSet:1.00, neighborhood:1.00, behavior:0.94, coOccurrence:0.86 |
| useError | hook | frontend/src/data/usePipelineSelectors.ts | consumerSet:1.00, neighborhood:1.00, behavior:0.94, coOccurrence:0.86 |
| useLoading | hook | frontend/src/data/usePipelineSelectors.ts | consumerSet:1.00, neighborhood:1.00, behavior:0.94, coOccurrence:0.86 |

*Pending semantic verification*

### cluster-004: signOut, useStatusSubscription
**Members:** 2 | **Avg Similarity:** 0.54 | **Spread:** 1 directories
**Dominant Signal:** consumerSet

| Unit | Kind | File | Key Signals |
|------|------|------|-------------|
| signOut | function | frontend/src/auth.ts | consumerSet:1.00, neighborhood:1.00, typeSignature:1.00, coOccurrence:0.86 |
| useStatusSubscription | hook | frontend/src/data/useStatusSubscription.ts | consumerSet:1.00, neighborhood:1.00, typeSignature:1.00, coOccurrence:0.86 |

*Pending semantic verification*


## CSS Intra-File Duplication

### index.css
Found 6 similar prefix groups within `frontend/src/index.css`:

| Group A | Group B | Score | Dominant Signal |
|---------|---------|-------|-----------------|
| split-view (3r) | detail-grid (3r) | 0.757 | categoryProfile |
| stage-dot (7r) | score-pip (6r) | 0.563 | categoryProfile |
| detection-step (7r) | tree-step (11r) | 0.542 | categoryProfile |
| detection-priority (6r) | stage-chip (5r) | 0.478 | categoryProfile |
| detection-step (7r) | detection-pattern (7r) | 0.430 | categoryProfile |
| tree-label (3r) | detection-step (7r) | 0.410 | categoryProfile |
