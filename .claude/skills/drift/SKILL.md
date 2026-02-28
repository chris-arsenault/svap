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

Run all three audit methodologies sequentially, compiling a unified manifest.

### Step 1: Structural Audit

Read `$DRIFT_SEMANTIC/skill/drift-audit/SKILL.md` and follow its complete methodology:
- Run `bash "$DRIFT_SEMANTIC/scripts/discover.sh" "$PROJECT_ROOT"` for raw inventory
- Perform intelligent analysis (read source files, identify drift areas)
- Write findings to `.drift-audit/drift-manifest.json` and `.drift-audit/drift-report.md`

All structural entries should have `"type": "structural"` (or no type field — structural
is the default since drift-audit predates the type system).

### Step 2: Behavioral Audit

Read `$DRIFT_SEMANTIC/skill/drift-audit-ux/SKILL.md` and follow its complete methodology:
- Work through the 7 behavioral domain checklist
- Read implementation code to understand actual behavior
- Build behavior matrices per domain
- Append findings to the existing manifest with `"type": "behavioral"`
- Append `## Behavioral Findings` section to drift-report.md

### Step 3: Semantic Audit

Read `$DRIFT_SEMANTIC/skill/drift-audit-semantic/SKILL.md` and follow its complete methodology:
- Run the pipeline health check (Phase 0) before attempting the full pipeline
- Run `bash "$DRIFT_SEMANTIC/cli.sh" run --project .` for tool-assisted analysis
- **Generate purpose statements** (Phase 3) — this is mandatory, not optional. Purpose
  statements are your primary semantic contribution and the pipeline's highest-value input.
- If the pipeline fails, fall back to individual stages. If only extraction works, purpose
  statements alone still enable meaningful semantic analysis.
- Verify clusters by reading source code — include code excerpts in every finding
- Append findings to manifest with `"type": "semantic"`
- Append `## Semantic Findings` section to drift-report.md
- **Zero semantic findings is a failure state**, not a valid result for any non-trivial
  codebase. If the pipeline produced no clusters, diagnose why and produce manual findings.

### Step 4: Update Summary

After all three audits, recompute the manifest's `summary` field:

```json
{
  "total_drift_areas": "<count all areas>",
  "total_files_affected": "<sum unique files across all areas>",
  "high_impact": "<count HIGH areas>",
  "medium_impact": "<count MEDIUM areas>",
  "low_impact": "<count LOW areas>",
  "by_type": {
    "structural": "<count>",
    "behavioral": "<count>",
    "semantic": "<count>"
  },
  "evidence_coverage": {
    "high": "<count areas with code_excerpts + line ranges>",
    "medium": "<count areas with file paths only>",
    "low": "<count areas with observation only>"
  }
}
```

### Step 5: Quality Gate

Before presenting findings to the user, validate that every finding has sufficient evidence.
This prevents shallow, generic output that could apply to any codebase.

**For each area in the manifest, verify:**

1. **Code excerpts exist** — every variant must have at least one `code_excerpts` entry with
   actual source code (not a description of code). If missing, go back and read the files.

2. **File paths include line ranges** — `files` entries should be `path:startLine-endLine`,
   not bare file paths. If missing, re-read the files and add line ranges.

3. **Analysis has substance** — the `analysis` field must be 3+ sentences covering: why the
   drift exists, concrete tradeoffs, and what convergence looks like. If it's 1-2 generic
   sentences, rewrite it.

4. **Recommendations are actionable** — the `recommendation` field must include a concrete
   target (API sketch, component interface, migration path). "Extract a shared component"
   is not actionable. "Extract `ModalShell({ onClose, escapeClose?, scrollLock?, children })`
   into `packages/shared-components/`" is actionable.

5. **Semantic findings have purpose statements** — every semantic finding must reference
   purpose statements that demonstrate functional equivalence. If the semantic pipeline
   produced zero findings, that's a pipeline failure, not an absence of semantic drift.
   Fall back to manual analysis.

**If any finding fails the quality gate**, fix it before presenting to the user. The
quality gate is non-negotiable — it's what distinguishes a useful audit from busywork.

### Re-Audit Behavior

If the manifest already exists, each audit phase compares against existing entries:
- **New findings** are appended
- **Previously found areas** are compared — note if drift has worsened, improved, or been resolved
- **Completed areas** are checked for regression — if drift has returned, flag it prominently

Present the combined findings when the audit phase is complete.

---

## Phase: Plan

Read `.drift-audit/drift-manifest.json` and produce a prioritized attack order.

### Step 1: Build Dependency Graph

For each area in the manifest:

1. **Explicit dependencies:** Scan the `analysis` and `recommendation` fields for references
   to other areas (by ID or by name). Example: "resolving area #1 would largely resolve this"
   means this area depends on area #1.

2. **File overlap dependencies:** If two areas share files, the higher-impact area should be
   resolved first (changing shared files twice creates churn).

3. **Logical dependencies:** Some dependencies are domain-logical even without textual
   references. Build config depends on dependency versions. TypeScript config depends on
   TypeScript version. Use judgment.

Produce a DAG of area IDs.

### Step 1b: Deduplicate Cross-Type Overlap

The three audit types have genuine overlap. The semantic tool's structural fingerprinting
can surface findings that also appear in structural or behavioral audits (e.g., "these
components all handle loading states differently" may appear as both a behavioral Domain 4
finding and a semantic cluster). Before prioritizing, merge overlapping entries:

1. **Detect overlap by file sets.** For every pair of areas, compute the Jaccard similarity
   of their file sets (union of all variant files). If overlap > 0.5, they likely describe
   the same drift from different angles.

2. **Merge strategy.** When two areas overlap:
   - Keep the **higher-impact** entry as the primary. If equal impact, prefer semantic
     (it has richer metadata: cluster scores, signal breakdowns, consolidation reasoning).
   - Merge the other entry's unique files and variants into the primary.
   - Append the other entry's `analysis` text to the primary's analysis as an
     "Also noted by [type] audit:" addendum.
   - Record the merged area's ID in a `merged_from` array on the primary entry.
   - Delete the secondary entry from the manifest.

3. **Log merges.** When presenting the plan, note which areas were merged so the user
   understands why a behavioral finding disappeared (it was absorbed into a semantic one).

Common overlaps to watch for:
- Behavioral Domain 4 (loading/error states) ↔ semantic clusters with `hasLoadingState`/`hasErrorHandling` behavior signals
- Behavioral Domain 2 (shared component adoption) ↔ semantic clusters where one member is in the shared library
- Structural "naming cluster" findings ↔ semantic clusters of the same units

### Step 2: Topological Sort with Impact Weighting

Within each dependency tier (areas whose dependencies are all resolved or have none):

1. **Impact:** HIGH (3) > MEDIUM (2) > LOW (1)
2. **File count:** More files = higher priority (larger blast radius, more value from early resolution)
3. **Variant count:** Fewer variants = simpler unification = quicker win (tiebreaker)

Sort by impact descending, then file count descending, then variant count ascending.

### Step 3: Merge with Existing Plan

If `.drift-audit/attack-plan.json` exists:
- Preserve phase progress for areas already in the plan
- Add newly discovered areas at appropriate rank positions
- Remove areas that no longer appear in the manifest (resolved naturally)
- Flag areas that regressed from `completed` back to having drift

If no plan exists, create it fresh.

### Step 4: Present to User

Display the ranked attack order:

```
Drift Attack Plan (N areas, M completed, K remaining)

Ready to unify:
  1. [HIGH] Area Name (X files, Y variants)
  2. [HIGH] Another Area (X files, Y variants) — depends on #1

In progress:
  3. [MEDIUM] Area Name — unify phase, 8/15 files done

Completed:
  ✓ Area Name — unified + guarded
  ✓ Another Area — unified + guarded

Blocked:
  5. [MEDIUM] Area Name — blocked by #1 (not yet completed)
```

Ask the user if they want to reorder or skip any areas. Apply their changes.

### Step 5: Save Plan

Write the plan to `.drift-audit/attack-plan.json`:

```json
{
  "created": "ISO-8601",
  "updated": "ISO-8601",
  "plan": [
    {
      "area_id": "kebab-case-id",
      "rank": 1,
      "depends_on": [],
      "phase": "planned",
      "canonical_variant": null,
      "unify_summary": null,
      "guard_artifacts": []
    }
  ]
}
```

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

Generate enforcement artifacts for all unified areas.

### Execution Loop

1. Read `.drift-audit/attack-plan.json`
2. Find all areas where `phase` is `guard`
3. For each area:

#### Per-Area Workflow

Read `$DRIFT_SEMANTIC/skill/drift-guard/SKILL.md` and follow its complete methodology:

**a. Read the canonical pattern** (now the only pattern, post-unification).

**b. Generate ESLint rules** — import restrictions, API usage restrictions, structural
pattern bans. Use `warn` severity initially.

**c. Write an ADR** — document what was decided and why. Number sequentially in `docs/adr/`.

**d. Write/update pattern guide** — practical usage guide in `docs/patterns/`.

**e. Update review checklist** — add drift-specific items.

**f. Update plan.**
Set area's `phase` to `completed`. Record `guard_artifacts` (list of files created).
Update `drift-manifest.json` status to `completed`.

4. Present consolidated summary:
   - ESLint rules created (with violation counts if measurable)
   - ADRs written
   - Pattern docs written
   - Recommended rollout (warn → fix → error → CI)

---

## Full Pipeline (`/drift`)

When invoked with no phase argument, run all phases in sequence:

1. **Audit** — discover all drift
2. **Plan** — prioritize and present to user for approval/reordering
3. **Unify** — resolve all planned areas autonomously
4. **Guard** — generate enforcement for all unified areas
5. **Summary** — present full pipeline results

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
