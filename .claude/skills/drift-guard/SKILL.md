---
name: drift-guard
description: >
  Generate automated guardrails to prevent technical drift from returning to a codebase after
  unification. Prioritizes hard, machine-enforceable protections (ESLint rules, ast-grep rules,
  TypeScript config) over documentation. Every drift area MUST have at least one lint rule or
  automated check before any ADRs or docs are written. Use this skill whenever the user wants to
  "prevent regression", "add linting rules", "enforce patterns", "create ADRs", "document
  architectural decisions", "guard against drift", or "lock in the canonical patterns". Also
  trigger when someone says "make sure this doesn't drift again", "how do I enforce this", or
  "write rules so future code follows the pattern". Works best after drift-unify but can be used
  standalone.
---

# Drift Guard Skill

You are creating automated guardrails to prevent technical drift from returning after unification.

**Priority order is non-negotiable:**
1. **Hard enforcement first** — ESLint rules, ast-grep rules, TypeScript config changes
2. **Documentation second** — ADRs, pattern guides, checklists

Every drift area MUST have at least one machine-enforceable rule before any documentation is
written. An ADR without a corresponding lint rule is a wish, not a guardrail. If you cannot
find an enforceable constraint for an area, explain specifically why to the user — don't
silently skip to documentation.

**Everything you generate must be derived from the project's actual canonical patterns.** Read the
codebase, read the unification log, understand what was decided and why, then create enforcement
that matches.

## CRITICAL: Drift Marker Requirement

**Every file you create or modify MUST include the correct drift marker on the FIRST LINE.**
Files without this marker are invisible to `drift library push` — they won't sync, won't
appear in the dashboard, and the user won't know they exist.

| File type | Required first line |
|-----------|-------------------|
| `.js`, `.cjs`, `.mjs`, `.ts` | `// drift-generated` |
| `.yml`, `.yaml` | `# drift-generated` |
| `.md` | `<!-- drift-generated -->` |

**This applies to ALL generated files:** ESLint rules, ast-grep rules, ADRs, pattern docs,
checklists, review templates — everything. No exceptions.

If you are editing an existing file that already has content on line 1, prepend the marker
as a new first line.

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

# What ast-grep rules exist?
ls "$PROJECT_ROOT"/.ast-grep/ 2>/dev/null || ls "$PROJECT_ROOT"/sgconfig.yml 2>/dev/null || echo "No ast-grep config"

# What docs already exist?
ls "$PROJECT_ROOT"/docs/ 2>/dev/null
```

Ask the user which unified patterns to guard. If a unification log exists, use it as context.

## Phase 1: Hard Enforcement (Mandatory)

For each drift area, you MUST produce at least one machine-enforceable protection. Work through
these mechanisms in order — use the first one that fits, and use multiple when possible.

### 1. ESLint Rules

**First line of every `.js`/`.ts` rule file:** `// drift-generated`

Read `$DRIFT_SEMANTIC/skill/drift-guard/references/eslint-rule-patterns.md` for the mechanical
details of writing rules. The WHAT to enforce comes from the codebase, not from that reference.

**Process for each drift area:**

1. **Read the canonical pattern** — the actual files, not a description of them.
2. **Read 1-2 old variant files** (from git history or unification log) to understand what
   should be banned.
3. **Identify EVERY enforceable boundary.** Look for ALL of the following — most drift areas
   have more than one enforceable constraint:
   - **Import restrictions** — ban imports of deprecated modules/components/paths
   - **API usage restrictions** — ban direct use of low-level APIs that should go
     through an abstraction
   - **Component usage restrictions** — ban old component names or require specific
     wrapper components
   - **Naming conventions** — enforce naming patterns for similar concepts
   - **Structural patterns** — ban specific AST shapes that represent old approaches
   - **Prop/argument patterns** — require or ban specific props, flags, or arguments
   - **File organization** — restrict which directories certain patterns can appear in
4. **Write rules that are precise.** A rule with false positives will be disabled. If you
   can't express a constraint precisely, document it as a review guideline instead — but
   only AFTER you've exhausted all lintable constraints.
5. **Use `warn` severity initially.** Upgrade to `error` after all violations are fixed.
6. **Include helpful messages.** Every violation message should:
   - Explain what's wrong in plain language
   - Say what to do instead (the specific canonical alternative)
   - Link to the pattern documentation (if it exists)

**Choose the right enforcement mechanism:**
- `no-restricted-imports` — simplest, for banning specific import paths/names
- `no-restricted-syntax` with AST selectors — for banning specific code shapes
- Custom rule module — for anything more complex (file-path-aware rules, counting rules, etc.)

For the project's ESLint config format, see
`$DRIFT_SEMANTIC/skill/drift-guard/references/eslint-rule-patterns.md`.

**Common enforceable patterns by drift type:**

| Drift Type | Likely Rules |
|------------|-------------|
| Multiple implementations of same concept | `no-restricted-imports` banning non-canonical modules |
| Direct use of low-level API | `no-restricted-syntax` banning raw API calls |
| Inconsistent component usage | `no-restricted-syntax` banning old JSX element names |
| Mixed async patterns | `no-restricted-syntax` banning specific call patterns |
| Multiple state management approaches | `no-restricted-imports` banning non-canonical state libs |
| CSS duplication | `no-restricted-imports` banning old CSS files; move shared styles to canonical location |
| Inconsistent error handling | Custom rule requiring catch blocks in specific contexts |
| Mixed fetch patterns | `no-restricted-imports` for raw `fetch`; `no-restricted-syntax` for direct `axios` calls |

### 2. ast-grep Rules (if project uses ast-grep)

**First line of every `.yml`/`.yaml` rule file:** `# drift-generated`

If the project has `sg` (ast-grep) configured, generate YAML pattern rules for structural
enforcement. ast-grep catches patterns that ESLint selectors struggle with:

- Multi-statement patterns (ESLint sees single nodes; ast-grep matches sequences)
- Language-aware structural matching (JSX nesting, type annotations)
- Cross-function patterns

```yaml
# drift-generated
id: ban-direct-db-query
language: TypeScript
rule:
  pattern: db.$METHOD($$$)
  not:
    inside:
      kind: method_definition
      has:
        field: name
        regex: ^(query|execute)$
message: "Use the data access layer instead of direct db calls. See docs/patterns/data-access.md"
severity: warning
```

Save to the project's ast-grep rules directory (typically `.ast-grep/rules/` or
`sgconfig.yml`-referenced location).

### 3. TypeScript Config Tightening

Some drift is preventable through stricter TypeScript configuration. If the canonical
pattern relies on type safety that lax config would allow to erode, recommend (and
implement with user approval) config changes:

- `"strict": true` — prevents gradual loosening of type checks
- `"noImplicitAny": true` — forces explicit typing where the canonical pattern uses types
- `"paths"` aliases — enforce canonical import paths (e.g., `@shared/*` instead of
  relative paths to shared modules)
- `"baseUrl"` + `"paths"` — redirect old module paths to canonical locations

Only recommend changes that directly prevent the specific drift. Don't use this as an
excuse for general TypeScript hardening.

### 4. Wire Rules into Config and Verify

After generating ALL rules for ALL areas (not one at a time):

1. **Update the ESLint config** to wire in all generated rules. Read the existing config,
   add imports for new rule modules, add rule entries with `warn` severity.
2. **Update ast-grep config** if rules were generated.
3. **Run and verify:**
   ```bash
   # ESLint — confirm rules are active
   npx eslint src/ --format compact 2>/dev/null | head -30
   npx eslint src/ --format compact 2>/dev/null | grep -c "Warning\|Error" || echo "0 violations"

   # ast-grep — confirm rules are active (if applicable)
   sg scan --rule .ast-grep/rules/ 2>/dev/null | head -20
   ```
4. **Report violation counts per rule.** This is the guard's scoreboard — it tells the
   user exactly how much cleanup remains.

**If zero violations are reported for a rule, investigate.** Either:
- The rule is correct and the unification was complete (good)
- The rule isn't matching what it should (bad — fix the selector/pattern)
- The rule isn't loaded (bad — check config wiring)

### Enforcement Quality Gate

Before proceeding to Phase 2, verify:

```
Guard Enforcement Summary:
  Drift areas:          N
  Areas with rules:     N  (MUST equal total areas)
  Total ESLint rules:   N
  Total ast-grep rules: N
  Total violations:     N  (expected: 0 if unify was complete)
  Rules verified:       yes/no
```

**If any area has zero enforceable rules,** explain to the user what about this area
cannot be machine-enforced and what the fallback is (review checklist item, manual
re-audit). Do not silently proceed — the user needs to know which areas depend on
human discipline vs machine enforcement.

---

## Phase 2: Documentation (After enforcement is complete)

Documentation artifacts exist to explain WHY the rules exist and HOW to follow the
canonical pattern. They are secondary to — and should reference — the hard enforcement
from Phase 1.

### 5. Architecture Decision Records

**First line of every ADR:** `<!-- drift-generated -->`

Create an ADR for each significant unification decision. See
`$DRIFT_SEMANTIC/skill/drift-guard/references/adr-template.md` for the template.

**An ADR answers "why did we decide this?"** It captures:
- What the problem was (the drift)
- What alternatives existed (the variants)
- What was chosen and why
- What the tradeoffs are

**The Enforcement section of every ADR must reference the actual rules created in Phase 1.**
Don't write "ESLint rules enforce this" — write the specific rule names:
```markdown
## Enforcement
- ESLint rule `drift-guard/no-direct-fetch` bans raw fetch() calls outside the API layer
- ESLint `no-restricted-imports` bans importing from `src/old-api/` paths
- ast-grep rule `ban-inline-styles` catches style objects outside the theme system
```

**Important: derive ADRs from the actual decisions made.** Read the unification log and
the user's stated reasoning. Don't invent reasons.

Save ADRs to `docs/adr/` (or wherever the project keeps documentation). Number sequentially.

### 6. Pattern Usage Guides

**First line of every pattern doc:** `<!-- drift-generated -->`

For each unified area, ensure a usage guide exists that shows developers how to follow the
canonical pattern. These should be practical, copy-paste-ready documents.

If drift-unify already created pattern docs, review and enhance them. If not, create them
by reading the canonical implementation and writing a usage guide.

### 7. Review Checklist

**First line of every checklist file:** `<!-- drift-generated -->`

Generate or update a PR review checklist that includes drift-prevention items. Match the
format the project already uses (GitHub PR template, CONTRIBUTING.md, or standalone doc).

Each checklist item should:
- Be specific to a real drift area (not generic "follow best practices")
- Reference the relevant lint rule — e.g., "If you see `drift-guard/no-direct-fetch`
  warnings, use the API client from `src/api/client.ts` instead"
- Be verifiable by a reviewer in under 30 seconds

**Review checklist items should cover what lint rules CANNOT catch:**
- Semantic correctness (does the code do the right thing, not just use the right API?)
- Architectural intent (is this the right place for this code?)
- Edge cases the canonical pattern handles that ad-hoc code might miss

### 8. Drift Guard Configuration File (optional)

If the user plans to re-run drift-audit periodically, create or update
`.drift-audit/config.json` to record canonical patterns:

```json
{
  "canonical_patterns": [
    {
      "area": "the drift area id from manifest",
      "canonical_variant": "the variant name that won",
      "enforced_by": ["drift-guard/no-direct-fetch", "no-restricted-imports"],
      "adr": "docs/adr/0001-whatever.md",
      "pattern_doc": "docs/patterns/whatever.md"
    }
  ]
}
```

---

## After Generating All Artifacts

### Verify All Guard Artifacts

Run the full verification suite to confirm everything is wired correctly:

```bash
drift verify "$PROJECT_ROOT"
```

This runs three checks and reports a scoreboard:

1. **Markers** — every file in sync directories has its drift marker on line 1.
   Reports which files are missing and what marker to add.
2. **ESLint** — every rule file in `eslint-rules/` is imported AND enabled in the
   ESLint config. Reports INTEGRATED / NOT INTEGRATED per rule with severity.
3. **ADR** — every ADR's `## Enforcement` section references rules and docs that
   actually exist. Reports OK / DEGRADED / BROKEN per ADR.

Use `--check markers`, `--check eslint`, or `--check adr` to run individual checks.
Use `--json` for machine-readable output.

**If any check fails, fix the issues:**

- **Missing markers** → add the appropriate marker as line 1 (the report says which).
- **Unintegrated ESLint rules** → wire them into the ESLint config. Provide the exact
  config additions needed (see
  `$DRIFT_SEMANTIC/skill/drift-guard/references/eslint-rule-patterns.md`). Ask the user
  if they want you to update the config.
- **Broken/degraded ADR enforcement** → create any missing referenced files before
  finalizing the ADR. An ADR's enforcement section must never reference artifacts
  that don't exist.

Re-run `drift verify "$PROJECT_ROOT"` after fixes until all checks pass.

### Library Push

If the project has a drift library configured (`.drift-audit/config.json`),
run `drift library push` to share the generated ESLint rules, ADRs, and pattern docs
with other projects. In online mode, the orchestrator handles this automatically
after the guard phase completes.

---

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
