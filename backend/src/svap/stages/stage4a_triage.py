"""
Stage 4A: Policy Triage — Shallow Vulnerability Ranking

Single LLM call that ranks all policies by likely vulnerability concentration.
Uses LLM training knowledge + existing case corpus to prioritize which policies
should receive expensive deep structural research.

Input:  Policies, taxonomy, cases from current run
Output: triage_results table (ranked policies with scores and rationale)
"""


import logging

from svap import delta
from svap.bedrock_client import BedrockClient
from svap.storage import SVAPStorage

logger = logging.getLogger(__name__)

TRIAGE_SYSTEM = (
    "You are an expert healthcare policy analyst assessing structural vulnerability "
    "to fraud. You rank policies by how many vulnerability qualities are likely "
    "present based on how the program actually operates — payment mechanics, "
    "verification structure, eligibility controls, and oversight architecture."
)


def run(storage: SVAPStorage, client: BedrockClient, run_id: str, config: dict):
    """Execute Pass 1: Triage all policies by vulnerability likelihood."""
    logger.info("Stage 4A: Policy Triage")
    storage.log_stage_start(run_id, 40)  # 40 = stage 4a in integer form

    try:
        storage.seed_dimensions_if_empty()
        policies = storage.get_policies()
        taxonomy = storage.get_approved_taxonomy()
        cases = storage.get_cases()

        if not policies:
            logger.info("No policies found. Run Stage 4 or seed policies first.")
            storage.log_stage_complete(run_id, 40, {"policies_triaged": 0})
            return

        if not taxonomy:
            logger.info("No taxonomy found. Run Stage 2 first.")
            storage.log_stage_complete(run_id, 40, {"policies_triaged": 0})
            return

        # Delta detection — single batch entity
        h = delta.compute_hash(
            ":".join(sorted(p["policy_id"] for p in policies)),
            delta.taxonomy_fingerprint(taxonomy),
            ":".join(sorted(c["case_id"] for c in cases)),
        )
        stored_hashes = storage.get_processing_hashes(40)
        if stored_hashes.get("triage_batch") == h:
            logger.info("Triage inputs unchanged. Skipping.")
            storage.log_stage_complete(run_id, 40, {"policies_triaged": 0, "skipped": True})
            return

        prompt = client.render_prompt(
            "stage4a_triage.txt",
            n_policies=len(policies),
            taxonomy_summary=_format_taxonomy(taxonomy),
            case_summary=_format_cases(cases),
            policy_list=_format_policies(policies),
        )

        result = client.invoke_json(prompt, system=TRIAGE_SYSTEM, max_tokens=8192)
        rankings = result.get("rankings", [])

        stored = 0
        for i, entry in enumerate(rankings):
            policy_id = _resolve_policy_id(entry.get("policy_name", ""), policies)
            if not policy_id:
                logger.warning("Could not match policy '%s'", entry.get('policy_name'))
                continue

            storage.insert_triage_result(run_id, {
                "policy_id": policy_id,
                "triage_score": float(entry.get("score", 0.0)),
                "rationale": entry.get("rationale", ""),
                "uncertainty": entry.get("uncertainty", ""),
                "priority_rank": i + 1,
            })
            storage.update_policy_lifecycle(policy_id, "triaged")
            stored += 1
            logger.info("#%d: %s -- score=%.2f", i + 1, entry.get('policy_name', '?'), entry.get('score', 0))

        storage.record_processing(40, "triage_batch", h, run_id)
        storage.log_stage_complete(run_id, 40, {
            "policies_triaged": stored,
            "total_rankings": len(rankings),
        })
        logger.info("Triage complete: %d policies ranked.", stored)

    except Exception as e:
        storage.log_stage_failed(run_id, 40, str(e))
        raise


def _format_taxonomy(taxonomy: list[dict]) -> str:
    parts = []
    for q in taxonomy:
        parts.append(
            f"{q['quality_id']}: {q['name']}\n"
            f"  Definition: {q['definition']}\n"
            f"  Recognition test: {q['recognition_test']}"
        )
    return "\n\n".join(parts)


def _format_cases(cases: list[dict]) -> str:
    if not cases:
        return "No cases in corpus yet."
    parts = []
    for c in cases[:20]:  # limit to 20 for context
        parts.append(
            f"- {c['case_name']}: {c['exploited_policy'][:100]}\n"
            f"  Enabling condition: {c['enabling_condition'][:150]}"
        )
    return "\n".join(parts)


def _format_policies(policies: list[dict]) -> str:
    parts = []
    for p in policies:
        desc = p.get("description") or "No description"
        parts.append(f"- {p['name']}: {desc[:200]}")
    return "\n".join(parts)


def _resolve_policy_id(name: str, policies: list[dict]) -> str | None:
    """Match a policy name from LLM output to an actual policy_id."""
    name_lower = name.lower().strip()
    for p in policies:
        if p["name"].lower().strip() == name_lower:
            return p["policy_id"]
    # Fuzzy: check if the LLM name is contained in or contains the policy name
    for p in policies:
        pname = p["name"].lower().strip()
        if name_lower in pname or pname in name_lower:
            return p["policy_id"]
    return None
