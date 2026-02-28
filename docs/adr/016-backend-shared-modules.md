# ADR 016: Backend Shared Modules (defaults.py, parallel.py)

## Status
Accepted (2026-02-28)

## Context
1. Default config was duplicated in `api.py` (module-level dict) and
   `stage_runner.py` (factory function) — identical values, two sources of truth.
2. Stages 5 and 6 had nearly identical `_run_parallel_*` functions for executing
   LLM calls concurrently.
3. Path parameters were accessed via raw dict indexing without error guards.

## Decisions

### defaults.py
Single source of truth for pipeline configuration defaults.
```python
from svap.defaults import default_config
```
Both `api.py` and `stage_runner.py` import from this shared module.
Factory function pattern returns a fresh dict per call.

### parallel.py
Generic parallel LLM execution utility.
```python
from svap.parallel import run_parallel_llm

total, failed = run_parallel_llm(
    invoke_fn=lambda prompt: _invoke_llm(client, prompt),
    jobs=[(label, prompt, context_dict), ...],
    on_result=lambda result, ctx: store_and_return_count(result, ctx),
    max_concurrency=5,
)
```
Stages provide their own `invoke_fn` (since LLM params differ) and
`on_result` callback (since storage logic differs).

### _path_param() in api.py
```python
val = _path_param(event, "case_id")  # raises ApiError(400) if missing
```
All path-parameterized routes use this instead of raw dict access.

## Rules
- Never define default config values in Lambda entry points
- Never copy-paste parallel execution logic — parameterize `run_parallel_llm()`
- Never access `event["pathParameters"]` directly — use `_path_param()`
