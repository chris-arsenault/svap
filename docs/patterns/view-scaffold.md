<!-- drift-generated -->
# View Scaffold Pattern

Every view in the frontend uses the same top-level structure.

## ViewHeader

```tsx
import { ViewHeader } from "../components/SharedUI";

<ViewHeader title="Page Title" description="One-line description of the view." />

// Dynamic descriptions use JSX:
<ViewHeader
  title="Detection Patterns"
  description={<>{count} anomaly signals across {groups} policies</>}
/>
```

## MetricCard

For views with a metrics summary row:

```tsx
import { MetricCard } from "../components/SharedUI";

<div className="metrics-row">
  <MetricCard label="Total" value={42} sub="items" />
  <MetricCard label="Active" value={10} sub="running" />
  <MetricCard label="Failed" value={2} sub={<span className="text-critical">needs attention</span>} />
</div>
```

`MetricCard` accepts:
- `label` — the metric name
- `value` — number or ReactNode
- `sub` — optional subtitle (string or ReactNode)
- `className` — optional additional class

## Adding a New View

```tsx
export default function MyView() {
  return (
    <div>
      <ViewHeader title="My View" description="What this view shows." />
      <div className="metrics-row">
        <MetricCard label="Items" value={count} sub="loaded" />
      </div>
      {/* view-specific content */}
    </div>
  );
}
```
