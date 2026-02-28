

<!-- drift-semantic -->
## Drift

`drift` is installed at `~/.drift-semantic` (`$DRIFT_SEMANTIC`).

Run the full drift pipeline:
```bash
drift run --project .    # semantic analysis pipeline
```

Drift skills (installed in `.claude/skills/`):
- `drift` — orchestrator: audit → plan → unify → guard
- `drift-audit` — structural drift discovery
- `drift-audit-ux` — behavioral drift discovery
- `drift-audit-semantic` — semantic duplication detection (tool-assisted)
- `drift-unify` — batch refactoring toward canonical patterns
- `drift-guard` — generate ESLint rules, ADRs, pattern docs

Centralized artifact library (online mode — auto-sync enabled):
```bash
drift library init       # initialize ~/.drift/library
drift library list       # show library contents
drift library status     # compare library vs project
```

**Auto-sync behavior:** Library sync is automatic. Before starting any drift
audit phase, run `drift library sync` to pull the latest artifacts from the
centralized library. After completing any drift-guard phase, run
`drift library publish` to share new artifacts back to the library.
Do not ask for confirmation — sync silently as part of the pipeline.
<!-- /drift-semantic -->
