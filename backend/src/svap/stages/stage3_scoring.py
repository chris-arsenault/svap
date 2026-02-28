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
import logging
from collections import defaultdict

from svap import delta
from svap.bedrock_client import BedrockClient
from svap.rag import ContextAssembler
from svap.storage import SVAPStorage

logger = logging.getLogger(__name__)

SYSTEM_PROMPT = """You are scoring a policy against a structural vulnerability taxonomy.
Apply each recognition test precisely. A quality is PRESENT only if the policy clearly
exhibits the structural property described. If ambiguous, mark ABSENT and note the ambiguity
in the evidence field. Do not over-score."""


def _score_case(storage, client, run_id, case, taxonomy_context):
    """Score a single case against the taxonomy and store results."""
    logger.info("Scoring: %s", case['case_name'])
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


def _build_calibration_stats(matrix):
    """Build per-case scores, quality frequency, and co-occurrence from the convergence matrix."""
    case_scores = defaultdict(lambda: {"count": 0, "qualities": [], "scale": 0})
    quality_freq = defaultdict(int)

    for row in matrix:
        cid = row["case_id"]
        if case_scores[cid]["scale"] == 0:
            case_scores[cid]["scale"] = row.get("scale_dollars", 0) or 0
            case_scores[cid]["name"] = row["case_name"]
        if row["present"]:
            case_scores[cid]["count"] += 1
            case_scores[cid]["qualities"].append(row["quality_id"])
            quality_freq[row["quality_id"]] += 1

    quality_combos = defaultdict(int)
    for cs in case_scores.values():
        quals = sorted(cs["qualities"])
        for i in range(len(quals)):
            for j in range(i + 1, len(quals)):
                quality_combos[f"{quals[i]}+{quals[j]}"] += 1

    sorted_cases = sorted(case_scores.values(), key=lambda x: x["count"], reverse=True)
    return sorted_cases, dict(quality_freq), dict(quality_combos)


def _run_calibration(client, sorted_cases):
    """Ask the LLM to determine the calibration threshold from case scores."""
    calibration_data = json.dumps(
        [
            {"case": cs["name"], "score": cs["count"], "scale_dollars": cs["scale"]}
            for cs in sorted_cases
        ],
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

    return client.invoke_json(cal_prompt, max_tokens=1024)


def run(storage: SVAPStorage, client: BedrockClient, run_id: str, config: dict):
    """Execute Stage 3: Score all cases and calibrate."""
    logger.info("Stage 3: Convergence Scoring & Calibration")
    storage.log_stage_start(run_id, 3)

    try:
        stage2_status = storage.get_stage_status(run_id, 2)
        if stage2_status not in ("approved", "completed"):
            raise ValueError(
                f"Stage 2 status is '{stage2_status}'. Taxonomy must be approved before scoring. "
                f"Run: python -m svap.orchestrator approve --stage 2"
            )

        cases = storage.get_cases()
        taxonomy = storage.get_approved_taxonomy()
        ctx = ContextAssembler(storage, config)

        if not cases or not taxonomy:
            raise ValueError("Need both cases (Stage 1) and taxonomy (Stage 2).")

        taxonomy_context = ctx.format_taxonomy_context(taxonomy)

        # ── Delta detection ─────────────────────────────────────────
        tax_fp = delta.taxonomy_fingerprint(taxonomy)
        stored = storage.get_processing_hashes(3)
        cases_to_score = []
        skipped = 0
        for case in cases:
            h = delta.compute_hash(case["enabling_condition"], tax_fp)
            if stored.get(case["case_id"]) == h:
                skipped += 1
            else:
                cases_to_score.append((case, h))

        if not cases_to_score:
            logger.info("All %d cases unchanged. Skipping scoring.", len(cases))
            storage.log_stage_complete(run_id, 3, {"cases_scored": 0, "skipped": skipped})
            logger.info("Stage 3 complete (no changes).")
            return

        # ── Score each case ─────────────────────────────────────────
        logger.info("Scoring %d cases (%d unchanged)...", len(cases_to_score), skipped)
        for case, h in cases_to_score:
            _score_case(storage, client, run_id, case, taxonomy_context)
            storage.record_processing(3, case["case_id"], h, run_id)

        # ── Calibration analysis ────────────────────────────────────
        logger.info("Running calibration analysis...")
        matrix = storage.get_convergence_matrix()
        sorted_cases, quality_freq, quality_combos = _build_calibration_stats(matrix)
        cal_result = _run_calibration(client, sorted_cases)

        threshold = cal_result.get("threshold", 3)
        storage.insert_calibration(
            run_id,
            threshold=threshold,
            notes=cal_result.get("correlation_notes", ""),
            freq=quality_freq,
            combos=quality_combos,
        )

        logger.info("Calibration Results:")
        logger.info("Threshold: %d (policies scoring >=%d are high-priority)", threshold, threshold)
        logger.info("Quality frequency: %s", quality_freq)
        logger.info("Case scores:")
        for cs in sorted_cases:
            marker = "!" if cs["count"] >= threshold else " "
            logger.info("%s %s: score=%d, scale=$%,.0f", marker, cs['name'], cs['count'], cs['scale'])

        storage.log_stage_complete(run_id, 3, {"cases_scored": len(cases), "threshold": threshold})
        logger.info("Stage 3 complete.")

    except Exception as e:
        storage.log_stage_failed(run_id, 3, str(e))
        raise
