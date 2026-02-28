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

1. Install drift: `curl -fsSL https://raw.githubusercontent.com/chris-arsenault/drift/main/install.sh | bash`
   This sets `DRIFT_SEMANTIC=~/.drift-semantic` in your shell profile. Or set it manually
   to wherever you cloned the repo. The tool auto-installs its own dependencies on first run.
2. Install the skill to this project: `drift install-skill` (or it's already done if you're reading this)
3. Check if `.drift-audit/drift-manifest.json` exists — append semantic findings to it
4. Identify the shared component/utility library — this is where consolidated implementations
   would eventually live

## Method A: Tool-Assisted (Preferred)

Use this method when the drift CLI is available. It provides deterministic structural
analysis that you then verify semantically.

### Phase 0: Pipeline Health Check

Before running the full pipeline, verify the environment can support it:

```bash
# Check that the CLI can find Python and create venvs
drift version 2>&1 && echo "CLI OK"

# Try a quick extract to verify the full toolchain works
bash "$DRIFT_SEMANTIC/cli.sh" extract --project . 2>&1 | tail -5
```

If extraction succeeds, proceed with Phase 1. If it fails:
- **Node.js not found:** Tell the user to install Node.js (required for ts-morph extraction)
- **Python venv error:** The CLI auto-discovers Python via uv, pyenv, conda, and system
  paths. If it still fails, tell the user to run `uv python install 3.12` or
  `sudo apt install python3-venv`.
- **Any other error:** Run individual stages to isolate the failure (see Phase 1 error recovery).

### Phase 1: Run the CLI Tool

```bash
bash "$DRIFT_SEMANTIC/cli.sh" run --project .
```

This runs the full pipeline:
1. **Extract** — ts-morph parses all exported code units (types, JSX, hooks, imports, call
   graph, consumer graph, behavior markers)
2. **ast-grep** — structural pattern matching for common code shapes
3. **Fingerprint** — JSX hash, hook profile, import constellation, behavior flags
4. **Type signatures** — normalized type hashes with identifiers stripped
5. **Call graph vectors** — callee sets, call sequences, chain patterns
6. **Dependency context** — consumer profiles, co-occurrence, neighborhood hashes
7. **Embed** — TF-IDF embeddings of purpose statements (if available from Phase 3)
8. **Score** — pairwise similarity across all units using 13 signals
9. **Cluster** — graph-based community detection over similarity matrix
10. **CSS extract** — parse `.css` files, fingerprint rules, link to components
11. **CSS score** — pairwise CSS similarity and clustering
12. **Report** — preliminary report with structural clusters and CSS findings

**Error recovery:** If `drift run` fails partway through, run individual stages to isolate
the problem and salvage partial results:

```bash
# Run stages individually — each one that succeeds produces usable artifacts
bash "$DRIFT_SEMANTIC/cli.sh" extract --project .     # MUST succeed — everything depends on this
bash "$DRIFT_SEMANTIC/cli.sh" ast-grep --project .     # Optional — skips gracefully if sg not found
bash "$DRIFT_SEMANTIC/cli.sh" fingerprint              # Needs code-units.json
bash "$DRIFT_SEMANTIC/cli.sh" typesig                  # Needs code-units.json
bash "$DRIFT_SEMANTIC/cli.sh" callgraph                # Needs code-units.json
bash "$DRIFT_SEMANTIC/cli.sh" depcontext               # Needs code-units.json
bash "$DRIFT_SEMANTIC/cli.sh" score                    # Needs fingerprints
bash "$DRIFT_SEMANTIC/cli.sh" cluster                  # Needs scores
bash "$DRIFT_SEMANTIC/cli.sh" css-extract --project .  # CSS extraction (needs code-units.json)
bash "$DRIFT_SEMANTIC/cli.sh" css-score                # CSS scoring + clustering
bash "$DRIFT_SEMANTIC/cli.sh" report                   # Needs clusters
```

If only extraction succeeds, you still have `code-units.json` — proceed to Phase 3
(Purpose Statements) which doesn't require the downstream stages and is the highest-value
step you can do regardless of pipeline health.

Output goes to `.drift-audit/semantic/`. Key artifacts:
- `code-units.json` — all extracted units with full metadata
- `clusters.json` — ranked clusters of structurally similar code
- `semantic-drift-report.md` — preliminary report (pending your verification)

### Phase 2: Verify Clusters

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

### Phase 3: Purpose Statements (CRITICAL — This Is Your Primary Contribution)

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

#### Step 3: Ingest and re-run with semantic embeddings

```bash
bash "$DRIFT_SEMANTIC/cli.sh" ingest-purposes --file .drift-audit/semantic/purpose-statements.json
bash "$DRIFT_SEMANTIC/cli.sh" embed       # Built-in TF-IDF, no external services
bash "$DRIFT_SEMANTIC/cli.sh" score
bash "$DRIFT_SEMANTIC/cli.sh" cluster
bash "$DRIFT_SEMANTIC/cli.sh" report
```

The embed step uses built-in TF-IDF to compare purpose statements — no external
services required. The re-scored clusters now include semantic similarity as a 13th signal,
making clusters much more precise for catching functionally identical code with
different names.

If scoring/clustering fails, you still have the purpose statements. Use them in Phase 2
(verification) — manually group units with similar purposes and assess them as clusters.

### Phase 4: Targeted Exploration

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

### Phase 5: Present and Output

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

## Method B: Agent-Driven (Fallback)

Use this method when the CLI tool is not available or for quick targeted investigations.

### Phase 1: Role Discovery (Sampling)

Read a diverse sample of 30-50 files across the codebase. For each file, identify its
**functional role** — what purpose does it serve? What problem does it solve?

Build a **role taxonomy** — a list of functional roles you observe organically.

Common role categories you might find:

**UI Roles:**
- Action toolbar / button bar
- Data list with filtering/sorting/selection
- Detail view / inspector panel
- Configuration form / settings panel
- Status indicator / progress display
- Navigation container (tabs, sidebar, breadcrumbs)
- Empty state / placeholder
- Modal workflow (multi-step process in a modal)

**Data Roles:**
- Entity/record loader (fetches data from persistence)
- Schema/config consumer (reads configuration and applies it)
- Worker dispatcher (sends tasks to background workers)
- Queue/batch manager (processes items in order)
- Persistence writer (saves data to IndexedDB/storage)

**Behavioral Roles:**
- Async operation lifecycle (loading -> success/error pattern)
- Configuration provider (supplies settings to downstream code)
- Event coordinator (connects triggers to handlers)

### Phase 2: Systematic Search

For each role with 2+ implementations, systematically search for ALL implementations.

**Do NOT rely on naming patterns.** Instead:
1. Formulate what the implementation DOES functionally
2. Search for files using BROAD structural heuristics (JSX patterns, data access, lifecycle)
3. Read candidates and determine if they serve the same role
4. Group implementations by role

### Phase 3: Divergence Analysis

For each role cluster, compare across: interface divergence, behavior divergence,
scope divergence, consolidation potential (HIGH/MEDIUM/LOW).

### Phase 4: Generate Output

Write findings to `.drift-audit/drift-manifest.json` with `"type": "semantic"`.
Include `semantic_role`, `consolidation_assessment`, and `shared_interface_sketch` fields.

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
