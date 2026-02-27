"""
Stage 5: Exploitation Prediction

For each high-scoring policy from Stage 4, generates specific predictions about
what exploitation would look like. Predictions are constrained to be structurally
entailed by the vulnerability qualities present — not free-form speculation.

HUMAN GATE: This stage ends in 'pending_review' status. SMEs must validate
that predictions are structurally sound before detection resources are allocated.

Input:  Scored policies (Stage 4) + Taxonomy + Calibration threshold
Output: Exploitation predictions in the `predictions` table
"""

import hashlib
from concurrent.futures import ThreadPoolExecutor, as_completed

from svap import delta
from svap.bedrock_client import BedrockClient
from svap.storage import SVAPStorage

SYSTEM_PROMPT = """You are a structural analyst predicting exploitation patterns. You NEVER
speculate freely. Every prediction must be CAUSED by a specific vulnerability quality or
combination of qualities present in the policy. If you cannot cite which structural quality
enables a predicted behavior, do not include it.

Think like an adversary who has studied this policy's structural properties and is designing
an exploitation scheme that takes maximum advantage of each vulnerability quality. The most
dangerous schemes exploit the INTERACTION between multiple qualities, not just individual ones."""


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


def _store_predictions(storage, run_id, policy_id, profile, result):
    """Parse LLM result and write predictions to DB. Returns count."""
    predictions = result if isinstance(result, list) else result.get("predictions", [result])
    count = 0
    for i, pred_data in enumerate(predictions):
        pred_id = hashlib.sha256(
            f"{policy_id}:pred:{i}".encode()
        ).hexdigest()[:12]

        pred = {
            "prediction_id": pred_id,
            "policy_id": policy_id,
            "convergence_score": profile["count"],
            "mechanics": pred_data.get("mechanics", ""),
            "enabling_qualities": pred_data.get("enabling_qualities", profile["qualities"]),
            "actor_profile": pred_data.get("actor_profile", ""),
            "lifecycle_stage": pred_data.get("lifecycle_stage", ""),
            "detection_difficulty": pred_data.get("detection_difficulty", ""),
        }
        storage.insert_prediction(run_id, pred)
        count += 1
    return count


def _run_parallel_predictions(storage, client, run_id, jobs, max_concurrency):
    """Execute LLM calls in parallel and store results. Returns (total, failed)."""
    print(f"  Submitting {len(jobs)} parallel Bedrock calls (concurrency={max_concurrency})...")

    total_predictions = 0
    failed_policies = []
    with ThreadPoolExecutor(max_workers=max_concurrency) as executor:
        future_to_policy = {
            executor.submit(_invoke_llm, client, prompt): (policy_id, profile, h)
            for policy_id, profile, h, prompt in jobs
        }
        for future in as_completed(future_to_policy):
            policy_id, profile, h = future_to_policy[future]
            try:
                result = future.result()
                count = _store_predictions(storage, run_id, policy_id, profile, result)
                storage.record_processing(5, policy_id, h, run_id)
                total_predictions += count
                print(f"    {profile['name']}: {count} predictions (total: {total_predictions})")
            except Exception as e:
                print(f"    FAILED {profile['name']}: {e}")
                failed_policies.append(policy_id)

    if failed_policies:
        print(f"\n  WARNING: {len(failed_policies)} policies failed prediction generation")

    return total_predictions, failed_policies


def run(storage: SVAPStorage, client: BedrockClient, run_id: str, config: dict):
    """Execute Stage 5: Generate exploitation predictions for high-scoring policies."""
    print("Stage 5: Exploitation Prediction")
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
            storage.log_stage_complete(run_id, 5, {"predictions_generated": 0})
            return

        # ── Delta detection ─────────────────────────────────────────
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
                "predictions_generated": 0,
                "skipped_unchanged": len(high_risk),
            })
            return

        print(
            f"  {len(to_predict)}/{len(high_risk)} policies changed "
            f"(threshold={threshold}), generating predictions..."
        )

        # ── Delete stale data BEFORE LLM calls ─────────────────────
        for policy_id, _profile, _h in to_predict:
            storage.delete_predictions_for_policy(policy_id)

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

        total_predictions, failed_policies = _run_parallel_predictions(
            storage, client, run_id, jobs, max_concurrency,
        )

        if total_predictions > 0:
            storage.log_stage_pending_review(run_id, 5)
            print(f"\n  Stage 5 complete: {total_predictions} predictions generated.")
            print("  HUMAN REVIEW REQUIRED before proceeding to Stage 6.")
            print("    Approve with: python -m svap.orchestrator approve --stage 5")
        else:
            storage.log_stage_complete(run_id, 5, {
                "predictions_generated": 0,
                "all_failed": len(failed_policies),
            })
            print(f"\n  Stage 5 complete: no predictions generated ({len(failed_policies)} failed).")

    except Exception as e:
        storage.log_stage_failed(run_id, 5, str(e))
        raise
