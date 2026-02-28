<!-- drift-generated -->
# ADR 018: ESLint Rule Canonical Location

## Status
Accepted (2026-02-28)

## Context
ESLint rules were duplicated between `eslint-rules/` (project root) and
`frontend/eslint-rules/`. The root-level rules were drift-generated artifacts that
diverged from the actively-used frontend rules. Specifically, the `no-inline-styles`
rule in `frontend/` was a degraded version missing the CSS custom property allowance
(`style={{ "--var": value }}`), while the root version had the correct implementation
but was not wired into the build.

This duplication created confusion about which rules were authoritative and led to
silent divergence where fixes applied to one location did not propagate to the other.

## Decision
`frontend/eslint-rules/` is the single canonical location for all ESLint rules.
The drift config sync path points here. No ESLint rules exist at the project root level.

### Canonical rules
| Rule | File | Purpose |
|------|------|---------|
| `no-inline-styles` | `frontend/eslint-rules/no-inline-styles.js` | Prevents inline `style={}` props, allows CSS custom property patterns |
| `no-escape-hatches` | `frontend/eslint-rules/no-escape-hatches.js` | Prevents `dangerouslySetInnerHTML`, `!important`, and `eslint-disable` |
| `no-direct-fetch` | `frontend/eslint-rules/no-direct-fetch.js` | Enforces use of shared `apiGet`/`apiPost` utilities |
| `no-manual-async-state` | `frontend/eslint-rules/no-manual-async-state.js` | Enforces `useAsyncAction()` over manual `useState` for async state |

## Rules
- All ESLint rules live in `frontend/eslint-rules/`
- No ESLint rules at the project root `eslint-rules/` directory
- Every rule must be imported and wired in `frontend/eslint.config.js`
- The drift config sync path (`.drift-audit/config.json`) points to `frontend/eslint-rules/`

## Consequences
- Rule updates happen in one place with no risk of divergence
- The upgraded `no-inline-styles` correctly allows CSS custom property patterns
  like `style={{ "--progress": `${pct}%` }}`
- The `no-escape-hatches` rule is now wired into the frontend ESLint config
- New drift-generated rules are placed directly in `frontend/eslint-rules/`

## Enforcement
- **Drift config:** `.drift-audit/config.json` sync path set to `frontend/eslint-rules/`
- **ESLint config:** All rules imported in `frontend/eslint.config.js`
- Root-level `eslint-rules/` directory should not contain rule files
