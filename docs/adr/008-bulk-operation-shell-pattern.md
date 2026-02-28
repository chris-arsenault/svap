<!-- drift-generated -->
# ADR-008: Bulk Operation Shell Pattern

## Status

Accepted (2026-02-27)

## Context

The illuminator app has several bulk operation modals (BulkToneRankingModal,
BulkFactCoverageModal, BulkBackportModal, BulkChronicleAnnotationModal,
BulkHistorianModal) that share an identical lifecycle pattern: overlay/dialog
presentation, minimize-to-pill via useFloatingPillStore, a phase state machine
(confirming, running, terminal), progress bars, terminal messages, failed-item
lists, cost summaries, and footer buttons that change with each phase.

Before unification, each bulk modal hand-rolled its own dialog, progress, and
error UI, leading to inconsistent UX and duplicated boilerplate across all five
modals.

## Decision

### BulkOperationShell is the canonical bulk modal container

All bulk operation modals must use `BulkOperationShell`. It is built on top of
ModalShell (ADR-007) and adds the bulk-specific lifecycle on top.

### BulkOperationShell handles these concerns

- **Overlay/dialog** presentation (delegated to ModalShell)
- **Pill lifecycle** via `useFloatingPillStore` (minimize, restore, dismiss)
- **Phase state machine**: `confirming` (user reviews scope), `running`
  (operation in progress), `terminal` (success or failure summary)
- **Footer buttons** that change per phase (confirm/cancel, minimize, close)

### Companion components for bulk modal content

BulkOperationShell exports companion components that per-modal code composes
inside the shell:

- `BulkProgressBar` -- progress indicator during the running phase
- `BulkTerminalMessage` -- success/failure summary in the terminal phase
- `BulkFailedList` -- list of items that failed during the operation
- `BulkCost` -- cost/token summary for operations that consume resources

### Per-modal code is content-only

Each bulk modal (BulkToneRankingModal, BulkFactCoverageModal, etc.) focuses
exclusively on its content-specific rendering: scope selection UI in the
confirming phase, per-item progress details in the running phase, and
domain-specific result summaries in the terminal phase.

## Consequences

- New bulk operations must use BulkOperationShell, not hand-roll their own
  modal, progress, or error UI
- Bulk modals are significantly shorter since dialog, pill, phase, and footer
  logic is handled by the shell
- Changes to bulk operation UX (e.g. adding a pause button, changing pill
  behavior) are made once in BulkOperationShell and apply to all bulk modals
- Non-bulk modals continue to use ModalShell directly (ADR-007)
