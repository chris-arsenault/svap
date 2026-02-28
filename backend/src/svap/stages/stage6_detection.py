"""
Stage 6: Detection Pattern Generation

Translates exploitation predictions into operationally actionable detection
patterns: what to look for in data, which data sources to query, what
constitutes anomalous vs. normal behavior, and how quickly detection is possible.

This is the most implementation-specific stage. The generic patterns generated
here should be refined by data engineers with access to actual system schemas.

Input:  Approved predictions (Stage 5) + optional data source catalog
Output: Detection patterns in the `detection_patterns` table
"""

import hashlib
from concurrent.futures import ThreadPoolExecutor, as_completed

from svap import delta
from svap.bedrock_client import BedrockClient
from svap.rag import ContextAssembler
from svap.storage import SVAPStorage

SYSTEM_PROMPT = """You are a fraud detection analyst designing monitoring rules. You translate
predicted exploitation steps into specific, queryable anomaly signals. Every pattern
must specify: what data source to query, what specific measurable condition to test, what
the normal baseline looks like, what false positives to expect, and how quickly the signal
becomes visible after the exploitation step begins.

Be concrete. "Monitor for unusual billing patterns" is useless. "Flag providers billing
>16 hours/day of personal care services, where normal P95 is 10 hours/day" is actionable.

Focus narrowly on the SPECIFIC STEP you are given, not the broader exploitation scheme."""


def _build_prompt(client, step, tree, tree_summary, data_sources_context):
    """Build the LLM prompt for a single exploitation step."""
    qualities_str = ", ".join(step.get("enabling_qualities", []))
    return client.render_prompt(
        "stage6_detect.txt",
        policy_name=step.get("policy_name", tree.get("policy_name", "")),
        step_title=step["title"],
        step_description=step["description"],
        step_actor_action=step.get("actor_action", ""),
        step_qualities=qualities_str,
        tree_summary=tree_summary,
        data_sources=data_sources_context,
    )


def _invoke_llm(client, prompt):
    """Make the Bedrock call. Thread-safe."""
    return client.invoke_json(prompt, system=SYSTEM_PROMPT, max_tokens=8192)


def _store_patterns(storage, run_id, step, result):
    """Parse LLM result and write detection patterns to DB. Returns count."""
    patterns = result if isinstance(result, list) else result.get("patterns", [result])
    count = 0
    for i, pat_data in enumerate(patterns):
        pat_id = hashlib.sha256(
            f"{step['step_id']}:pat:{i}".encode()
        ).hexdigest()[:12]

        pattern = {
            "pattern_id": pat_id,
            "step_id": step["step_id"],
            "data_source": pat_data.get("data_source", ""),
            "anomaly_signal": pat_data.get("anomaly_signal", ""),
            "baseline": pat_data.get("baseline", ""),
            "false_positive_risk": pat_data.get("false_positive_risk", ""),
            "detection_latency": pat_data.get("detection_latency", ""),
            "priority": pat_data.get("priority", "medium"),
            "implementation_notes": pat_data.get("implementation_notes", ""),
        }
        storage.insert_detection_pattern(run_id, pattern)
        count += 1
    return count


def _build_tree_summary(tree, steps):
    """Build a concise text summary of the exploitation tree for context."""
    lines = [f"Exploitation tree for {tree.get('policy_name', 'Unknown')}:"]
    lines.append(f"  Actor: {tree.get('actor_profile', 'Unknown')}")
    lines.append(f"  Lifecycle: {tree.get('lifecycle_stage', 'Unknown')}")
    lines.append(f"  Steps ({len(steps)} total):")
    for s in steps:
        indent = "    "
        branch = " [BRANCH POINT]" if s.get("is_branch_point") else ""
        label = f" ({s['branch_label']})" if s.get("branch_label") else ""
        lines.append(f"{indent}{s['step_order']}. {s['title']}{branch}{label}")
    return "\n".join(lines)


def _print_pattern_summary(all_patterns):
    """Print detection pattern summary grouped by priority."""
    print("\n  Detection Pattern Summary:")
    by_priority = {"critical": [], "high": [], "medium": [], "low": []}
    for p in all_patterns:
        by_priority.get(p.get("priority", "medium"), by_priority["medium"]).append(p)

    for priority in ["critical", "high", "medium", "low"]:
        if by_priority[priority]:
            print(f"\n    [{priority.upper()}]")
            for p in by_priority[priority]:
                print(f"      - {p['policy_name']}: {p['anomaly_signal'][:100]}")
                print(f"        Data source: {p['data_source']}")


def _get_data_sources_context(storage, config):
    """Retrieve data source context from RAG or fall back to defaults."""
    ctx = ContextAssembler(storage, config)
    context = ctx.retrieve("data sources claims enrollment provider", doc_type="other")
    return context or _default_data_sources()


def _load_steps_from_trees(storage, trees):
    """Load all exploitation steps from approved trees with context."""
    all_steps = []
    for tree in trees:
        steps = storage.get_exploitation_steps(tree["tree_id"])
        summary = _build_tree_summary(tree, steps)
        for step in steps:
            step["policy_name"] = tree["policy_name"]
            all_steps.append((step, tree, summary))
    return all_steps


def _detect_changed_steps(storage, all_steps):
    """Return steps whose content hash changed since last run."""
    stored = storage.get_processing_hashes(6)
    changed = []
    for step, tree, summary in all_steps:
        quals_str = ":".join(sorted(step.get("enabling_qualities", [])))
        h = delta.compute_hash(step["description"], quals_str)
        if stored.get(step["step_id"]) != h:
            changed.append((step, tree, summary, h))
    return changed


def _run_parallel_detection(storage, client, run_id, jobs, max_concurrency):
    """Execute LLM calls in parallel and store results. Returns (total, failed)."""
    print(f"  Submitting {len(jobs)} parallel Bedrock calls (concurrency={max_concurrency})...")

    total_patterns = 0
    failed_steps = []
    with ThreadPoolExecutor(max_workers=max_concurrency) as executor:
        future_to_step = {
            executor.submit(_invoke_llm, client, prompt): (step, h)
            for step, h, prompt in jobs
        }
        for future in as_completed(future_to_step):
            step, h = future_to_step[future]
            try:
                result = future.result()
                count = _store_patterns(storage, run_id, step, result)
                storage.record_processing(6, step["step_id"], h, run_id)
                total_patterns += count
                policy = step.get("policy_name", "")
                print(f"    {policy}/{step['title']}: {count} patterns (total: {total_patterns})")
            except Exception as e:
                print(f"    FAILED {step['title']}: {e}")
                failed_steps.append(step["step_id"])

    if failed_steps:
        print(f"\n  WARNING: {len(failed_steps)} steps failed pattern generation")

    return total_patterns, failed_steps


def run(storage: SVAPStorage, client: BedrockClient, run_id: str, config: dict):
    """Execute Stage 6: Generate detection patterns for approved exploitation steps."""
    print("Stage 6: Detection Pattern Generation")
    storage.log_stage_start(run_id, 6)

    try:
        stage5_status = storage.get_stage_status(run_id, 5)
        if stage5_status not in ("approved", "completed"):
            raise ValueError(
                f"Stage 5 status is '{stage5_status}'. Trees must be approved first."
            )

        trees = storage.get_exploitation_trees(approved_only=True)
        if not trees:
            raise ValueError("No approved exploitation trees found. Run Stage 5 first.")

        all_steps = _load_steps_from_trees(storage, trees)
        if not all_steps:
            raise ValueError("No exploitation steps found. Run Stage 5 first.")

        # ── Delta detection ─────────────────────────────────────────
        to_detect = _detect_changed_steps(storage, all_steps)
        if not to_detect:
            print(f"  All {len(all_steps)} steps unchanged — skipping.")
            storage.log_stage_complete(run_id, 6, {
                "patterns_generated": 0,
                "skipped_unchanged": len(all_steps),
            })
            return

        print(f"  {len(to_detect)}/{len(all_steps)} steps changed, generating patterns...")

        # ── Delete stale patterns BEFORE LLM calls ──────────────────
        for step, _tree, _summary, _h in to_detect:
            storage.delete_patterns_for_step(step["step_id"])

        data_sources_context = _get_data_sources_context(storage, config)

        max_concurrency = config.get("pipeline", {}).get("max_concurrency", 5)

        # Build all prompts (fast, sequential)
        jobs = []
        for step, tree, summary, h in to_detect:
            prompt = _build_prompt(client, step, tree, summary, data_sources_context)
            jobs.append((step, h, prompt))

        total_patterns, failed_steps = _run_parallel_detection(
            storage, client, run_id, jobs, max_concurrency,
        )

        all_patterns = storage.get_detection_patterns()
        print(f"\n  Stage 6 complete: {total_patterns} detection patterns generated.")
        _print_pattern_summary(all_patterns)

        result_meta = {"patterns_generated": total_patterns}
        if failed_steps:
            result_meta["failed_steps"] = len(failed_steps)
        storage.log_stage_complete(run_id, 6, result_meta)

    except Exception as e:
        storage.log_stage_failed(run_id, 6, str(e))
        raise


def _default_data_sources():
    """Default data source catalog for HHS context. Replace with your actual catalog."""
    return """Available data sources (replace with your actual data catalog):
- Claims Database: Medicare FFS claims (Part A, B, D), including procedure codes,
  diagnosis codes, provider NPIs, beneficiary IDs, dates, amounts
- Enrollment Database: Medicare/Medicaid beneficiary enrollment, plan selections,
  demographics, eligibility status
- Provider Enrollment: NPI registry, provider enrollment dates, specialty codes,
  practice locations, ownership information
- MA Encounter Data: Medicare Advantage plan encounter submissions, risk adjustment
  codes, plan identifiers
- EVV Data: Electronic Visit Verification records (Medicaid HCBS), GPS coordinates,
  check-in/check-out times
- Marketplace Enrollment: ACA marketplace applications, APTC amounts, broker IDs,
  plan selections, income attestations
- Exclusions Database: OIG exclusion list, state exclusion actions, CMS revocations
- Financial Data: Provider payment amounts, beneficiary cost-sharing, plan bid data"""
