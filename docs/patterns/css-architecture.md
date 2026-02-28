<!-- drift-generated -->
# CSS Architecture Pattern Guide

**ADR:** [009-css-architecture-no-inline-styles](../adr/009-css-architecture-no-inline-styles.md)
**ESLint rule:** `local/no-inline-styles` (warn)

## Quick Rules

1. **No `style={{}}`** — all styling goes in `index.css` or component-scoped CSS
2. **Use utility classes** — `.flex-row`, `.mb-2`, `.ml-auto`, `.text-sm`, `.text-secondary`, etc.
3. **Use panel/metric/badge classes** — the design system provides `.panel`, `.metric-card`, `.badge`, etc.
4. **CSS custom properties** for theme tokens — never hardcode colors

## Global Styles

SVAP uses a single `index.css` for all styles. Views import no CSS files directly.

| Section | Purpose |
|---------|---------|
| `:root` variables | Color tokens, spacing, typography |
| Layout classes | `.panel`, `.panel-header`, `.panel-body`, `.view-header` |
| Metric classes | `.metrics-row`, `.metric-card`, `.metric-label`, `.metric-value` |
| Data classes | `.data-table`, `.td-mono`, `.td-muted` |
| Utility classes | `.flex-row`, `.mb-2`, `.ml-auto`, `.font-semibold`, `.text-sm`, `.text-secondary`, `.cursor-pointer` |
| Component classes | `.badge`, `.btn`, `.error-banner`, `.stage-chip` |

## Component Pattern

```tsx
// Use existing CSS classes — no inline styles
export function SomeView() {
  return (
    <div className="panel stagger-in">
      <div className="panel-header">
        <h3>Title</h3>
        <button className="btn btn-accent">Action</button>
      </div>
      <div className="panel-body">
        <div className="flex-row">
          <span className="font-semibold">Label</span>
          <span className="text-secondary ml-auto">Value</span>
        </div>
      </div>
    </div>
  );
}
```

## Dynamic Values (Only Acceptable `style={}` Use)

When a value is genuinely computed at runtime, use CSS custom properties:

```tsx
{/* eslint-disable-next-line local/no-inline-styles -- dynamic width from data */}
<div className="bar-fill" style={{ width: `${pct}%` }} />
```

This is the **only** acceptable use of `style={}`. Add an `eslint-disable` comment.

## Conditional Classes

Use template literals for simple conditionals:

```tsx
<div className={`panel ${isExpanded ? "expanded" : ""}`}>
<span className={`badge ${level}`}>{label}</span>
<span className={status === "completed" ? "stage-chip completed" : "stage-chip"}>
```

## What NOT To Do

```tsx
// BAD: inline style object
<div style={{ display: "flex", gap: "8px", padding: "12px" }}>

// BAD: style variable
const boxStyle = { border: "1px solid var(--border)" };
<div style={boxStyle}>

// GOOD: CSS class
<div className="flex-row">

// GOOD: utility class
<div className="mb-2 text-sm text-secondary">
```
