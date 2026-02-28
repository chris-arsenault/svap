<!-- drift-generated -->
# Backend Logging Pattern Guide

**ADR:** [017-backend-structured-logging](../adr/017-backend-structured-logging.md)

## Quick Rules

1. Every Python file under `backend/src/svap/` MUST use `logging.getLogger(__name__)`
2. Never use `print()` in production code
3. Use `logger.info()` for progress, `logger.warning()` for skips, `logger.error()` for failures
4. Use `%s`-style formatting (not f-strings) in log calls for deferred evaluation

## Pattern

```python
import logging

logger = logging.getLogger(__name__)


def run_stage(config, run_id):
    logger.info("Starting stage for run %s", run_id)

    cases = get_cases(run_id)
    if not cases:
        logger.warning("No cases found for run %s, skipping", run_id)
        return

    for case in cases:
        try:
            result = process_case(case)
            logger.info("Processed case %s: %d items", case["id"], len(result))
        except Exception as e:
            logger.error("Failed to process case %s: %s", case["id"], str(e))
            raise
```

## Before / After

### Before (print-based)
```python
def run(config, run_id):
    print(f"[Stage 3] Starting scoring for run {run_id}")
    policies = get_policies()
    print(f"[Stage 3] Found {len(policies)} policies")

    for policy in policies:
        try:
            score = compute_score(policy)
            print(f"[Stage 3] Scored policy {policy['id']}: {score}")
        except Exception as e:
            print(f"[Stage 3] ERROR scoring policy {policy['id']}: {e}")
            raise
```

### After (logging-based)
```python
import logging

logger = logging.getLogger(__name__)


def run(config, run_id):
    logger.info("Starting scoring for run %s", run_id)
    policies = get_policies()
    logger.info("Found %d policies", len(policies))

    for policy in policies:
        try:
            score = compute_score(policy)
            logger.info("Scored policy %s: %s", policy["id"], score)
        except Exception as e:
            logger.error("Failed to score policy %s: %s", policy["id"], str(e))
            raise
```

## Severity Levels

| Level | When to use | Example |
|-------|-------------|---------|
| `logger.info()` | Normal progress, counts, completions | `logger.info("Processed %d cases", count)` |
| `logger.warning()` | Skipped items, degraded behavior, empty results | `logger.warning("No documents for case %s, skipping", case_id)` |
| `logger.error()` | Failures, exceptions, invalid data | `logger.error("LLM invocation failed: %s", str(e))` |
| `logger.debug()` | Verbose diagnostic info (not for normal runs) | `logger.debug("Raw LLM response: %s", response)` |

## Why `%s` Instead of f-strings

```python
# GOOD: deferred formatting — string is only built if the log level is enabled
logger.info("Processing %d items for case %s", len(items), case_id)

# BAD: eager formatting — f-string is always evaluated, even if logging is disabled
logger.info(f"Processing {len(items)} items for case {case_id}")
```

Using `%s`-style placeholders lets the logging framework skip string formatting
entirely when the log level is above the threshold, which matters in tight loops.

## Enforcement

- **CI script:** `backend/scripts/check-no-print.sh` scans Python files under
  `backend/src/svap/` and fails the build if `print(` is found
- All 11 stage files now use `logging.getLogger(__name__)` instead of `print()`

## What NOT To Do

```python
# BAD: print statements
print("Starting stage 3")
print(f"Error: {e}")

# BAD: root logger
logging.info("This uses the root logger")

# BAD: logger with hardcoded name
logger = logging.getLogger("stage3")

# GOOD: module-level logger with __name__
logger = logging.getLogger(__name__)
logger.info("Starting stage")
```
