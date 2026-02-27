"""
Stage 4: Policy Corpus Scanning

Applies the validated taxonomy to policies that have NOT been the subject of
major exploitation cases. Two sub-steps:
  4a: Structural Characterization — extract structural properties from policy text
  4b: Vulnerability Scoring — score each policy against the taxonomy

Input:  Policies (loaded or extracted from docs) + Taxonomy (Stage 2) + Calibration (Stage 3)
Output: Scored and ranked policy list in `policies` and `policy_scores` tables
"""

import hashlib
import json

from svap.bedrock_client import BedrockClient
from svap.rag import ContextAssembler
from svap.storage import SVAPStorage

SYSTEM_PROMPT_CHARACTERIZE = """You are a structural analyst characterizing how a government
policy or program works. Focus on the mechanical structure: how money flows, who reports what,
what verification exists, what barriers exist to participation. Be concrete and specific.
Do not evaluate whether the policy is good or bad — just describe its structural properties."""

SYSTEM_PROMPT_SCORE = """You are scoring a policy against a structural vulnerability taxonomy.
Apply each recognition test to the policy's structural characterization. A quality is PRESENT
only if the structural characterization clearly shows the property. Mark ABSENT if ambiguous.
Be conservative — false negatives are better than false positives at this stage."""


def _characterize_policy(storage, client, ctx, run_id, policy):
    """Run structural characterization for a single policy."""
    if policy.get("structural_characterization"):
        print(f"    Already characterized: {policy['name']}")
        return

    print(f"    Characterizing: {policy['name']}")
    rag_context = ctx.retrieve(
        policy["name"] + " " + (policy.get("description", "") or ""), doc_type="policy"
    )

    prompt = client.render_prompt(
        "stage4_characterize.txt",
        policy_name=policy["name"],
        policy_description=policy.get("description", "No description provided."),
        rag_context=rag_context or "No additional source documents available.",
    )

    characterization = client.invoke(prompt, system=SYSTEM_PROMPT_CHARACTERIZE, max_tokens=2048)
    policy["structural_characterization"] = characterization
    storage.insert_policy(policy)


def _score_policy(storage, client, run_id, policy, taxonomy_context):
    """Score a single policy against the taxonomy and return the result."""
    print(f"    Scoring: {policy['name']}")
    prompt = client.render_prompt(
        "stage4_score.txt",
        policy_name=policy["name"],
        structural_characterization=policy.get(
            "structural_characterization", policy.get("description", "")
        ),
        taxonomy=taxonomy_context,
    )

    scores = client.invoke_json(prompt, system=SYSTEM_PROMPT_SCORE, max_tokens=2048)
    score_map = scores.get("scores", scores)

    convergence_count = 0
    for quality_id, score_data in score_map.items():
        if isinstance(score_data, dict):
            present = score_data.get("present", False)
            evidence = score_data.get("evidence", "")
        else:
            present = bool(score_data)
            evidence = ""
        storage.insert_policy_score(run_id, policy["policy_id"], quality_id, present, evidence)
        if present:
            convergence_count += 1

    return {
        "policy": policy["name"],
        "policy_id": policy["policy_id"],
        "convergence_score": convergence_count,
    }


def run(storage: SVAPStorage, client: BedrockClient, run_id: str, config: dict):
    """Execute Stage 4: Characterize and score all policies."""
    print("Stage 4: Policy Corpus Scanning")
    storage.log_stage_start(run_id, 4)

    try:
        taxonomy = storage.get_approved_taxonomy()
        calibration = storage.get_calibration(run_id)
        if not taxonomy:
            raise ValueError("No taxonomy found. Run Stages 1-3 first.")

        ctx = ContextAssembler(storage, config)
        taxonomy_context = ctx.format_taxonomy_context(taxonomy)
        threshold = calibration["threshold"] if calibration else 3

        policies = storage.get_policies()
        if not policies:
            print("  No policies pre-loaded. Extracting from policy documents...")
            policies = _extract_policies_from_docs(storage, client, config)

        if not policies:
            print("  No policies found. Load policies first.")
            storage.log_stage_complete(run_id, 4, {"policies_scored": 0})
            return

        # ── 4a: Structural Characterization ─────────────────────────
        print(f"  Characterizing {len(policies)} policies...")
        for policy in policies:
            _characterize_policy(storage, client, ctx, run_id, policy)

        policies = storage.get_policies()

        # ── 4b: Vulnerability Scoring ───────────────────────────────
        # Skip policies already assessed by deep research (Stage 4B/4C)
        policies_to_score = []
        for policy in policies:
            existing = storage.get_quality_assessments(run_id, policy["policy_id"])
            if existing:
                print(f"    Skipping {policy['name']} — already assessed via deep research")
            else:
                policies_to_score.append(policy)

        print(f"  Scoring {len(policies_to_score)} policies against {len(taxonomy)} qualities...")
        results = [
            _score_policy(storage, client, run_id, policy, taxonomy_context) for policy in policies_to_score
        ]

        # ── Report ──────────────────────────────────────────────────
        results.sort(key=lambda x: x["convergence_score"], reverse=True)
        _print_ranking(results, threshold)

        storage.log_stage_complete(
            run_id,
            4,
            {
                "policies_scored": len(results),
                "above_threshold": sum(1 for r in results if r["convergence_score"] >= threshold),
            },
        )
        print(f"\n  Stage 4 complete: {len(results)} policies scored.")

    except Exception as e:
        storage.log_stage_failed(run_id, 4, str(e))
        raise


def _print_ranking(results, threshold):
    """Print the policy vulnerability ranking."""
    print(f"\n  Policy Vulnerability Ranking (threshold={threshold}):")
    for r in results:
        if r["convergence_score"] >= threshold:
            marker = "[HIGH]"
        elif r["convergence_score"] >= threshold - 1:
            marker = "[MED] "
        else:
            marker = "[LOW] "
        print(f"    {marker} {r['policy']}: score={r['convergence_score']}")


def load_seed_policies(storage: SVAPStorage, seed_path: str):
    """Load pre-defined policies from a seed JSON file."""
    with open(seed_path) as f:
        policies = json.load(f)
    for p in policies:
        storage.insert_policy(p)
    print(f"  Loaded {len(policies)} seed policies.")


def _extract_policies_from_docs(storage, client, config):
    """Extract policy descriptions from policy documents in RAG store."""
    docs = storage.get_all_documents(doc_type="policy")
    if not docs:
        return []

    all_policies = []
    for doc in docs:
        prompt = f"""Extract a list of distinct government policies, programs, or payment
models described in this document. For each, provide a name and brief description.

Return JSON: [{{"name": "...", "description": "..."}}]

DOCUMENT:
{doc["full_text"][:8000]}"""

        try:
            extracted = client.invoke_json(prompt, max_tokens=2048)
            items = extracted if isinstance(extracted, list) else extracted.get("policies", [])
            for item in items:
                policy_id = hashlib.sha256(item["name"].encode()).hexdigest()[:12]
                policy = {
                    "policy_id": policy_id,
                    "name": item["name"],
                    "description": item.get("description", ""),
                    "source_document": doc["filename"],
                }
                storage.insert_policy(policy)
                all_policies.append(policy)
        except Exception as e:
            print(f"    Warning: Could not extract policies from {doc['filename']}: {e}")

    return all_policies
