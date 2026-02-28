<!-- drift-generated -->
# Expand/Collapse Pattern

All expandable UI elements use shared hooks from `hooks.ts`.

## Single-expand (one item at a time)

```tsx
import { useExpandSingle, expandableProps } from "../hooks";

const { expandedId, toggle } = useExpandSingle();

// In JSX:
<div
  className="panel-header clickable"
  {...expandableProps(() => toggle(item.id))}
>
  {expandedId === item.id ? <ChevronDown /> : <ChevronRight />}
  {item.name}
</div>
{expandedId === item.id && <DetailPanel item={item} />}
```

## Multi-expand (multiple items open simultaneously)

```tsx
import { useExpandSet, expandableProps } from "../hooks";

const { expanded, toggle, set, reset } = useExpandSet();

// In JSX:
<div {...expandableProps(() => toggle(item.id))}>
  {expanded.has(item.id) ? <ChevronDown /> : <ChevronRight />}
</div>

// Expand all / collapse all:
<button onClick={() => set(new Set(allIds))}>Expand all</button>
<button onClick={reset}>Collapse all</button>
```

## expandableProps

Returns `{ role, tabIndex, onClick, onKeyDown }` for keyboard accessibility.
Handles Enter and Space key presses. Always use this instead of manual handlers.

```tsx
// Replaces:
role="button"
tabIndex={0}
onClick={() => onToggle(id)}
onKeyDown={(e) => { if (e.key === "Enter" || e.key === " ") { ... } }}

// With:
{...expandableProps(() => onToggle(id))}
```

## When to use which

| Scenario | Hook |
|----------|------|
| Card grid, detail panel, feed list | `useExpandSingle` |
| Tree with multiple open branches | `useExpandSet` |
| Per-node local state (recursive) | `useState(bool)` is acceptable |
