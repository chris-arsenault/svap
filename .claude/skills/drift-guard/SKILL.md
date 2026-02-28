---
name: drift-guard
description: >
  Generate automated guardrails to prevent technical drift from returning to a codebase after
  unification. Produces custom ESLint rules, Architecture Decision Records, pattern documentation,
  and review checklists — all derived from the actual canonical patterns the user chose, not from
  predetermined templates. Use this skill whenever the user wants to "prevent regression", "add
  linting rules", "enforce patterns", "create ADRs", "document architectural decisions", "guard
  against drift", or "lock in the canonical patterns". Also trigger when someone says "make sure
  this doesn't drift again", "how do I enforce this", or "write rules so future code follows the
  pattern". Works best after drift-unify but can be used standalone.
---

# Drift Guard Skill

You are creating automated guardrails to prevent technical drift from returning after unification.
You produce ESLint rules, ADRs, and documentation — all based on the actual decisions made for
THIS codebase, not generic best practices.

**Everything you generate must be derived from the project's actual canonical patterns.** Read the
codebase, read the unification log, understand what was decided and why, then create enforcement
that matches.

## Prerequisites

The drift tool must be installed (`$DRIFT_SEMANTIC` set). If not, see the drift installation
instructions.

Orient yourself:
```bash
PROJECT_ROOT="<path>"

# What unification work has been done?
cat "$PROJECT_ROOT/UNIFICATION_LOG.md" 2>/dev/null || echo "No unification log"
cat "$PROJECT_ROOT/.drift-audit/drift-manifest.json" 2>/dev/null | python3 -c "
import json, sys
m = json.load(sys.stdin)
for a in m.get('areas', []):
    print(f\"{a['status']:10} {a['impact']:6} {a['name']}\")
" 2>/dev/null || echo "No manifest"

# What's the ESLint setup?
ls "$PROJECT_ROOT"/eslint.config.* 2>/dev/null && echo "Flat config"
ls "$PROJECT_ROOT"/.eslintrc* 2>/dev/null && echo "Legacy config"
cat "$PROJECT_ROOT/package.json" | python3 -c "
import json, sys
pkg = json.load(sys.stdin)
deps = {**pkg.get('dependencies', {}), **pkg.get('devDependencies', {})}
eslint_deps = {k: v for k, v in deps.items() if 'eslint' in k.lower()}
for name, ver in sorted(eslint_deps.items()):
    print(f'  {name}: {ver}')
" 2>/dev/null

# What docs already exist?
ls "$PROJECT_ROOT"/docs/ 2>/dev/null
```

Ask the user which unified patterns to guard. If a unification log exists, use it as context.

## What to Produce

For each unified drift area, generate the artifacts below. The exact content depends entirely
on what was unified — you determine the right rules and docs by reading the canonical patterns
and the unification log.

### 1. ESLint Rules

Read `$DRIFT_SEMANTIC/skill/drift-guard/references/eslint-rule-patterns.md` for the mechanical
details of writing rules. But the WHAT to enforce comes from the codebase, not from that reference.

**Process for each drift area:**

1. **Read the canonical pattern** — the actual files, not a description of them.
2. **Read 1-2 old variant files** (from git history or unification log) to understand what
   should be banned.
3. **Identify enforceable boundaries.** Not everything can be enforced by a linter. Focus on:
   - **Import restrictions** — banning imports of deprecated modules/components
   - **API usage restrictions** — banning direct use of low-level APIs that should go
     through an abstraction
   - **Naming conventions** — enforcing consistent naming for similar concepts
   - **Structural patterns** — banning specific AST patterns that represent old approaches
4. **Write rules that are precise.** A rule with false positives will be disabled. If you
   can't express the constraint precisely in ESLint, document it as a review guideline instead.
5. **Use `warn` severity initially.** Upgrade to `error` after all violations are fixed.
6. **Include helpful messages.** Every violation message should:
   - Explain what's wrong in plain language
   - Say what to do instead
   - Link to the pattern documentation

**Choose the right enforcement mechanism:**
- `no-restricted-imports` — simplest, for banning specific import paths
- `no-restricted-syntax` with AST selectors — for banning specific code shapes
- Custom rule module — for anything more complex (file-path-aware rules, counting rules, etc.)

For the project's ESLint config format, see `$DRIFT_SEMANTIC/skill/drift-guard/references/eslint-rule-patterns.md`
for how to wire up each mechanism.

### 2. Architecture Decision Records

Create an ADR for each significant unification decision. See
`$DRIFT_SEMANTIC/skill/drift-guard/references/adr-template.md` for the template.

**An ADR answers "why did we decide this?"** It captures:
- What the problem was (the drift)
- What alternatives existed (the variants)
- What was chosen and why
- What the tradeoffs are

**Important: derive ADRs from the actual decisions made.** Read the unification log and
the user's stated reasoning. Don't invent reasons or add your own opinions about why a
pattern was chosen — document the actual decision-making process.

Save ADRs to `docs/adr/` (or wherever the project keeps documentation). Number sequentially.
Check for existing ADRs first to continue the sequence.

### 3. Pattern Usage Guides

For each unified area, ensure a usage guide exists that shows developers how to follow the
canonical pattern. These should be practical, copy-paste-ready documents.

If drift-unify already created pattern docs, review and enhance them. If not, create them
by reading the canonical implementation and writing a usage guide.

### 4. Review Checklist

Generate or update a PR review checklist that includes drift-prevention items. Match the
format the project already uses (GitHub PR template, CONTRIBUTING.md, or standalone doc).

Each checklist item should:
- Be specific to a real drift area (not generic "follow best practices")
- Reference the relevant pattern doc or ADR
- Be verifiable by a reviewer in under 30 seconds

### 5. Drift Guard Configuration File (optional)

If the user plans to re-run drift-audit periodically, create a `.drift-audit/config.json`
that records which patterns are canonical so future audits can check for regression:

```json
{
  "canonical_patterns": [
    {
      "area": "the drift area id from manifest",
      "canonical_variant": "the variant name that won",
      "enforced_by": ["eslint-rule-name", "review-checklist"],
      "adr": "docs/adr/0001-whatever.md",
      "pattern_doc": "docs/patterns/whatever.md"
    }
  ]
}
```

## After Generating Artifacts

If the project has a drift library configured (`.drift-audit/config.json` with tags),
suggest running `drift library publish` to share the generated ESLint rules, ADRs, and
pattern docs with other projects that share the same tags.

## Rollout Guidance

After generating guard artifacts, recommend a rollout plan to the user:

1. **Commit rules as `warn`.** This surfaces violations without blocking work.
2. **Fix remaining violations.** There may be files that weren't converted yet.
3. **Upgrade to `error`.** Once clean, rules become hard gates.
4. **Add to CI.** If the project has CI, the rules should run there too.

Tell the user how to check current violation count:
```bash
npx eslint src/ --format compact 2>/dev/null | grep -c "Warning\|Error" || echo "0 violations"
```

## Maintenance

Guard artifacts need maintenance as the codebase evolves:
- New drift areas need new rules and ADRs
- Pattern docs need updating when canonical patterns evolve
- Rules may need adjustment if they produce false positives
- ADRs can be superseded (mark old ones as "Superseded by ADR-XXXX")

Recommend the user re-run drift-audit periodically (monthly or quarterly) to catch
drift that rules don't cover.
