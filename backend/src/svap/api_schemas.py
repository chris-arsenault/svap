"""
Response shape transformers for the SVAP API.

The pipeline stores data in normalized PostgreSQL tables (cases, convergence_scores,
policy_scores, etc.). The UI expects enriched objects with computed fields
(qualities arrays, colors, risk levels). This module handles the transformation.
"""

from collections import defaultdict

_COLOR_PALETTE = [
    "var(--v1)", "var(--v2)", "var(--v3)", "var(--v4)",
    "var(--v5)", "var(--v6)", "var(--v7)", "var(--v8)",
]


def _quality_color(quality_id: str, all_quality_ids: list[str]) -> str:
    """Assign color by position in the sorted quality list."""
    try:
        idx = all_quality_ids.index(quality_id)
    except ValueError:
        idx = 0
    return _COLOR_PALETTE[idx % len(_COLOR_PALETTE)]


def enrich_cases(cases: list[dict], convergence_matrix: list[dict]) -> list[dict]:
    """Add `qualities` array to each case from convergence_scores JOIN."""
    case_qualities = defaultdict(list)
    for row in convergence_matrix:
        if row.get("present"):
            case_qualities[row["case_id"]].append(row["quality_id"])

    for case in cases:
        case["qualities"] = sorted(case_qualities.get(case["case_id"], []))
    return cases


def enrich_taxonomy(taxonomy: list[dict], convergence_matrix: list[dict]) -> list[dict]:
    """Add `color` and `case_count` to each taxonomy quality."""
    all_quality_ids = sorted(q["quality_id"] for q in taxonomy)
    quality_case_count = defaultdict(set)
    for row in convergence_matrix:
        if row.get("present"):
            quality_case_count[row["quality_id"]].add(row["case_id"])

    for q in taxonomy:
        q["color"] = _quality_color(q["quality_id"], all_quality_ids)
        q["case_count"] = len(quality_case_count.get(q["quality_id"], set()))
        # Parse canonical_examples from JSON string if needed
        if isinstance(q.get("canonical_examples"), str):
            import json

            try:
                q["canonical_examples"] = json.loads(q["canonical_examples"])
            except (json.JSONDecodeError, TypeError):
                q["canonical_examples"] = []
    return taxonomy


def enrich_policies(
    policies: list[dict], policy_scores: list[dict], calibration: dict | None
) -> list[dict]:
    """Add `qualities`, `convergence_score`, `risk_level` to each policy."""
    threshold = calibration["threshold"] if calibration else 3
    policy_qualities = defaultdict(list)
    for row in policy_scores:
        if row.get("present"):
            policy_qualities[row["policy_id"]].append(row["quality_id"])

    for p in policies:
        quals = sorted(policy_qualities.get(p["policy_id"], []))
        score = len(quals)
        p["qualities"] = quals
        p["convergence_score"] = score
        p["risk_level"] = compute_risk_level(score, threshold)
    return policies


def compute_risk_level(score: int, threshold: int) -> str:
    if score >= threshold + 2:
        return "critical"
    elif score >= threshold:
        return "high"
    elif score >= threshold - 1:
        return "medium"
    else:
        return "low"


def enrich_predictions(predictions: list[dict]) -> list[dict]:
    """Parse enabling_qualities from JSON string if needed."""
    import json

    for pred in predictions:
        if isinstance(pred.get("enabling_qualities"), str):
            try:
                pred["enabling_qualities"] = json.loads(pred["enabling_qualities"])
            except (json.JSONDecodeError, TypeError):
                pred["enabling_qualities"] = []
    return predictions


def build_case_convergence(cases: list[dict], convergence_matrix: list[dict]) -> list[dict]:
    """Build convergence summary for each case (for ConvergenceMatrix view)."""
    case_map = {c["case_id"]: c for c in cases}
    case_qualities = defaultdict(list)
    for row in convergence_matrix:
        if row.get("present"):
            case_qualities[row["case_id"]].append(row["quality_id"])

    result = []
    for case_id, quals in case_qualities.items():
        c = case_map.get(case_id, {})
        result.append(
            {
                "name": c.get("case_name", case_id),
                "score": len(quals),
                "scale": c.get("scale_dollars"),
                "qualities": sorted(quals),
            }
        )
    return sorted(result, key=lambda x: -x["score"])


def build_policy_convergence(policies: list[dict]) -> list[dict]:
    """Build convergence summary for each policy (already enriched with qualities)."""
    return [
        {
            "name": p["name"],
            "score": p.get("convergence_score", 0),
            "qualities": p.get("qualities", []),
        }
        for p in sorted(policies, key=lambda x: -x.get("convergence_score", 0))
    ]
