<!-- drift-generated -->
# ADR 017: Backend Structured Logging

## Status
Accepted (2026-02-28)

## Context
All 11 pipeline stage files (`stage0_corpus_init.py` through `stage6_foresight.py`)
used `print()` for output. Meanwhile, `api.py` and `stage_runner.py` used Python's
`logging` module. Lambda output via `print()` lacks timestamps, severity levels, and
module context in CloudWatch, making it difficult to filter logs by severity or trace
issues back to specific modules during pipeline debugging.

## Decision
All backend Python files under `backend/src/svap/` must use `logging.getLogger(__name__)`
for output. No `print()` calls in production code.

### Required pattern
```python
import logging

logger = logging.getLogger(__name__)

# Use appropriate severity levels
logger.info("Processing %d cases", len(cases))
logger.warning("Skipping case %s: no enforcement actions", case_id)
logger.error("Failed to invoke LLM: %s", str(e))
```

## Rules
- Every Python file under `backend/src/svap/` MUST use `logging.getLogger(__name__)`
- No `print()` calls in production code (test files and scripts are exempt)
- Use `logger.info()` for progress messages
- Use `logger.warning()` for skipped items or degraded behavior
- Use `logger.error()` for failures and exceptions

## Consequences
- CloudWatch logs now have structured severity (INFO/WARNING/ERROR), module names,
  and consistent formatting
- Logs can be filtered by severity level in CloudWatch Insights
- Module context (`__name__`) identifies which stage or module produced each log line
- Consistent with the existing logging pattern in `api.py` and `stage_runner.py`

## Enforcement
- **CI script:** `backend/scripts/check-no-print.sh` scans all Python files under
  `backend/src/svap/` for `print(` calls and fails the build if any are found
- **Pattern doc:** [backend-logging](../patterns/backend-logging.md)
