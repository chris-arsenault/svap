# Architecture Decision Record Template

Save ADRs to `docs/adr/` (or wherever the project keeps documentation).

**Naming:** `NNNN-short-title.md` — number sequentially, never reuse numbers.

## Template

```markdown
<!-- drift-generated -->
# ADR-NNNN: [Title]

**Status:** Proposed | Accepted | Deprecated | Superseded by ADR-XXXX
**Date:** YYYY-MM-DD
**Deciders:** [who was involved]

## Context

What is the problem? What drift existed? Why did it matter?

Be specific — reference actual file counts, variant names from the audit,
and concrete consequences of the inconsistency.

## Decision

What was chosen and how does it work? Reference the actual canonical files
by path so future readers can find the implementation.

## Consequences

### Positive
What becomes easier, more consistent, or more maintainable?

### Negative
What's harder, what's lost, what are the tradeoffs?

### Migration
What had to change? How many files? Is migration complete or ongoing?

## Enforcement

How is this decision enforced going forward?
- ESLint rules (name them)
- Review checklist items
- Pattern documentation (link it)

## Alternatives Considered

| Alternative | Why not chosen |
|-------------|---------------|
| [Variant name from audit] | [Specific reason based on project context] |
```

## Writing Guidance

- **Derive from reality.** ADRs document decisions that were actually made, based on
  the unification log and user's stated reasoning. Don't invent rationale.
- **Be specific.** "Better DX" is vague. "Reduces modal implementation from ~40 lines
  to ~12 lines with consistent animation and focus handling" is specific.
- **Include file paths.** Future readers need to find the canonical implementation.
- **Keep them short.** 1-2 pages. If longer, the decision might need splitting.
- **Date them.** Decisions have context. What was right in 2025 may need revisiting later.
- **Supersede, don't delete.** If a decision changes, mark the old ADR as superseded
  and write a new one explaining the change.
