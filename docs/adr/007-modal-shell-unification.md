<!-- drift-generated -->
# ADR-007: Modal Shell Unification

## Status

Accepted (2026-02-26)

## Context

The codebase had 30+ modal components across 5 apps, each hand-rolling their
own overlay, header, close button, and dismiss behavior. Patterns varied:

- **Overlay close**: Some modals closed on overlay click (with mouseDown guard
  to prevent text-selection dismissal), others did not
- **Escape key**: Only 1 of 30+ modals handled Escape to close
- **Scroll lock**: Most modals did not lock body scroll
- **CSS classes**: Each modal used its own prefix (`rfm-`, `hcpm-`, `tapm-`,
  `static-page-modal-`, etc.) for identical structural elements

A shared `ModalShell` component existed in `@penguin-tales/shared-components`
but was only used by 5 modals in name-forge and coherence-engine.

## Decision

### ModalShell is the canonical modal container

All new modals must use `ModalShell`. Existing modals should be migrated
incrementally.

### ModalShell provides these behaviors by default

- **Overlay** with click-to-close (using mouseDown guard to prevent
  text-selection dismissal)
- **Escape key** to close
- **Body scroll lock** while open
- **Header** with optional icon, title, disabled badge, and close button
- **Footer** slot for action buttons
- **Tabbed mode** with sidebar when `tabs` prop is provided
- **`preventOverlayClose`** prop to disable overlay click and Escape
  (for modals with unsaved state or destructive actions)

### Sizing via className

ModalShell uses `.modal` as its base class (from shared-components modal.css).
Per-modal sizing is controlled via `className` prop:

- `modal-sm` for small dialogs (max-width 500px)
- `modal-lg` for large dialogs (max-width 1100px)
- Custom classes like `rfm-dialog` for specific widths

Custom classes should use `.modal.<custom-class>` specificity to override
the base `.modal` sizing (especially `height: auto` to replace the default
`height: 80vh` for content-sized dialogs).

### Migration pattern

When migrating a hand-rolled modal:

1. Replace overlay div with `<ModalShell>`
2. Remove mouseDown/click overlay handlers (ModalShell handles it)
3. Remove Escape key handlers (ModalShell handles it)
4. Move footer buttons to `footer` prop
5. Keep content-specific CSS classes (counts grids, charts, entity lists)
6. Remove overlay/header/footer CSS (provided by ModalShell + modal.css)
7. Add `.modal.<custom>` sizing override if needed

## Consequences

- All modals get consistent dismiss behavior (Escape, overlay click, scroll
  lock) without per-modal implementation
- New modals are ~30-50 lines shorter (no boilerplate overlay/header/close)
- Bulk workflow modals (with minimize-to-pill behavior) are a separate
  migration — they need a BulkOperationModal wrapper built on ModalShell
- Complex multi-phase wizard modals may need additional ModalShell features
  (step indicators, dynamic titles) in future iterations
