<!-- drift-generated -->
# ADR-004: Component-Local Pure CSS Architecture

**Status:** Accepted
**Date:** 2026-02-25
**Deciders:** tsonu
**Drift area:** CSS styling (new — not in original drift audit)

## Context

An audit of CSS patterns across illuminator, viewer, and chronicler revealed three divergent approaches:

| App | Pattern | Inline `style={}` | `className=` | CSS files |
|-----|---------|-------------------|--------------|-----------|
| Illuminator | Inline-dominant | 3,263 (95 files) | 1,077 (72 files) | 1 (`App.css`) |
| Viewer | Class-dominant | 1 (1 file) | 35 (3 files) | 1 (`styles.css`) |
| Chronicler | CSS modules | 67 (7 files) | 395 (10 files) | 10 (`.module.css`) |

Illuminator is almost entirely JavaScript style objects — `style={{ display: 'flex', gap: '8px' }}` — with a single CSS file. Chronicler uses CSS modules properly but still has 67 inline style occurrences. Viewer is the cleanest, using almost no inline styles.

The reference implementation (stack-atlas) uses **zero inline styles** across its entire frontend. All styling is through component-local `.css` files imported as side effects (`import "./Component.css"`) with plain `className` strings.

Inline styles cause:
- No hover/focus/media-query support without JS workarounds
- No CSS variable theming propagation
- Larger JSX that mixes layout concerns with behavior
- No caching — style objects recreated every render unless memoized
- Harder to audit visual consistency across the app

## Decision

All MFE apps converge on **component-local pure CSS** with **global framework styles**:

### Component-local CSS
- Each component that needs styling gets a co-located `.css` file: `Component.css` next to `Component.jsx`/`.tsx`
- Imported as a side effect: `import "./Component.css";`
- **Not** CSS modules (no `.module.css`) — plain CSS with scoped class naming conventions
- Classes applied via plain `className` strings or template literals for conditionals

### Global framework styles
- CSS custom properties (`:root` variables) for theming tokens — colors, spacing scale, typography
- Shared patterns (buttons, form fields, panels) in a global stylesheet
- Animations (`@keyframes`) in global stylesheets

### No JavaScript styles
- No `style={{}}` object literals on JSX elements
- No `style={variable}` references
- No CSS-in-JS libraries (styled-components, emotion, etc.)
- For genuinely dynamic values (computed positions, runtime colors), use CSS custom properties set via a single `style={{ '--x': value }}` with an `eslint-disable` comment explaining why

### Class naming
- Plain strings: `className="entity-card"`
- Template literals for conditionals: `` className={`entity-card ${isActive ? "active" : ""}`} ``
- No `clsx`/`classnames` library needed at current codebase size

### Reference implementation
- `../stack-atlas/frontend/src/` — zero inline styles, co-located `ComponentName.css` files, CSS custom properties in `:root`

## Consequences

### Positive
- Consistent styling approach across all MFE apps
- Full access to CSS features: media queries, pseudo-classes, animations, variables
- Smaller JSX — layout separated from behavior
- Better performance — browser CSS caching, no runtime style object creation
- Easier visual auditing — all styles in `.css` files, searchable/greppable

### Negative
- Large migration effort for illuminator (3,263 inline style occurrences across 95 files)
- Moderate effort for chronicler (67 inline styles + converting `.module.css` to plain `.css`)
- Developers must create/maintain a separate `.css` file per component
- Class name collisions possible without CSS modules (mitigated by scoped naming)

### Migration
- **Not started.** This ADR establishes the target pattern and lint guardrails only.
- Migration will happen incrementally: new code follows the pattern, existing code converted file-by-file.
- ESLint rule `local/no-inline-styles` set to `warn` to surface violations without blocking.

## Enforcement

- **ESLint rule:** `local/no-inline-styles` — warns on any `style=` JSX attribute. See `eslint-rules/no-inline-styles.js`.
- **Review checklist:** New components must have a co-located `.css` file if they need styling. No `style={{}}` in new code.
- **Pattern documentation:** `docs/patterns/css-architecture.md`

## Alternatives Considered

| Alternative | Why not chosen |
|-------------|---------------|
| CSS Modules (`.module.css`) | Chronicler uses this. Adds import ceremony (`styles.foo`), requires build tooling awareness. Plain CSS with scoped naming is simpler and matches the stack-atlas reference. |
| CSS-in-JS (styled-components, emotion) | Runtime overhead, bundle size, locks into a library. Pure CSS has zero runtime cost. |
| Tailwind CSS | Utility-first approach clutters JSX with long class strings. Doesn't match the existing codebase aesthetic. |
| Keep current mixed approach | Three different CSS strategies across apps increases cognitive load and makes cross-app consistency impossible. |
