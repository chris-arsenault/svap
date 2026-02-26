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

import json
import hashlib
from svap.storage import SVAPStorage
from svap.bedrock_client import BedrockClient
from svap.rag import ContextAssembler


SYSTEM_PROMPT = """You are a structural analyst predicting exploitation patterns. You NEVER 
speculate freely. Every prediction must be CAUSED by a specific vulnerability quality or 
combination of qualities present in the policy. If you cannot cite which structural quality 
enables a predicted behavior, do not include it.

Think like an adversary who has studied this policy's structural properties and is designing 
an exploitation scheme that takes maximum advantage of each vulnerability quality. The most 
dangerous schemes exploit the INTERACTION between multiple qualities, not just individual ones."""


def run(storage: SVAPStorage, client: BedrockClient, run_id: str, config: dict):
    """Execute Stage 5: Generate exploitation predictions for high-scoring policies."""
    print("Stage 5: Exploitation Prediction")
    storage.log_stage_start(run_id, 5)

    try:
        taxonomy = storage.get_taxonomy(run_id)
        calibration = storage.get_calibration(run_id)
        threshold = calibration["threshold"] if calibration else 3
        ctx = ContextAssembler(storage, config)

        # Get policies and their scores
        policies = storage.get_policies(run_id)
        policy_scores = storage.get_policy_scores(run_id)

        # Build per-policy score profiles
        policy_profiles = {}
        for ps in policy_scores:
            pid = ps["policy_id"]
            if pid not in policy_profiles:
                policy_profiles[pid] = {"name": ps["name"], "qualities": [], "count": 0}
            if ps["present"]:
                policy_profiles[pid]["qualities"].append(ps["quality_id"])
                policy_profiles[pid]["count"] += 1

        # Filter to policies at or above threshold
        high_risk = {pid: prof for pid, prof in policy_profiles.items() if prof["count"] >= threshold}

        if not high_risk:
            print(f"  No policies scored at or above threshold ({threshold}).")
            storage.log_stage_complete(run_id, 5, {"predictions_generated": 0})
            return

        print(f"  Generating predictions for {len(high_risk)} high-risk policies (threshold={threshold})...")

        # Build quality lookup for the prompt
        quality_lookup = {q["quality_id"]: q for q in taxonomy}
        total_predictions = 0

        for policy_id, profile in sorted(high_risk.items(), key=lambda x: -x[1]["count"]):
            print(f"    Predicting: {profile['name']} (score={profile['count']})")

            # Get full policy info
            policy = next((p for p in policies if p["policy_id"] == policy_id), None)
            if not policy:
                continue

            # Build quality profile for this policy
            quality_descriptions = []
            for qid in profile["qualities"]:
                q = quality_lookup.get(qid)
                if q:
                    # Get the evidence for how this quality manifests in this policy
                    evidence_row = next(
                        (ps for ps in policy_scores
                         if ps["policy_id"] == policy_id and ps["quality_id"] == qid and ps["present"]),
                        None
                    )
                    evidence = evidence_row["evidence"] if evidence_row else ""
                    quality_descriptions.append(
                        f"- {qid} ({q['name']}): {q['definition']}\n"
                        f"  How it manifests here: {evidence}"
                    )

            prompt = client.render_prompt(
                "stage5_predict.txt",
                policy_name=profile["name"],
                policy_description=policy.get("structural_characterization") or policy.get("description", ""),
                convergence_score=str(profile["count"]),
                quality_profile="\n".join(quality_descriptions),
            )

            result = client.invoke_json(
                prompt, system=SYSTEM_PROMPT, temperature=0.3, max_tokens=4096
            )

            predictions = result if isinstance(result, list) else result.get("predictions", [result])

            for i, pred_data in enumerate(predictions):
                pred_id = hashlib.sha256(
                    f"{policy_id}:{i}:{pred_data.get('mechanics', '')[:50]}".encode()
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
                total_predictions += 1

        # ── Human gate ──────────────────────────────────────────────
        storage.log_stage_pending_review(run_id, 5)

        print(f"\n  Stage 5 complete: {total_predictions} predictions generated.")
        print("  ⚠ HUMAN REVIEW REQUIRED before proceeding to Stage 6.")
        print(f"    Approve with: python -m svap.orchestrator approve --stage 5")

    except Exception as e:
        storage.log_stage_failed(run_id, 5, str(e))
        raise
