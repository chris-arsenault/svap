"""
Stage 5: Exploitation Tree Generation

For each high-scoring policy from Stage 4, generates a structured exploitation
tree showing all plausible exploitation paths. Trees have shared early steps
that branch into divergent exploitation paths, each citing enabling qualities.

HUMAN GATE: This stage ends in 'pending_review' status. SMEs must validate
that trees are structurally sound before detection resources are allocated.

Input:  Scored policies (Stage 4) + Taxonomy + Calibration threshold
Output: Exploitation trees in `exploitation_trees` + `exploitation_steps` tables
"""

import hashlib

from svap import delta
from svap.bedrock_client import BedrockClient
from svap.parallel import run_parallel_llm
from svap.storage import SVAPStorage

SYSTEM_PROMPT = """You are a structural analyst building exploitation decision trees. You NEVER
speculate freely. Every step must be CAUSED by a specific vulnerability quality or
combination of qualities present in the policy. If you cannot cite which structural quality
enables a step, do not include it.

Think like an adversary mapping all viable exploitation paths through this policy's structural
properties. The most dangerous paths exploit the INTERACTION between multiple qualities. Shared
setup steps appear once; distinct exploitation mechanisms branch into separate paths."""


def _build_policy_profiles(policy_scores):
    """Build per-policy score profiles from raw policy scores."""
    profiles = {}
    for ps in policy_scores:
        pid = ps["policy_id"]
        if pid not in profiles:
            profiles[pid] = {"name": ps["name"], "qualities": [], "count": 0}
        if ps["present"]:
            profiles[pid]["qualities"].append(ps["quality_id"])
            profiles[pid]["count"] += 1
    return profiles


def _build_prompt(client, policy_id, profile, policy, quality_lookup, policy_scores):
    """Build the LLM prompt for a single policy. No I/O besides template read."""
    quality_descriptions = []
    for qid in profile["qualities"]:
        q = quality_lookup.get(qid)
        if not q:
            continue
        evidence_row = next(
            (
                ps
                for ps in policy_scores
                if ps["policy_id"] == policy_id and ps["quality_id"] == qid and ps["present"]
            ),
            None,
        )
        evidence = evidence_row["evidence"] if evidence_row else ""
        quality_descriptions.append(
            f"- {qid} ({q['name']}): {q['definition']}\n  How it manifests here: {evidence}"
        )

    return client.render_prompt(
        "stage5_predict.txt",
        policy_name=profile["name"],
        policy_description=policy.get("structural_characterization")
        or policy.get("description", ""),
        convergence_score=str(profile["count"]),
        quality_profile="\n".join(quality_descriptions),
    )


def _invoke_llm(client, prompt):
    """Make the Bedrock call. Thread-safe (boto3 clients are thread-safe)."""
    return client.invoke_json(prompt, system=SYSTEM_PROMPT, temperature=0.3, max_tokens=4096)


def _store_tree(storage, run_id, policy_id, profile, result):
    """Parse LLM result and write exploitation tree + steps to DB. Returns step count."""
    tree_id = hashlib.sha256(policy_id.encode()).hexdigest()[:12]
    tree = {
        "tree_id": tree_id,
        "policy_id": policy_id,
        "convergence_score": profile["count"],
        "actor_profile": result.get("actor_profile", ""),
        "lifecycle_stage": result.get("lifecycle_stage", ""),
        "detection_difficulty": result.get("detection_difficulty", ""),
    }
    storage.insert_exploitation_tree(run_id, tree)

    steps = result.get("steps", [])
    if not steps:
        return 0

    order_to_id = {}
    for step_data in steps:
        order = step_data["step_order"]
        step_id = hashlib.sha256(
            f"{policy_id}:step:{order}:{step_data['title'][:50]}".encode()
        ).hexdigest()[:12]
        order_to_id[order] = step_id

        parent_order = step_data.get("parent_step_order")
        parent_id = order_to_id.get(parent_order) if parent_order else None
        if parent_order and parent_id is None:
            print(f"    WARNING: step {order} references parent_step_order={parent_order} "
                  f"which hasn't been seen yet — treating as root")

        step = {
            "step_id": step_id,
            "tree_id": tree_id,
            "parent_step_id": parent_id,
            "step_order": order,
            "title": step_data["title"],
            "description": step_data.get("description", ""),
            "actor_action": step_data.get("actor_action", ""),
            "is_branch_point": step_data.get("is_branch_point", False),
            "branch_label": step_data.get("branch_label"),
        }
        qualities = step_data.get("enabling_qualities", [])
        storage.insert_exploitation_step(step, qualities)

    return len(steps)


def _run_parallel_predictions(storage, client, run_id, jobs, max_concurrency):
    """Execute LLM calls in parallel and store results. Returns (total_steps, failed)."""

    def _on_result(result, ctx):
        count = _store_tree(storage, run_id, ctx["policy_id"], ctx["profile"], result)
        storage.record_processing(5, ctx["policy_id"], ctx["h"], run_id)
        return count

    parallel_jobs = [
        (profile["name"], prompt, {"policy_id": policy_id, "profile": profile, "h": h})
        for policy_id, profile, h, prompt in jobs
    ]
    return run_parallel_llm(
        lambda prompt: _invoke_llm(client, prompt),
        parallel_jobs, _on_result, max_concurrency,
    )


def run(storage: SVAPStorage, client: BedrockClient, run_id: str, config: dict):
    """Execute Stage 5: Generate exploitation trees for high-scoring policies."""
    print("Stage 5: Exploitation Tree Generation")
    storage.log_stage_start(run_id, 5)

    try:
        taxonomy = storage.get_approved_taxonomy()
        calibration = storage.get_calibration()
        threshold = calibration["threshold"] if calibration else 3

        policies = storage.get_policies()
        policy_scores = storage.get_policy_scores()

        policy_profiles = _build_policy_profiles(policy_scores)
        high_risk = {
            pid: prof for pid, prof in policy_profiles.items() if prof["count"] >= threshold
        }

        if not high_risk:
            print(f"  No policies scored at or above threshold ({threshold}).")
            storage.log_stage_complete(run_id, 5, {"trees_generated": 0})
            return

        # -- Delta detection -----------------------------------------------
        cal_fp = delta.calibration_fingerprint(calibration)
        stored = storage.get_processing_hashes(5)

        to_predict = []
        for policy_id, profile in sorted(high_risk.items(), key=lambda x: -x[1]["count"]):
            quality_profile = delta.policy_quality_profile(policy_id, policy_scores)
            h = delta.compute_hash(quality_profile, cal_fp)
            if stored.get(policy_id) != h:
                to_predict.append((policy_id, profile, h))

        if not to_predict:
            print(f"  All {len(high_risk)} high-risk policies unchanged — skipping.")
            storage.log_stage_complete(run_id, 5, {
                "trees_generated": 0,
                "skipped_unchanged": len(high_risk),
            })
            return

        print(
            f"  {len(to_predict)}/{len(high_risk)} policies changed "
            f"(threshold={threshold}), generating exploitation trees..."
        )

        # -- Delete stale data BEFORE LLM calls ----------------------------
        for policy_id, _profile, _h in to_predict:
            storage.delete_tree_for_policy(policy_id)

        quality_lookup = {q["quality_id"]: q for q in taxonomy}
        max_concurrency = config.get("pipeline", {}).get("max_concurrency", 5)

        # Build all prompts (fast, sequential)
        jobs = []
        for policy_id, profile, h in to_predict:
            policy = next((p for p in policies if p["policy_id"] == policy_id), None)
            if not policy:
                continue
            prompt = _build_prompt(client, policy_id, profile, policy, quality_lookup, policy_scores)
            jobs.append((policy_id, profile, h, prompt))

        total_steps, failed_policies = _run_parallel_predictions(
            storage, client, run_id, jobs, max_concurrency,
        )

        if total_steps > 0:
            storage.log_stage_pending_review(run_id, 5)
            print(f"\n  Stage 5 complete: {total_steps} steps generated across {len(jobs) - len(failed_policies)} trees.")
            print("  HUMAN REVIEW REQUIRED before proceeding to Stage 6.")
            print("    Approve with: python -m svap.orchestrator approve --stage 5")
        else:
            storage.log_stage_complete(run_id, 5, {
                "trees_generated": 0,
                "steps_generated": 0,
                "all_failed": len(failed_policies),
            })
            print(f"\n  Stage 5 complete: no steps generated ({len(failed_policies)} failed).")

    except Exception as e:
        storage.log_stage_failed(run_id, 5, str(e))
        raise
