"""Shared parallel LLM execution utility."""

import logging
from concurrent.futures import ThreadPoolExecutor, as_completed

logger = logging.getLogger(__name__)


def run_parallel_llm(invoke_fn, jobs, on_result, max_concurrency):
    """Execute LLM calls in parallel, streaming results to a callback.

    Args:
        invoke_fn: callable(prompt) -> parsed JSON result.
        jobs: iterable of ``(label, prompt, context)`` tuples.  *label* is
            used for log messages; *context* is passed through to *on_result*.
        on_result: callable(result, context) -> int.  Called with the parsed
            LLM result and the opaque job context; should persist the result
            and return the number of items stored.
        max_concurrency: maximum concurrent threads.

    Returns:
        ``(total_count, failed_labels)`` — *total_count* is the sum of all
        *on_result* return values; *failed_labels* lists labels for jobs
        whose LLM call raised an exception.
    """
    jobs = list(jobs)
    logger.info("Submitting %d parallel Bedrock calls (concurrency=%d)", len(jobs), max_concurrency)

    total = 0
    failed = []
    with ThreadPoolExecutor(max_workers=max_concurrency) as executor:
        futures = {
            executor.submit(invoke_fn, prompt): (label, ctx)
            for label, prompt, ctx in jobs
        }
        for future in as_completed(futures):
            label, ctx = futures[future]
            try:
                result = future.result()
                count = on_result(result, ctx)
                total += count
                logger.info("%s: %d items (total: %d)", label, count, total)
            except Exception as e:
                logger.error("FAILED %s: %s", label, e)
                failed.append(label)

    if failed:
        logger.warning("%d items failed", len(failed))

    return total, failed
