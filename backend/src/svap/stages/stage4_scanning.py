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
import logging

from svap import delta
from svap.bedrock_client import BedrockClient
from svap.rag import ContextAssembler
from svap.storage import SVAPStorage

logger = logging.getLogger(__name__)

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
        logger.info("Already characterized: %s", policy['name'])
        return

    logger.info("Characterizing: %s", policy['name'])
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
    logger.info("Scoring: %s", policy['name'])
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


def _filter_changed_policies(storage, policies, taxonomy):
    """Return (changed_list, skipped_count) using delta detection."""
    tax_fp = delta.taxonomy_fingerprint(taxonomy)
    stored_hashes = storage.get_processing_hashes(4)
    changed = []
    skipped = 0
    for policy in policies:
        h = delta.compute_hash(policy.get("structural_characterization", ""), tax_fp)
        if stored_hashes.get(policy["policy_id"]) == h:
            skipped += 1
        else:
            changed.append((policy, h))
    return changed, skipped


def run(storage: SVAPStorage, client: BedrockClient, run_id: str, config: dict):
    """Execute Stage 4: Characterize and score all policies."""
    logger.info("Stage 4: Policy Corpus Scanning")
    storage.log_stage_start(run_id, 4)

    try:
        taxonomy = storage.get_approved_taxonomy()
        calibration = storage.get_calibration()
        if not taxonomy:
            raise ValueError("No taxonomy found. Run Stages 1-3 first.")

        ctx = ContextAssembler(storage, config)
        taxonomy_context = ctx.format_taxonomy_context(taxonomy)
        threshold = calibration["threshold"] if calibration else 3

        policies = storage.get_policies()
        if not policies:
            logger.info("No policies pre-loaded. Extracting from policy documents...")
            policies = _extract_policies_from_docs(storage, client, config)

        if not policies:
            logger.info("No policies found. Load policies first.")
            storage.log_stage_complete(run_id, 4, {"policies_scored": 0})
            return

        # ── 4a: Structural Characterization ─────────────────────────
        logger.info("Characterizing %d policies...", len(policies))
        for policy in policies:
            _characterize_policy(storage, client, ctx, run_id, policy)

        policies = storage.get_policies()

        # ── 4b: Vulnerability Scoring ───────────────────────────────
        # Skip policies already assessed by deep research (Stage 4B/4C)
        policies_to_score = []
        for policy in policies:
            existing = storage.get_quality_assessments(policy["policy_id"])
            if existing:
                logger.info("Skipping %s -- already assessed via deep research", policy['name'])
            else:
                policies_to_score.append(policy)

        # Delta detection for scoring
        delta_policies, skipped = _filter_changed_policies(storage, policies_to_score, taxonomy)

        if not delta_policies:
            logger.info("All %d scorable policies unchanged. Skipping scoring.", len(policies_to_score))
            storage.log_stage_complete(run_id, 4, {
                "policies_scored": 0,
                "skipped_unchanged": skipped,
            })
            logger.info("Stage 4 complete (no changes).")
            return

        logger.info("Scoring %d policies (%d unchanged)...", len(delta_policies), skipped)
        results = []
        for policy, h in delta_policies:
            result = _score_policy(storage, client, run_id, policy, taxonomy_context)
            storage.record_processing(4, policy["policy_id"], h, run_id)
            results.append(result)

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
        logger.info("Stage 4 complete: %d policies scored.", len(results))

    except Exception as e:
        storage.log_stage_failed(run_id, 4, str(e))
        raise


def _print_ranking(results, threshold):
    """Print the policy vulnerability ranking."""
    logger.info("Policy Vulnerability Ranking (threshold=%d):", threshold)
    for r in results:
        if r["convergence_score"] >= threshold:
            marker = "[HIGH]"
        elif r["convergence_score"] >= threshold - 1:
            marker = "[MED] "
        else:
            marker = "[LOW] "
        logger.info("%s %s: score=%d", marker, r['policy'], r['convergence_score'])


def load_seed_policies(storage: SVAPStorage, seed_path: str):
    """Load pre-defined policies from a seed JSON file."""
    with open(seed_path) as f:
        policies = json.load(f)
    for p in policies:
        storage.insert_policy(p)
    logger.info("Loaded %d seed policies.", len(policies))


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
            logger.warning("Could not extract policies from %s: %s", doc['filename'], e)

    return all_policies
