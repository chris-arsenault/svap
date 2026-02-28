<!-- drift-generated -->
# ADR-009: CSS Architecture -- No Inline Styles

## Status

Accepted (2026-02-27)

## Context

Styling across the monorepo used a mix of approaches: component-local `.css`
files with class prefixes, inline `style={{ ... }}` attributes for one-off
tweaks, and `const styles = { ... }` JavaScript objects passed to `style`
props. This inconsistency made theming difficult -- CSS custom property
overrides in `:root` could not reach inline styles, and JavaScript style
objects bypassed the cascade entirely.

The codebase also had theming drift: each app defined its own color values
rather than overriding a shared set of `--color-*` CSS custom properties.

## Decision

### All styling uses CSS classes in component-local .css files

Static styles go in companion `.css` files alongside their component. Each
component uses a scoped class prefix (e.g. `chronicle-panel-`, `rfm-`) to
avoid collisions.

### Inline style attributes are banned except for CSS custom properties

The ESLint rule `local/no-inline-styles` enforces this at warn level. The
only allowed inline styles are CSS custom property assignments:

```jsx
// Allowed -- passing dynamic values into CSS
<div style={{ '--progress-pct': `${percent}%` }}>

// Banned -- static styling belongs in .css files
<div style={{ marginTop: '8px', color: 'red' }}>

// Banned -- JS style objects bypass the cascade
const styles = { container: { padding: 16 } };
```

### Theming via CSS custom property overrides

Each app overrides canonical `--color-*` variables in its `:root` selector.
Component `.css` files reference these variables rather than hard-coding color
values. This allows per-app theming through a single `:root` block.

### No JavaScript style objects

`const styles = { ... }` objects are banned. All styles go through CSS classes
so they participate in the cascade and can be overridden by theming variables.

## Consequences

- New components must have a companion `.css` file for their styles
- Dynamic values that need to reach CSS (progress percentages, computed
  positions, etc.) are passed via CSS custom properties on `style`
- The `local/no-inline-styles` ESLint rule catches violations during
  development; it is set to warn level to allow incremental migration of
  remaining inline styles
- Per-app theming is controlled entirely through `:root` CSS custom property
  overrides, making theme changes predictable and auditable
