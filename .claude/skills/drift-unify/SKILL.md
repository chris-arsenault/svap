---
name: drift-unify
description: >
  Refactor a codebase to eliminate technical drift by converging variant implementations toward
  a chosen canonical pattern. Use this skill whenever the user wants to "unify", "standardize",
  "consolidate", "refactor for consistency", "make all X work the same way", or "convert to the
  canonical pattern". Also trigger when someone says "fix the drift in [area]", "make these
  consistent", or "apply the audit recommendations". This skill works in focused batches — one
  drift area per session — and produces refactored code, shared utilities, a changelog, and a
  backlog of remaining work.
---

# Drift Unify Skill

You are refactoring a codebase to eliminate technical drift. You work in focused, reviewable
batches — one drift area at a time, typically 5-15 files per session.

**Your role is to execute the user's architectural decisions, not to make them.** The user
(informed by drift-audit findings) tells you which pattern is canonical. You figure out how
to safely migrate files to that pattern.

## Prerequisites

The drift tool must be installed (`$DRIFT_SEMANTIC` set). If not, see the drift installation
instructions.

Before starting, you need two things:

1. **Which drift area** — what concept is being unified (the user tells you, or you read
   from a drift-manifest.json if one exists).

2. **Which pattern wins** — the user must confirm the canonical approach. If they haven't
   decided, help them compare variants from the audit, but let them choose.

Check for prior work:
```bash
PROJECT_ROOT="<path>"
ls "$PROJECT_ROOT/.drift-audit/drift-manifest.json" 2>/dev/null && echo "Manifest found"
ls "$PROJECT_ROOT/UNIFICATION_LOG.md" 2>/dev/null && echo "Prior unification work exists"
ls "$PROJECT_ROOT/DRIFT_BACKLOG.md" 2>/dev/null && echo "Backlog exists"
```

## Workflow

### Step 1: Understand the Canonical Pattern

Before changing anything, deeply understand the pattern the user chose as canonical:

1. **Read the canonical implementation(s).** The user should point you to 1-3 files that
   exemplify the target pattern. Read them thoroughly — understand the full shape: structure,
   state handling, error handling, types, composition, and edge cases.

2. **Read the variant implementations.** Read 1-2 examples of each non-canonical variant
   to understand what you'll be migrating FROM. Note what each variant does that the canonical
   pattern needs to handle.

3. **Identify the gap.** Does the canonical pattern already handle everything the variants do?
   Or does it need to be extended? Common gaps:
   - Variant handles an edge case the canonical doesn't
   - Variant uses a feature the canonical's approach can't express
   - Variant has performance characteristics the canonical would lose

   If there are gaps, discuss with the user before proceeding. The canonical pattern may need
   to be extended, or the variant may need to remain as an intentional exception.

### Step 2: Prepare Shared Infrastructure

If unification requires shared utilities that don't yet exist (shared components, hooks,
helper functions, types, etc.), create them first. Base them on the canonical implementation
the user identified — extract and generalize.

Place shared code wherever the project's conventions dictate. If unclear, ask the user.

### Step 3: Plan the Batch

Identify which files to change in this session. If a manifest exists, extract the file list:

```bash
# Read files for a specific drift area from the manifest
python3 -c "
import json, sys
m = json.load(open('$PROJECT_ROOT/.drift-audit/drift-manifest.json'))
area = next(a for a in m['areas'] if a['id'] == '<area-id>')
for v in area['variants']:
    # Skip the canonical variant — those files don't need changing
    for f in v['files']:
        print(f)
" 2>/dev/null
```

If there are more files than can be safely handled in one session (typically 15+), prioritize:
1. **Shared infrastructure** — files imported by many others (highest ripple effect)
2. **High-traffic features** — code paths users exercise frequently
3. **Simple cases** — files where the migration is straightforward (build momentum)

Tell the user the plan: "I'll convert these N files in this session. M files remain for
future sessions."

### Step 4: Refactor

For each file:

1. **Read the full file.** Understand everything it does, not just the drift pattern.
2. **Plan changes.** Identify every place the old pattern appears and what replaces it.
3. **Apply changes.** Edit the file, preserving all business logic and behavior.
4. **Verify.** Check imports, types, exported interface compatibility.

**Rules during refactoring:**

- **Preserve behavior.** You are changing HOW something is implemented, not WHAT it does.
  Every user-visible behavior, callback, error state, and edge case must be preserved.

- **Preserve interfaces.** A component's props, a function's signature, a hook's return
  value — these are contracts with the rest of the codebase. Don't break them unless the
  user explicitly approves breaking changes.

- **Preserve error handling.** If the old code handled errors (even badly), the new code
  must too. Improve error handling using the canonical approach, but don't drop it.

- **One file at a time.** Read, plan, edit, verify — for each file. Don't batch-edit
  blindly.

- **Flag uncertainty.** If you encounter a case where migration isn't straightforward,
  stop and discuss with the user rather than guessing. Add it to the backlog if needed.

- **Mark intentional exceptions.** If a file shouldn't be converted (performance reason,
  third-party constraint, etc.), document it as an intentional exception rather than
  forcing it into the canonical pattern.

### Step 5: Document What Changed

After completing the batch, update these files:

#### UNIFICATION_LOG.md (append)

```markdown
## [Date] — [Area Name]

### Canonical Pattern
[Brief: which pattern was chosen and where the reference implementation lives]

### Files Changed
- `path/to/file.ext` — [what was changed, one line]
- ...

### Shared Utilities Created/Updated
- `path/to/shared/thing.ext` — [created/updated, brief description]

### Intentional Exceptions
- `path/to/exception.ext` — [why this file was NOT converted]

### Breaking Changes
- [None, or describe what changed in public interfaces]

### Open Questions
- [Anything unresolved that came up during refactoring]
```

#### DRIFT_BACKLOG.md (update)

```markdown
# Drift Unification Backlog
Last updated: [date]

## In Progress
- [ ] [Area Name] — N/M files done (K remaining)
  - Next batch: [list files for next session]

## Queued
- [ ] [Another Area] — 0/N files
  - Depends on: [any prerequisites]

## Completed
- [x] [Finished Area] — N/N files (completed [date])

## Intentional Exceptions (not to be converted)
- `path/to/file.ext` — [reason]
```

#### Pattern Documentation (create if it doesn't exist)

For the drift area you just unified, create a pattern doc that future developers
(and future AI sessions) can reference. Save it wherever the project keeps docs.

Contents:
- What the canonical pattern is and why it was chosen
- How to use it (with a copy-paste-ready example)
- What NOT to do (the old variants, briefly)
- Where the reference implementation lives

This document is NOT an ADR (those explain WHY a decision was made). This is a
usage guide (it explains HOW to follow the decision). Both are valuable.

### Step 6: Summary

After completing the batch, tell the user:
- How many files were changed
- What shared utilities were created/updated
- What's left in the backlog
- Any issues or questions that came up
- Whether they should run tests, and what to look for

## Session Boundaries

Each session should focus on ONE drift area. If the user wants to tackle multiple
areas, do them sequentially — finish (or checkpoint) one before starting another.
This keeps changes reviewable and rollback-friendly.

If a file has drift in multiple areas, only fix the area you're currently working on.
Note the other drift in the backlog for a future session.
