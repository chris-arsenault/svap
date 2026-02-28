<!-- drift-generated -->
# CSS Architecture Pattern Guide

**ADR:** [004-css-architecture](../adr/004-css-architecture.md)
**Reference:** `../stack-atlas/frontend/src/` (zero inline styles, co-located CSS)

## Quick Rules

1. **No `style={{}}`** — all styling goes in `.css` files
2. **Co-locate CSS** — `Component.css` next to `Component.jsx`/`.tsx`
3. **Import as side effect** — `import "./Component.css";`
4. **Plain className strings** — no CSS modules, no utility libraries
5. **CSS custom properties** for dynamic theming values

## File Structure

```
components/
├── EntityBrowser.jsx
├── EntityBrowser.css        ← component-local styles
├── ChroniclePanel.jsx
├── ChroniclePanel.css       ← component-local styles
├── IlluminatorSidebar.jsx
└── IlluminatorSidebar.css   ← component-local styles
```

## Component Pattern

```jsx
// EntityCard.jsx
import "./EntityCard.css";

export function EntityCard({ entity, isActive, prominence }) {
  return (
    <div className={`entity-card ${isActive ? "active" : ""}`}>
      <span className="entity-card-name">{entity.name}</span>
      <span className="entity-card-kind">{entity.kind}</span>
      <span className={`entity-card-prominence prominence-${prominence}`}>
        {prominence}
      </span>
    </div>
  );
}
```

```css
/* EntityCard.css */
.entity-card {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 10px;
  border-radius: 4px;
  cursor: pointer;
}

.entity-card.active {
  background: var(--accent-subtle);
}

.entity-card-name {
  font-weight: 500;
  color: var(--text-primary);
}

.entity-card-kind {
  font-size: 11px;
  color: var(--text-muted);
}

.prominence-mythic { color: var(--prominence-mythic); }
.prominence-renowned { color: var(--prominence-renowned); }
.prominence-recognized { color: var(--prominence-recognized); }
```

## Dynamic Values

When a value is genuinely computed at runtime (SVG positions, user-driven colors), use CSS custom properties:

```jsx
// Tooltip positioned by mouse event — CSS handles the rendering
<div
  className="tooltip"
  style={{ '--tooltip-x': `${x}px`, '--tooltip-y': `${y}px` }}
>
```

```css
.tooltip {
  position: absolute;
  left: var(--tooltip-x);
  top: var(--tooltip-y);
}
```

This is the **only** acceptable use of `style={}`. Add an `eslint-disable` comment:

```jsx
{/* eslint-disable-next-line local/no-inline-styles -- dynamic position from mouse event */}
<div className="tooltip" style={{ '--tooltip-x': `${x}px`, '--tooltip-y': `${y}px` }} />
```

## Conditional Classes

Use template literals for simple conditionals:

```jsx
<div className={`panel ${isOpen ? "open" : ""}`}>
<button className={`btn ${variant}`}>
<span className={`status ${status === "active" ? "status-active" : "status-inactive"}`}>
```

For many conditions, build the string:

```jsx
const cls = ["entity-row"];
if (isSelected) cls.push("selected");
if (isHighlighted) cls.push("highlighted");
if (prominence === "mythic") cls.push("mythic");

<div className={cls.join(" ")}>
```

## Global Framework Styles

Global styles live in the app's root `src/` directory:

| File | Purpose |
|------|---------|
| `variables.css` | `:root` custom properties — colors, spacing, typography tokens |
| `styles.css` | Shared patterns — buttons, form fields, panels, badges |
| `animations.css` | `@keyframes` definitions |
| `App.css` | App shell layout (grid, sidebar, top bar) |

Import chain: `main.jsx` imports `variables.css` → `styles.css` → `App.css`. Component CSS files reference the custom properties without importing them.

## Class Naming

Use component-scoped prefixes to avoid collisions (no CSS modules):

```css
/* Good — scoped to component */
.entity-browser { ... }
.entity-browser-header { ... }
.entity-browser-list { ... }
.entity-browser-item { ... }

/* Bad — too generic, will collide */
.header { ... }
.list { ... }
.item { ... }
```

## What NOT To Do

```jsx
// BAD: inline style object
<div style={{ display: "flex", gap: "8px", padding: "12px" }}>

// BAD: style variable
const boxStyle = { border: "1px solid #ccc" };
<div style={boxStyle}>

// BAD: conditional inline style
<div style={{ opacity: isVisible ? 1 : 0 }}>

// GOOD: CSS class
<div className="flex-row padded">

// GOOD: conditional class
<div className={isVisible ? "visible" : "hidden"}>
```
