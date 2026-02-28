<!-- drift-generated -->
# Empty State Handling Pattern Guide

## Quick Rules

1. When a list or table has no data, show an empty-state message instead of an empty container
2. The message should tell the user what is missing and how to populate it
3. Use `className="empty-state"` for the wrapper div
4. Inside tables, use `<td colSpan>` for the empty row (CaseSourcing pattern)

## Standard Pattern (List/Card Views)

Replace the entire list or grid with a single centered message:

```tsx
{items.length === 0 ? (
  <div className="empty-state">
    No cases found. Run the pipeline to get started.
  </div>
) : (
  <div className="card-grid">
    {items.map((item) => (
      <Card key={item.id} {...item} />
    ))}
  </div>
)}
```

## Table Pattern (CaseSourcing)

When the empty state is inside a `<table>`, use a full-width table row:

```tsx
<table>
  <thead>
    <tr>
      <th>Name</th>
      <th>Status</th>
      <th>Actions</th>
    </tr>
  </thead>
  <tbody>
    {rows.length === 0 ? (
      <tr>
        <td colSpan={3} className="empty-state">
          No sources found. Upload enforcement documents to begin.
        </td>
      </tr>
    ) : (
      rows.map((row) => <SourceRow key={row.id} {...row} />)
    )}
  </tbody>
</table>
```

## Message Format

Follow this template for empty-state messages:

```
No {items} found. {Action} to get started.
```

Examples by view:

| View | Message |
|------|---------|
| CasesView | No cases found. Run the pipeline to get started. |
| CaseSourcing | No sources found. Upload enforcement documents to begin. |
| TaxonomyView | No taxonomy qualities found. Run Stage 2 to extract them. |
| DetectionView | No detection results found. Run Stage 4 to generate scores. |
| PredictionView | No predictions found. Run Stage 5 to generate them. |

## CSS

The `empty-state` class is defined in `frontend/src/index.css`:

```css
.empty-state {
  text-align: center;
  padding: 2rem;
  color: var(--text-muted);
}
```

## What NOT To Do

```tsx
// BAD: empty table with just headers and no rows
{items.length === 0 && <table><thead>...</thead><tbody></tbody></table>}

// BAD: no feedback at all
{items.length > 0 && items.map(...)}

// BAD: custom class name instead of the standard one
<div className="no-data-placeholder">Nothing here</div>

// GOOD: standard empty-state pattern
{items.length === 0 ? (
  <div className="empty-state">No cases found. Run the pipeline to get started.</div>
) : (
  items.map(...)
)}
```
