"""
Stage 3: Convergence Scoring & Calibration

Scores each known case against the full taxonomy to produce a convergence
matrix, then analyzes the matrix to identify the threshold where multiple
converging qualities predict large-scale exploitation.

This is the validation stage — if convergence scores don't correlate with
exploitation severity, the taxonomy (Stage 2) needs refinement.

Input:  Cases (Stage 1) + Approved Taxonomy (Stage 2)
Output: Convergence matrix + calibration threshold in `convergence_scores` and `calibration` tables
"""

import json
from collections import defaultdict
from svap.storage import SVAPStorage
from svap.bedrock_client import BedrockClient
from svap.rag import ContextAssembler


SYSTEM_PROMPT = """You are scoring a policy against a structural vulnerability taxonomy.
Apply each recognition test precisely. A quality is PRESENT only if the policy clearly 
exhibits the structural property described. If ambiguous, mark ABSENT and note the ambiguity 
in the evidence field. Do not over-score."""


def run(storage: SVAPStorage, client: BedrockClient, run_id: str, config: dict):
    """Execute Stage 3: Score all cases and calibrate."""
    print("Stage 3: Convergence Scoring & Calibration")
    storage.log_stage_start(run_id, 3)

    try:
        # Verify Stage 2 is approved
        stage2_status = storage.get_stage_status(run_id, 2)
        if stage2_status not in ("approved", "completed"):
            raise ValueError(
                f"Stage 2 status is '{stage2_status}'. Taxonomy must be approved before scoring. "
                f"Run: python -m svap.orchestrator approve --stage 2"
            )

        cases = storage.get_cases(run_id)
        taxonomy = storage.get_taxonomy(run_id)
        ctx = ContextAssembler(storage, config)

        if not cases or not taxonomy:
            raise ValueError("Need both cases (Stage 1) and taxonomy (Stage 2).")

        taxonomy_context = ctx.format_taxonomy_context(taxonomy)

        # ── Score each case ─────────────────────────────────────────
        print(f"  Scoring {len(cases)} cases against {len(taxonomy)} qualities...")
        for case in cases:
            print(f"    Scoring: {case['case_name']}")
            prompt = client.render_prompt(
                "stage3_score.txt",
                case_name=case["case_name"],
                exploited_policy=case["exploited_policy"],
                scheme_mechanics=case["scheme_mechanics"],
                enabling_condition=case["enabling_condition"],
                taxonomy=taxonomy_context,
            )

            scores = client.invoke_json(prompt, system=SYSTEM_PROMPT, max_tokens=2048)
            score_map = scores.get("scores", scores)

            for quality_id, score_data in score_map.items():
                if isinstance(score_data, dict):
                    present = score_data.get("present", False)
                    evidence = score_data.get("evidence", "")
                else:
                    present = bool(score_data)
                    evidence = ""
                storage.insert_convergence_score(run_id, case["case_id"], quality_id, present, evidence)

        # ── Calibration analysis ────────────────────────────────────
        print("  Running calibration analysis...")
        matrix = storage.get_convergence_matrix(run_id)

        # Build per-case convergence counts
        case_scores = defaultdict(lambda: {"count": 0, "qualities": [], "scale": 0})
        for row in matrix:
            cid = row["case_id"]
            if case_scores[cid]["scale"] == 0:
                case_scores[cid]["scale"] = row.get("scale_dollars", 0) or 0
                case_scores[cid]["name"] = row["case_name"]
            if row["present"]:
                case_scores[cid]["count"] += 1
                case_scores[cid]["qualities"].append(row["quality_id"])

        # Find threshold
        sorted_cases = sorted(case_scores.values(), key=lambda x: x["count"], reverse=True)

        # Quality frequency
        quality_freq = defaultdict(int)
        for row in matrix:
            if row["present"]:
                quality_freq[row["quality_id"]] += 1

        # Quality co-occurrence
        quality_combos = defaultdict(int)
        for cs in case_scores.values():
            quals = sorted(cs["qualities"])
            for i in range(len(quals)):
                for j in range(i + 1, len(quals)):
                    quality_combos[f"{quals[i]}+{quals[j]}"] += 1

        # Determine threshold via correlation analysis prompt
        calibration_data = json.dumps(
            [{"case": cs["name"], "score": cs["count"], "scale_dollars": cs["scale"]}
             for cs in sorted_cases],
            indent=2,
        )

        cal_prompt = f"""Analyze this convergence score data to determine the calibration threshold.

Each entry shows a known exploitation case, its convergence score (number of vulnerability 
qualities present), and the scale in dollars.

{calibration_data}

Determine:
1. THRESHOLD: The convergence score at or above which exploitation tends to be large-scale 
   (>$100M+). This is the minimum score that should trigger proactive investigation.
2. CORRELATION_NOTES: Describe the relationship between convergence score and scale. 
   Is it linear? Is there a clear step-function? Are there outliers?

Return JSON: {{"threshold": N, "correlation_notes": "..."}}"""

        cal_result = client.invoke_json(cal_prompt, max_tokens=1024)

        storage.insert_calibration(
            run_id,
            threshold=cal_result.get("threshold", 3),
            notes=cal_result.get("correlation_notes", ""),
            freq=dict(quality_freq),
            combos=dict(quality_combos),
        )

        threshold = cal_result.get("threshold", 3)
        print(f"\n  Calibration Results:")
        print(f"    Threshold: {threshold} (policies scoring ≥{threshold} are high-priority)")
        print(f"    Quality frequency: {dict(quality_freq)}")
        print(f"    Case scores:")
        for cs in sorted_cases:
            marker = "⚠" if cs["count"] >= threshold else " "
            print(f"      {marker} {cs['name']}: score={cs['count']}, scale=${cs['scale']:,.0f}")

        storage.log_stage_complete(run_id, 3, {
            "cases_scored": len(cases),
            "threshold": threshold,
        })
        print(f"\n  Stage 3 complete.")

    except Exception as e:
        storage.log_stage_failed(run_id, 3, str(e))
        raise
