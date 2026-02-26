# Dashboard.jsx — Build Requirements

## What This File Is

`src/views/Dashboard.jsx` is the landing page of the SVAP (Structural Vulnerability Analysis Pipeline) workstation UI. It's the only missing view component. The file should be placed at `/src/views/Dashboard.jsx` and exported as the default export.

## Context

This is a React/Vite intelligence-analyst workstation for analyzing HHS healthcare fraud vulnerabilities. The UI has 7 views navigated via a sidebar. Dashboard is the overview/summary view. All other views exist and work. The Dashboard is the only missing piece.

## Data Access

Import and use the pipeline context hook:

```js
import { usePipeline } from '../data/usePipelineData';
```

Inside the component call:

```js
const {
  cases,           // Array of case objects (see shape below)
  policies,        // Array of policy objects
  predictions,     // Array of prediction objects
  taxonomy,        // Array of 8 vulnerability qualities (V1-V8)
  counts,          // { cases: N, taxonomy_qualities: N, policies: N, predictions: N, detection_patterns: N }
  source,          // 'static' | 'live' — whether connected to backend API
  calibration,     // { threshold: number } — convergence score threshold
  loading,         // boolean
} = usePipeline();
```

### Data Shapes

**Case** (from `seedCases.js`):
```js
{
  case_id: "C001",
  case_name: "Operation Gold Rush — DME Catheter Fraud",
  scheme_mechanics: "...",
  exploited_policy: "Medicare Part B FFS...",
  enabling_condition: "...",
  scale_dollars: 10600000000,  // raw number
  detection_method: "...",
  qualities: ["V1", "V2", "V7", "V8"],  // which vulnerability qualities this case exhibits
}
```

**Policy** (from `seedPolicies.js`):
```js
{
  policy_id: "P001",
  name: "Home & Community-Based Services (HCBS)",
  description: "...",
  convergence_score: 6,   // how many vulnerability qualities converge (0-8)
  qualities: ["V1", "V2", "V4", "V5", "V6", "V8"],
  risk_level: "critical",  // "critical" | "high" | "medium" | "low"
}
```

**Prediction** (from `predictions.js`):
```js
{
  id: "PR001",
  policy_id: "P001",
  policy_name: "HCBS",
  convergence_score: 6,
  mechanics: "...",
  enabling_qualities: ["V1", "V2", "V4", "V5", "V6", "V8"],
  actor_profile: "...",
  detection_difficulty: "Medium — ...",
  lifecycle_stage: "...",
}
```

**Taxonomy quality** (from `seedTaxonomy.js`):
```js
{
  quality_id: "V1",
  name: "Payment Precedes Verification",
  definition: "...",
  recognition_test: "...",
  exploitation_logic: "...",
  color: "var(--v1)",
  case_count: 5,
}
```

**Convergence threshold** (from `seedPolicies.js`): `CONVERGENCE_THRESHOLD = 3` — policies at or above this score are flagged.

## Available UI Components

Import from `../components/SharedUI`:

```js
import { Badge, ScoreBar, QualityTags, formatDollars, RiskBadge } from '../components/SharedUI';
```

- `<Badge level="critical|high|medium|low|accent|neutral">{text}</Badge>` — colored pill
- `<ScoreBar score={6} max={8} threshold={3} />` — horizontal pip bar showing convergence score
- `<QualityTags ids={["V1","V2"]} />` — renders colored tags for vulnerability quality IDs
- `formatDollars(10600000000)` → `"$10.6B"` — compact dollar formatting
- `<RiskBadge level="critical" />` — renders `Badge` with label text CRITICAL/HIGH/MEDIUM/LOW

## Available CSS Classes

The design system is in `src/index.css`. Use these existing classes — **do not add new CSS**:

| Class | Purpose |
|-------|---------|
| `.view-header` | Top section of each view (contains `h2` and `.view-desc` paragraph) |
| `.metrics-row` | CSS grid row for metric cards (`repeat(auto-fit, minmax(180px, 1fr))`) |
| `.metric-card` | Individual stat card (contains `.metric-label`, `.metric-value`, `.metric-sub`) |
| `.panel` | Bordered container with dark background |
| `.panel-header` | Flex row inside panel (title left, badge/count right) |
| `.panel-body` | Panel content area; add `.dense` for no padding |
| `.data-table` | Full-width table with styled headers and hover rows |
| `.stagger-in` | Fade-slide-in animation (auto-staggers for children 1-6) |
| `.badge-*` | `critical`, `high`, `medium`, `low`, `accent`, `neutral` variants |

## Dashboard Layout Requirements

The Dashboard should show these sections top-to-bottom:

### 1. View Header
- Title: "Dashboard"
- Description: something like "SVAP pipeline overview — {source === 'live' ? 'connected to backend' : 'running on static seed data'}"
- Class: `.view-header .stagger-in`

### 2. Metrics Row
- 5 metric cards in a `.metrics-row`:
  - **Cases**: `counts.cases` (sub: "enforcement corpus")
  - **Qualities**: `counts.taxonomy_qualities` (sub: "vulnerability taxonomy")
  - **Policies Scanned**: `counts.policies` (sub: "analyzed")
  - **Predictions**: `counts.predictions` (sub: "exploitation predictions")
  - **Threshold**: `calibration.threshold` (sub: "convergence threshold")
- Each card uses `.metric-card` with `.metric-label`, `.metric-value`, `.metric-sub` children
- Each card gets `.stagger-in`

### 3. Highest-Risk Policies Table
- Panel with header "Highest-Risk Policies"
- Right side of panel-header: count badge
- Sort policies by `convergence_score` descending, show top 5
- `.data-table` with columns:
  - **Policy** — `p.name`, bold
  - **Convergence** — `<ScoreBar score={p.convergence_score} max={8} />`
  - **Risk** — `<RiskBadge level={p.risk_level} />`
  - **Qualities** — `<QualityTags ids={p.qualities} />`
- Table rows get `.stagger-in` with incremental `animationDelay`

### 4. Largest Cases Table
- Panel with header "Largest Enforcement Cases"
- Sort cases by `scale_dollars` descending, show top 5
- `.data-table` with columns:
  - **Case** — `c.case_name`, bold, truncated with ellipsis if long
  - **Scale** — `formatDollars(c.scale_dollars)`, monospace font, amber color
  - **Qualities** — `<QualityTags ids={c.qualities} />`
  - **Detection** — `c.detection_method`, muted color, smaller text

### 5. Calibration Note
- Small `.panel` at the bottom
- Text explaining the calibration finding, something like: "Every policy scoring ≥{calibration.threshold} on the convergence index corresponds to a program where enforcement fraud has already been documented. The taxonomy was derived from {counts.cases} enforcement cases and identifies {counts.taxonomy_qualities} structural vulnerability qualities."
- Muted text, smaller font, use `var(--text-secondary)` color

## Component Signature

```jsx
export default function Dashboard({ onNavigate }) {
  // onNavigate is passed from App.jsx for cross-view navigation
  // e.g. onNavigate('cases') switches to the CaseSourcing view
}
```

The `onNavigate` prop is available if you want to make table rows or section headers clickable to navigate to detail views (e.g., clicking a policy row navigates to `'policies'`). This is optional but nice to have.

## Design Principles

- **Dense information display** — this is an analyst workstation, not a marketing page
- **No gratuitous whitespace** — panels should be compact
- **Monospace for data** — use `fontFamily: 'var(--font-mono)'` for numbers and IDs
- **Stagger animations** — all top-level sections should have `.stagger-in`
- **Dark theme** — everything uses CSS variables from the design system, never hardcode colors
- **No emojis, no icons** — this is a text-driven utilitarian interface (no lucide-react imports needed)
- **Keep it under 120 lines** — other views average 100-140 lines; Dashboard should be similar

## Build Verification

After creating the file, run:
```bash
cd /home/claude/svap-ui && npx vite build
```

The build should succeed with 0 errors. The existing `App.jsx` already imports `Dashboard` from `./views/Dashboard` so no other file changes are needed.

## File Output

Place the completed file at:
- Working copy: `/home/claude/svap-ui/src/views/Dashboard.jsx`
- Also update the zip: re-zip the project to `/mnt/user-data/outputs/svap-ui-project.zip`
