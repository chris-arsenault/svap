<!-- drift-generated -->
# ADR-006: Shared Hook and Image Store API

## Status

Accepted (2026-02-26)

## Context

### Hook divergence

`useEditorState` existed in two versions: the shared package
(`@penguin-tales/shared-components`) tracked selection by index with no
persistence, while coherence-engine had a local copy that tracked selection
by ID with localStorage persistence via a `persistKey` option. The barrel
file in coherence-engine re-exported from the shared package, making the
local copy dead code — but the `persistKey` feature was silently lost.

### Image store bypass

`useImageUrl` from `@penguin-tales/image-store` returned `{ url, loading }`.
Illuminator bypassed this with a local hook that returned
`{ url, loading, error, metadata }` (reading directly from Dexie for
richer generation provenance data).

## Decision

### useEditorState

The shared `useEditorState` now supports:

- **ID-based selection** via `selectedId` state (derived from `idField`)
- **localStorage persistence** via optional `persistKey` option
- When `persistKey` is provided, the selected item ID survives page reloads

Local copies are deleted. All consumers import from
`@penguin-tales/shared-components`.

### useImageUrl

The shared `useImageUrl` now returns:

```typescript
{
  url: string | null;
  loading: boolean;
  error: string | null;
  metadata: ImageEntryMetadata | null;
}
```

`ImageEntryMetadata` includes generation provenance fields: `entityCulture`,
`originalPrompt`, `finalPrompt`, `revisedPrompt`, `generatedAt`, `model`,
`size`.

## Consequences

- Illuminator's local `useImageUrl` can be replaced by the shared version
- Any new hook that needs selection + persistence should use `useEditorState`
  with `persistKey`, not create a local version
- The `ImageEntryMetadata` type is the canonical metadata shape; adding new
  fields goes through the shared package
