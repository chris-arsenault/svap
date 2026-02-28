---
name: drift-audit-semantic
description: >
  Detect semantic duplication across a codebase — places where the same functional concept
  is implemented independently in multiple locations with different names, APIs, and
  implementations. Unlike structural drift (same library used differently) or behavioral
  drift (same interaction works differently), semantic drift is about the same PURPOSE
  being served by unrelated code that shares no naming or structural similarity.

  Examples: three components that all render "a horizontal bar of action buttons" but are
  named ButtonHeader, ToolBar, and GridComponent. Or three functions that all "load entity
  data from persistence" but are named loadWorldData(), fetchEntities(), and buildStateForSlot().

  Use this skill whenever the user mentions "semantic duplication", "same thing implemented
  multiple times", "consolidation opportunities", "shared component candidates", or describes
  finding functionally identical code under different names. Also trigger for "why do we have
  three ways to do X", "these should be the same component", or "full stack DRY audit".
---

# Semantic Drift Audit Skill

You are performing a **semantic drift audit** — discovering places where the same functional
concept is implemented independently in different parts of the codebase, potentially with
completely different names, APIs, and implementations.

This skill combines a **deterministic CLI tool** for structural analysis with **your semantic
understanding** for verification and interpretation. The tool parses ASTs, computes structural
fingerprints, and clusters similar code units. You verify whether those clusters represent
genuine semantic duplication.

## Prerequisites

The drift orchestrator runs the semantic pipeline before invoking this skill. The following
artifacts must exist in `.drift-audit/semantic/` before proceeding:

- `code-units.json` — all extracted units with full metadata
- `clusters.json` — ranked clusters of structurally similar code
- `semantic-drift-report.md` — preliminary report (pending your verification)

If these files do not exist, the pipeline failed — return to the orchestrator's Step 1 to
diagnose. Do not proceed with manual-only analysis.

Also check:
- `.drift-audit/drift-manifest.json` — append semantic findings to it if it exists
- Identify the shared component/utility library — this is where consolidated implementations
  would eventually live

### Phase 1: Verify Clusters

Read `clusters.json`. For each top-ranked cluster (start with top 10-20):

1. **Read the source code** of each cluster member (file paths in code-units.json)
2. **Assess semantic equivalence**: Do these units serve the same PURPOSE?
   - DUPLICATE: Same purpose, should be one implementation
   - OVERLAPPING: Significant shared purpose with some genuine differences
   - RELATED: Same category but different needs
   - FALSE_POSITIVE: Structural similarity is coincidental
3. **Note the dominant signal** — what made the tool think these are similar?
   If it's `jsxStructure`, the components render similar layouts.
   If it's `calleeSet`, they call the same functions.
   If it's `typeSignature`, they have the same interface shape.

Write your verdicts to a findings file:

```json
[
  {
    "clusterId": "cluster-001",
    "verdict": "DUPLICATE",
    "confidence": 0.9,
    "role": "action toolbar — horizontal bar of contextual action buttons",
    "sharedBehavior": ["renders button row", "uses icon imports", "triggers modal actions"],
    "meaningfulDifferences": [],
    "accidentalDifferences": ["different prop names", "different icon library"],
    "featureGaps": ["ButtonHeader has tooltip, ToolBar doesn't"],
    "consolidationComplexity": "LOW",
    "consolidationReasoning": "Shared ActionBar({ items }) would replace all three",
    "consumerImpact": "12 components import these across 3 apps",
    "code_excerpts": [
      {
        "unitId": "src/components/ButtonHeader.tsx::ButtonHeader",
        "file": "src/components/ButtonHeader.tsx",
        "start_line": 15,
        "end_line": 28,
        "snippet": "return (\n  <div className=\"button-header\">\n    {items.map(item => (\n      <button key={item.id} onClick={item.action}>\n        <Icon name={item.icon} />\n        {item.tooltip && <Tooltip>{item.tooltip}</Tooltip>}\n      </button>\n    ))}\n  </div>\n);"
      },
      {
        "unitId": "src/components/ToolBar.tsx::ToolBar",
        "file": "src/components/ToolBar.tsx",
        "start_line": 22,
        "end_line": 33,
        "snippet": "return (\n  <div className=\"toolbar-row\">\n    {actions.map(a => (\n      <IconButton key={a.key} icon={a.icon} onClick={a.handler} />\n    ))}\n  </div>\n);"
      }
    ],
    "target_interface": "ActionBar({ items: { id, icon, label, action, tooltip? }[], orientation?: 'horizontal' | 'vertical' })"
  }
]
```

Save to `.drift-audit/semantic/findings.json`, then re-generate the report:

```bash
bash "$DRIFT_SEMANTIC/cli.sh" ingest-findings --file .drift-audit/semantic/findings.json
bash "$DRIFT_SEMANTIC/cli.sh" report
```

### Phase 2: Purpose Statements (CRITICAL — This Is Your Primary Contribution)

This is the most important phase of the semantic audit. The pipeline detects structural
similarity — units that LOOK alike. Purpose statements detect semantic similarity — units
that DO the same thing regardless of how they look. **You must complete this phase even if
the pipeline partially failed.** If only `code-units.json` exists, that's enough to proceed.

**Why this matters:** Two components named `ButtonHeader` and `GridActions` with completely
different JSX trees, different hooks, and different imports might both serve the purpose
"renders a row of contextual action buttons for the current entity." Without purpose
statements, the pipeline can't detect this. With them, it can.

#### Step 1: Read the extracted units

```bash
# How many units were extracted?
python3 -c "import json; d=json.load(open('.drift-audit/semantic/code-units.json')); print(len(d), 'units')"
```

Read `code-units.json`. For large codebases (500+ units), prioritize:
- Components (highest semantic duplication risk)
- Hooks (second highest)
- Functions that access data stores or external APIs
- Skip type aliases, constants, and enums (rarely semantically duplicated)

#### Step 2: Write purpose statements

For each unit, read its source code and write a one-sentence description of its **functional
purpose** — what it does for the user or system, not how it's implemented.

**Good purpose statements** describe the WHAT and WHY:
- "Renders a horizontal bar of contextual action buttons for the currently selected entity"
- "Loads world metadata from IndexedDB and returns it with loading/error state"
- "Manages a queue of background AI generation tasks with progress tracking"

**Bad purpose statements** describe the HOW (implementation details):
- "A React component that uses useState and maps over an array" (too generic)
- "Exports a function" (useless)
- "Handles click events" (what does clicking DO?)

Write purpose statements in batches. For a 500-unit codebase, aim for at least 200
statements covering all components and hooks. Save as `purpose-statements.json`:

```json
[
  { "unitId": "src/components/ButtonHeader.tsx::ButtonHeader", "purpose": "Renders a horizontal bar of contextual action buttons for the current view" },
  { "unitId": "src/components/ToolBar.tsx::ToolBar", "purpose": "Renders a horizontal toolbar of action buttons with icons for entity operations" },
  { "unitId": "src/hooks/useWorldData.ts::useWorldData", "purpose": "Loads world metadata from IndexedDB and returns it with loading and error state" }
]
```

#### Step 3: Save purpose statements

Save the file to `.drift-audit/semantic/purpose-statements.json`. The orchestrator
will ingest these and re-run the downstream pipeline stages (embed → score → cluster →
report) to incorporate semantic embeddings into similarity scoring.

### Phase 3: Targeted Exploration

Use inspection commands to explore specific units or clusters:

```bash
# What's similar to a specific component?
bash "$DRIFT_SEMANTIC/cli.sh" inspect similar "src/components/ButtonHeader.jsx::ButtonHeader" --top 10

# Who imports this unit?
bash "$DRIFT_SEMANTIC/cli.sh" inspect consumers "src/hooks/useWorldDataLoader.ts::useWorldDataLoader"

# What does this unit call?
bash "$DRIFT_SEMANTIC/cli.sh" inspect callers "src/lib/EntityList.tsx::EntityList"

# Show cluster details
bash "$DRIFT_SEMANTIC/cli.sh" inspect cluster cluster-003
```

### Phase 4: Present and Output

Read `semantic-drift-report.md` and present findings to the user. Each semantic finding
must include concrete evidence — not just cluster IDs and scores.

**For each semantic finding, include:**
1. The **functional role** these units share (from your purpose statements)
2. **Code excerpts** (5-15 lines each) from at least 2 cluster members showing what they do
3. **Specific shared behaviors** — name the actual functions/hooks/patterns they have in common
4. **Specific differences** — which are accidental (naming, API shape) vs meaningful (features)
5. **Consolidation sketch** — a concrete interface for the unified replacement:
   ```tsx
   // Proposed shared component
   interface ActionBarProps {
     items: ActionItem[];
     orientation?: 'horizontal' | 'vertical';
     showTooltips?: boolean;  // Feature gap: only ButtonHeader has this
   }
   ```

Present findings in priority order:
1. Highest consolidation-potential clusters (DUPLICATE verdict, easy wins)
2. Clusters with the most implementations (widest duplication)
3. Infrastructure-level duplication (data loading, worker patterns)
4. Cases where you're unsure

Write findings to `.drift-audit/drift-manifest.json` as entries with `"type": "semantic"`.
Each entry must include `code_excerpts`, `implementation_details`, and `evidence_quality`
fields (same schema as structural findings). The report command handles manifest integration
automatically, but you must ensure the findings.json verdicts contain sufficient evidence.

---

## Scope Layers

Audit across three layers for full-stack semantic DRY:

### Layer 1: UI Components
Components that render the same kind of UI element or solve the same UX problem.

### Layer 2: Data & Infrastructure
Functions, hooks, and modules that perform the same data operation or infrastructure task.

### Layer 3: Behavioral Contracts
Cross-cutting concerns implemented differently across features.

## What IS semantic drift

- Three components that all render "a toolbar of action buttons" but are named `ButtonHeader`,
  `ToolBar`, and `ActionsRow`
- Two hooks that both "load entities from Dexie and return them with loading state" but one
  is called `useWorldDataLoader` and the other is inline in a component's useEffect
- Four places that "send a task to a web worker and track its progress" with four different
  message formats

## What is NOT semantic drift

- A search filter component and a settings form both use text inputs — different purposes
- A simple confirmation dialog and a multi-step wizard both use modals — different complexity
- App-specific business logic that happens to use similar patterns — coincidental similarity

## Scope Control

If the user wants to focus on a specific layer (just UI components, just data patterns),
respect that. A targeted audit is perfectly valid. Use `inspect` and `search` commands
for focused exploration.
