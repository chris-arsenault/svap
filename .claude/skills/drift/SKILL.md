---
name: drift
description: >
  Orchestrate the full drift pipeline: audit, prioritize, unify, and guard. Wraps all 5 drift
  skills into a coordinated workflow with dependency-aware prioritization and progress tracking.

  Use this skill whenever the user wants to "run the drift pipeline", "audit and fix drift",
  "what should I unify next", "show drift progress", or any full-pipeline drift work. Also
  trigger for "drift plan", "drift unify", "drift guard", or just "drift".

  Phase commands: `/drift` (full pipeline), `/drift audit` (audits only), `/drift plan`
  (prioritize only), `/drift unify` (unify only), `/drift guard` (guard only).
---

# Drift Orchestrator

You are coordinating the full drift pipeline — from discovery through unification to
prevention. You wrap 5 sub-skills into a single coordinated workflow.

## Phase Routing

Parse the user's invocation to determine which phase to run:

| Invocation | Phase | What runs |
|-----------|-------|-----------|
| `/drift` or "run the drift pipeline" | `full` | audit → plan → unify → guard |
| `/drift audit` or "audit for drift" | `audit` | All three audits, then stop |
| `/drift plan` or "show drift plan" / "what should I unify next" | `plan` | Prioritize and present, then stop |
| `/drift unify` or "unify drift" / "fix drift" | `unify` | Unify planned areas, then stop |
| `/drift guard` or "guard against drift" / "lock patterns" | `guard` | Guard completed areas, then stop |

Each phase runs ONLY that phase and stops. `/drift` (no args) runs all phases sequentially.

---

## Phase: Audit

Run the semantic pipeline first, then all three audit methodologies, compiling a unified manifest.

### Step 0: Library Pull

If `.drift-audit/config.json` exists and `mode` is `"online"`, pull from the library:

```bash
drift library pull
```

Skip if the config file does not exist or mode is `"offline"`.

### Step 1: Run Semantic Pipeline

Run the full pipeline before any manual analysis. This is mandatory — it produces the
structural artifacts (fingerprints, similarity scores, clusters) that inform all three
audit phases.

```bash
PROJECT_ROOT="<path>"
bash "$DRIFT_SEMANTIC/cli.sh" run --project "$PROJECT_ROOT"
```

**Verification:** After the pipeline completes, confirm the artifacts exist:

```bash
ls .drift-audit/semantic/code-units.json .drift-audit/semantic/clusters.json
```

- Both exist → pipeline succeeded, proceed to Step 2.
- Only `code-units.json` exists → downstream stages failed. Run individual stages to
  isolate the failure:

```bash
bash "$DRIFT_SEMANTIC/cli.sh" fingerprint
bash "$DRIFT_SEMANTIC/cli.sh" typesig
bash "$DRIFT_SEMANTIC/cli.sh" callgraph
bash "$DRIFT_SEMANTIC/cli.sh" depcontext
bash "$DRIFT_SEMANTIC/cli.sh" score
bash "$DRIFT_SEMANTIC/cli.sh" cluster
bash "$DRIFT_SEMANTIC/cli.sh" css-extract --project "$PROJECT_ROOT"
bash "$DRIFT_SEMANTIC/cli.sh" css-score
bash "$DRIFT_SEMANTIC/cli.sh" report
```

- Neither file exists → extraction failed. Check `drift version` and diagnose before
  proceeding. Do not skip the pipeline and fall back to manual-only analysis.

### Step 2: Structural Audit

Read `$DRIFT_SEMANTIC/skill/drift-audit/SKILL.md` for the analysis methodology, then:
- Run `bash "$DRIFT_SEMANTIC/scripts/discover.sh" "$PROJECT_ROOT"` for raw inventory
- Perform intelligent analysis (read source files, identify drift areas)
- Use the pipeline's `code-units.json` to cross-reference extracted units
- Write findings to `.drift-audit/drift-manifest.json` and `.drift-audit/drift-report.md`

All structural entries should have `"type": "structural"` (or no type field — structural
is the default since drift-audit predates the type system).

### Step 3: Behavioral Audit

Read `$DRIFT_SEMANTIC/skill/drift-audit-ux/SKILL.md` for the analysis methodology, then:
- Work through the 7 behavioral domain checklist
- Read implementation code to understand actual behavior
- Build behavior matrices per domain
- Append findings to the existing manifest with `"type": "behavioral"`
- Append `## Behavioral Findings` section to drift-report.md

### Step 4: Semantic Audit

The pipeline already ran in Step 1. Read `$DRIFT_SEMANTIC/skill/drift-audit-semantic/SKILL.md`
for the cluster verification and purpose statement methodology (start from Phase 1).

- Verify pipeline clusters by reading source code — include code excerpts in every finding
- **Generate purpose statements** — this is mandatory, not optional. Purpose statements
  are your primary semantic contribution and the pipeline's highest-value input.
- Append findings to manifest with `"type": "semantic"`
- Append `## Semantic Findings` section to drift-report.md
- **Zero semantic findings is a failure state**, not a valid result for any non-trivial
  codebase. If the pipeline produced no clusters, diagnose why and produce manual findings.

### Step 5: Re-run Pipeline with Purpose Statements

After writing purpose statements, re-run the downstream stages to incorporate semantic
embeddings into the similarity scoring:

```bash
bash "$DRIFT_SEMANTIC/cli.sh" ingest-purposes --file .drift-audit/semantic/purpose-statements.json
bash "$DRIFT_SEMANTIC/cli.sh" embed
bash "$DRIFT_SEMANTIC/cli.sh" score
bash "$DRIFT_SEMANTIC/cli.sh" cluster
bash "$DRIFT_SEMANTIC/cli.sh" report
```

Review the updated clusters — purpose-enhanced scoring may surface new semantic findings
that structural signals alone missed.

### Step 6: Update Summary and Run Quality Gate

After all three audits, validate the manifest and recompute the summary:

```bash
drift validate "$PROJECT_ROOT" --fix-summary
```

This script:
- Recomputes the summary (area counts by impact/type, unique files, evidence coverage)
- Writes the corrected summary back to the manifest
- Runs the quality gate on every area:
  - `code_excerpts`: every variant has code excerpts with actual source
  - `line_ranges`: file paths use `path:startLine-endLine` format
  - `analysis_depth`: 3+ sentences of substantive analysis
  - `recommendation_specific`: 50+ chars with concrete targets
  - `semantic_purpose`: semantic findings reference purpose statements

**If any area fails the quality gate**, the script exits non-zero and reports which checks
failed. Fix the failing areas before presenting findings to the user. The quality gate is
non-negotiable — it's what distinguishes a useful audit from busywork.

### Re-Audit Behavior

If the manifest already exists, first check for regressions in previously completed areas:

```bash
drift plan-update "$PROJECT_ROOT" --check-regressions
```

This exits non-zero if any completed plan entries have regressed in the manifest. Use
`--json` for structured output including ADR violation flags.

Then each audit phase compares against existing entries:
- **New findings** are appended
- **Previously found areas** are compared — note if drift has worsened, improved, or been resolved
- **Completed areas** are checked for regression — if drift has returned, flag it prominently

#### ADR Violation Detection

When re-auditing finds drift in an area that was previously `completed`:

1. **Check if the area has an associated ADR:**
   Look in the attack plan's `guard_artifacts` for ADR paths, or search `docs/adr/`
   for ADRs whose Context section references the area's ID or name.

2. **If an ADR exists for a regressed area, this is an ADR violation:**
   - Flag it with `"severity": "violation"` in the finding — this is higher than HIGH
   - Include the ADR reference: "This area was resolved by ADR-NNNN but drift has
     returned, suggesting enforcement mechanisms have failed."
   - Check which enforcement mechanism failed (run the ADR enforcement check from
     the guard phase's Step 6 for this specific ADR)
   - ADR violations must appear at the **TOP** of re-audit findings, before any
     normal priority sorting

3. **If no ADR exists for a regressed area:**
   - This is a normal regression, not a violation
   - Recommend adding guard artifacts to prevent future regression

Present the combined findings when the audit phase is complete.

---

## Phase: Plan

Build a prioritized attack plan from the manifest. The plan script handles all mechanical
work: cross-type deduplication (Jaccard on file sets), dependency graph construction,
topological sorting with impact weighting, and merging with any existing plan.

### Step 1: Build Plan

```bash
drift plan "$PROJECT_ROOT"
```

This script:
- Deduplicates cross-type overlap (Jaccard > 0.5 on file sets → merge)
- Builds dependency DAG from file overlap (higher-impact blocks lower)
- Topologically sorts: impact desc → file count desc → variant count asc
- Merges with existing plan (preserves phase progress, flags regressions)
- Writes `.drift-audit/attack-plan.json`
- Outputs the ranked attack order

Use `--merge-threshold 0.6` to adjust dedup sensitivity, `--json` for machine output.

### Step 2: Present to User

Present the script's output to the user. The plan shows:
- **Ready to unify:** areas with all dependencies resolved
- **In progress:** areas mid-unification
- **Completed:** areas already unified + guarded
- **Blocked:** areas waiting on dependencies
- **Regressions:** previously completed areas where drift returned

Ask the user if they want to reorder or skip any areas. If changes are needed, edit
`.drift-audit/attack-plan.json` directly and re-run `drift plan "$PROJECT_ROOT"`.

Phase values: `pending` (in manifest but not yet planned), `planned` (approved for unification),
`unify` (unification in progress), `guard` (unified, guard pending), `completed` (fully done).

---

## Phase: Unify

Execute unification for all eligible areas in the attack plan.

### Execution Loop

1. Read `.drift-audit/attack-plan.json`
2. Find all areas where `phase` is `planned` AND all `depends_on` areas are `completed`
3. For each eligible area, in rank order:

#### Per-Area Workflow

Read `$DRIFT_SEMANTIC/skill/drift-unify/SKILL.md` and follow its complete methodology for this area:

**a. Determine canonical pattern.**
If `canonical_variant` is set in the plan, use it. Otherwise, read the manifest's
`recommendation` field and present the variant options to the user. The user picks the
canonical. Update the plan entry.

**b. Understand the canonical.**
Read the canonical implementation files thoroughly. Read 1-2 variant files to understand
what you're migrating from.

**c. Prepare shared infrastructure.**
If consolidation requires new shared components/hooks/utilities, create them first.

**d. Refactor files.**
For each non-canonical file in the area's manifest entry:
- Read the full file
- Plan the changes to align with the canonical pattern
- Apply changes, preserving all business logic and behavior
- Verify imports and types

**e. Document.**
- Append to `UNIFICATION_LOG.md` (what changed, what was created, exceptions, breaking changes)
- Update `DRIFT_BACKLOG.md` (what's left if the area isn't fully done)

**f. Update plan.**
Set the area's `phase` to `guard`. Record `unify_summary`. Update `drift-manifest.json`
status to `in_progress` or `completed`.

4. After all eligible areas are processed, present a consolidated summary:
   - Areas unified in this session
   - Files changed per area
   - Shared utilities created
   - Areas now eligible for guard
   - Areas still blocked

---

## Phase: Guard

Generate enforcement artifacts for all unified areas. **Hard enforcement (lint rules) comes
first and is mandatory. Documentation (ADRs, guides) comes second.**

Read `$DRIFT_SEMANTIC/skill/drift-guard/SKILL.md` and follow its two-phase methodology.

### Step 1: Generate Hard Enforcement for ALL Areas

1. Read `.drift-audit/attack-plan.json`
2. Find all areas where `phase` is `guard`
3. For EVERY area, generate lint rules BEFORE writing any documentation:

**Per-area rule generation:**

**a. Read the canonical pattern** (now the only pattern, post-unification).

**b. Read 1-2 old variant files** (from git history or unification log) to understand
what to ban.

**c. Generate ESLint rules** — at minimum one of:
   - `no-restricted-imports` banning non-canonical module paths
   - `no-restricted-syntax` banning old code patterns via AST selectors
   - Custom rule module for complex detection logic
   Use `warn` severity initially.

**d. Generate ast-grep rules** if the project uses ast-grep — for structural patterns
that ESLint selectors can't express.

**e. Apply TypeScript config changes** if tighter types prevent the drift
(e.g., `paths` aliases to enforce canonical import paths).

### Step 2: Wire All Rules and Verify

After generating rules for ALL areas (not one at a time):

1. Update the ESLint config to import and enable every new rule.
2. Run ESLint and report violation counts per rule.
3. If zero violations for a rule, verify it's actually matching (could indicate
   the rule isn't loaded or the selector is wrong).

Present the enforcement scoreboard:
```
Guard Enforcement:
  Areas guarded:        N/N
  ESLint rules:         N (M violations found)
  ast-grep rules:       N
  Config changes:       N
```

If any area has NO enforceable rule, explain specifically why to the user.

### Step 3: Generate Documentation for ALL Areas

Only after all rules are wired and verified.

**Every markdown file you create MUST start with `<!-- drift-generated -->` on line 1.**
Files without this marker won't sync to the library. This applies to ADRs, pattern docs,
and checklists — no exceptions.

**a. Write an ADR** for each area — first line `<!-- drift-generated -->`, then the
ADR content. The Enforcement section MUST reference the specific rule names created
in Step 1.

**b. Write/update pattern guide** — first line `<!-- drift-generated -->`, then
practical usage guide in `docs/patterns/`.

**c. Update review checklist** — first line `<!-- drift-generated -->`, then
drift-specific items covering what lint rules cannot catch (semantic correctness,
architectural intent, edge cases).

### Step 4: Finalize Areas in Plan

For each guarded area, finalize it in the plan and manifest:

```bash
drift plan-update "$PROJECT_ROOT" --finalize <area-id> --guard-artifacts file1.js docs/adr/001.md ...
```

This script sets the plan entry's phase to `completed`, records the guard artifacts list,
and updates the manifest area status to `completed`.

Present consolidated summary:
- ESLint rules created per area with violation counts
- ast-grep rules created
- Config changes made
- ADRs written (with enforcement section referencing rules)
- Pattern docs written
- Recommended rollout (warn → fix → error → CI)

### Step 5: Verify All Guard Artifacts

Run the full verification suite to confirm everything is wired correctly:

```bash
drift verify "$PROJECT_ROOT"
```

This runs three checks:

1. **Markers** — every file in sync directories has its drift marker on line 1.
   Files without markers won't sync to the library.
2. **ESLint** — every rule file in `eslint-rules/` is imported AND enabled in the
   ESLint config. Reports INTEGRATED / NOT INTEGRATED per rule.
3. **ADR** — every ADR's `## Enforcement` section references rules and docs that
   actually exist. Reports OK / DEGRADED / BROKEN per ADR.

Use `--check markers`, `--check eslint`, or `--check adr` to run individual checks.
Use `--json` for machine-readable output.

**If any check fails:**
- Missing markers → add the appropriate marker as line 1 (the report tells you which)
- Unintegrated rules → wire them into the ESLint config (see
  `$DRIFT_SEMANTIC/skill/drift-guard/references/eslint-rule-patterns.md`)
- Broken ADR enforcement → fix the missing referenced files

Re-run `drift verify "$PROJECT_ROOT"` after fixes until all checks pass.
Do not proceed to library push with failing checks.

---

## Full Pipeline (`/drift`)

When invoked with no phase argument, run all phases in sequence:

1. **Audit** — library pull (if online), run semantic pipeline, discover all drift
2. **Validate** — `drift validate "$PROJECT_ROOT" --fix-summary` (recompute summary + quality gate)
3. **Plan** — `drift plan "$PROJECT_ROOT"` then present to user for approval/reordering
4. **Unify** — resolve all planned areas autonomously
5. **Guard** — generate enforcement for all unified areas
6. **Finalize** — `drift plan-update "$PROJECT_ROOT" --finalize <id> --guard-artifacts ...` per area
7. **Verify** — `drift verify "$PROJECT_ROOT"` (markers + ESLint + ADR checks). Fix any failures.
8. **Library Push** — if `.drift-audit/config.json` has `"mode": "online"`, run
   `drift library push` to share guard artifacts to the centralized library.
   Check the push output for skipped files — any skipped file is a bug to fix.
9. **Summary** — present full pipeline results

The plan phase is the one human checkpoint in the full pipeline. After the user
approves the plan, unify and guard run autonomously with a summary at the end.

---

## Progress Tracking

The orchestrator maintains two sources of truth:

1. **`.drift-audit/drift-manifest.json`** — the findings (what drift exists).
   Updated by audit phases. Status field updated by unify/guard phases.

2. **`.drift-audit/attack-plan.json`** — the execution plan (what to do about it).
   Created by plan phase. Updated by unify/guard phases.

When starting any phase, always read both files to understand current state.
When completing any phase, update both files to reflect progress.

---

## Error Handling

- **Audit finds no drift:** Congratulate the user. Skip remaining phases.
- **Plan has no eligible areas:** All remaining areas are blocked by incomplete
  dependencies. Show what's blocking what and ask the user how to proceed.
- **Unify encounters an area too large for one session:** Document progress in
  the plan (keep phase as `unify`), note remaining files in DRIFT_BACKLOG.md,
  continue to next area.
- **Guard can't express a constraint in ESLint:** Document it as a review
  guideline in the checklist instead. Don't force imprecise rules.
- **Re-audit finds regression:** Flag it prominently. Ask user whether to
  re-plan the regressed area or investigate why the guard failed.
