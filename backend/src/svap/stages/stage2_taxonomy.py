"""
Stage 2: Vulnerability Taxonomy Extraction

Takes the enabling conditions from Stage 1 cases and abstracts them into
a reusable taxonomy of structural vulnerability qualities. This is the
intellectual core of the pipeline.

Three-pass iterative process:
  Pass 1 (Clustering):  Group enabling conditions from NEW cases by structural similarity
  Pass 2 (Refinement):  Refine each cluster into a full quality definition
  Pass 3 (Dedup):       Compare new qualities against existing taxonomy, merge or add

Delta: Only processes cases not yet recorded in taxonomy_case_log.
Semantic dedup: New qualities compared against existing taxonomy via LLM.

HUMAN GATE: If novel draft qualities are added, ends in 'pending_review'.
If all new qualities merge with existing taxonomy, completes automatically.

Input:  Cases from Stage 1 (specifically the enabling_condition field)
Output: Taxonomy of vulnerability qualities in the `taxonomy` table
"""

import hashlib
import json
import logging

from svap.bedrock_client import BedrockClient
from svap.rag import ContextAssembler
from svap.storage import SVAPStorage

logger = logging.getLogger(__name__)

SYSTEM_PROMPT_CLUSTER = """You are a structural analyst. Your task is to find the abstract
patterns that make policies exploitable. You think in terms of system design properties —
payment timing, verification architecture, information asymmetry, barrier structures —
not in terms of specific domains or actors. You are looking for qualities that would create
exploitable conditions in ANY policy system, not just the specific domain you're analyzing."""

SYSTEM_PROMPT_REFINE = """You are refining a taxonomy of structural vulnerability qualities.
Each quality must be precise enough that two independent analysts would agree on whether a
given policy has it. The recognition test must be a set of concrete yes/no questions, not
subjective judgments. The exploitation logic must articulate the causal mechanism — why
this structural property creates exploitable conditions."""

SYSTEM_PROMPT_DEDUP = """You are a taxonomy curator comparing a newly extracted vulnerability
quality against an existing approved taxonomy. Your task is to determine whether the new
quality is semantically equivalent to any existing quality. Two qualities are equivalent if
they describe the same fundamental structural property that creates exploitable conditions,
even if worded differently or illustrated with different examples."""


def _semantic_dedup(
    client: BedrockClient, draft: dict, existing_taxonomy: list[dict],
) -> dict | None:
    """Compare a draft quality against existing taxonomy.

    Returns the match result dict if semantically equivalent to an existing
    quality, or None if the draft is novel.
    """
    if not existing_taxonomy:
        return None

    existing_text = "\n\n".join(
        f"ID: {q['quality_id']}\n"
        f"Name: {q['name']}\n"
        f"Definition: {q['definition']}\n"
        f"Exploitation Logic: {q.get('exploitation_logic', '')}"
        for q in existing_taxonomy
    )

    prompt = client.render_prompt(
        "stage2_dedup.txt",
        new_name=draft.get("name", ""),
        new_definition=draft.get("definition", ""),
        new_exploitation_logic=draft.get("exploitation_logic", ""),
        existing_taxonomy=existing_text,
    )

    result = client.invoke_json(prompt, system=SYSTEM_PROMPT_DEDUP, max_tokens=1024)

    if result.get("match") and result.get("existing_quality_id"):
        valid_ids = {q["quality_id"] for q in existing_taxonomy}
        if result["existing_quality_id"] in valid_ids:
            return result
    return None


def run(storage: SVAPStorage, client: BedrockClient, run_id: str, config: dict):
    """Execute Stage 2: Extract taxonomy from case enabling conditions (delta)."""
    logger.info("Stage 2: Vulnerability Taxonomy Extraction")
    storage.log_stage_start(run_id, 2)

    try:
        # 1. Delta detection — find unprocessed cases
        cases = storage.get_cases()
        if not cases:
            raise ValueError("No cases found. Run Stage 1 first.")

        processed_ids = storage.get_taxonomy_processed_case_ids()
        new_cases = [c for c in cases if c["case_id"] not in processed_ids]

        if not new_cases:
            taxonomy = storage.get_taxonomy()
            logger.info(
                "All %d cases already processed for taxonomy. "
                "Nothing to extract. (%d qualities in taxonomy)",
                len(cases), len(taxonomy),
            )
            storage.log_stage_complete(run_id, 2, {
                "qualities_total": len(taxonomy),
                "cases_processed": 0,
                "note": "no new cases",
            })
            return

        logger.info(
            "%d new cases to process (%d already processed)",
            len(new_cases), len(cases) - len(new_cases),
        )

        ContextAssembler(storage, config)

        # 2. Pass 1: Cluster ONLY new cases' enabling conditions
        logger.info("Pass 1: Clustering enabling conditions from new cases...")
        enabling_conditions = "\n\n".join(
            f"CASE: {c['case_name']}\nENABLING CONDITION: {c['enabling_condition']}"
            for c in new_cases
        )

        cluster_prompt = client.render_prompt(
            "stage2_cluster.txt",
            enabling_conditions=enabling_conditions,
            num_cases=str(len(new_cases)),
        )

        clusters = client.invoke_json(
            cluster_prompt, system=SYSTEM_PROMPT_CLUSTER, max_tokens=4096,
        )
        qualities_draft = (
            clusters if isinstance(clusters, list)
            else clusters.get("qualities", [])
        )
        logger.info("Identified %d draft qualities.", len(qualities_draft))

        # 3. Pass 2: Refine each draft quality
        logger.info("Pass 2: Refining each quality...")
        all_quality_names = [q.get("name", "") for q in qualities_draft]
        refined_qualities = []

        for i, draft in enumerate(qualities_draft):
            name = draft.get("name", f"Quality {i + 1}")
            logger.info("Refining: %s", name)

            other_qualities = [n for n in all_quality_names if n != draft.get("name")]
            refine_prompt = client.render_prompt(
                "stage2_refine.txt",
                quality_name=draft.get("name", ""),
                quality_definition=draft.get("definition", ""),
                example_conditions=json.dumps(
                    draft.get("enabling_conditions", []), indent=2,
                ),
                other_quality_names=", ".join(other_qualities),
            )

            refined = client.invoke_json(
                refine_prompt, system=SYSTEM_PROMPT_REFINE, max_tokens=2048,
            )

            final_name = refined.get("name", name)
            quality_id = hashlib.sha256(final_name.encode()).hexdigest()[:8]

            refined_qualities.append({
                "quality_id": quality_id,
                "name": final_name,
                "definition": refined.get("definition", draft.get("definition", "")),
                "recognition_test": refined.get("recognition_test", ""),
                "exploitation_logic": refined.get("exploitation_logic", ""),
                "canonical_examples": refined.get(
                    "canonical_examples", draft.get("enabling_conditions", []),
                ),
            })

        # 4. Pass 3: Semantic deduplication against existing taxonomy
        logger.info("Pass 3: Semantic deduplication against existing taxonomy...")
        existing = storage.get_taxonomy()
        novel_qualities = []
        merged_count = 0

        for draft in refined_qualities:
            match = _semantic_dedup(client, draft, existing)
            if match:
                matched_id = match["existing_quality_id"]
                matched_name = next(
                    (q["name"] for q in existing if q["quality_id"] == matched_id),
                    matched_id,
                )
                logger.info(
                    "MERGED: '%s' -> existing '%s'", draft['name'], matched_name,
                )
                storage.merge_quality_examples(
                    matched_id, draft.get("canonical_examples", []),
                )
                merged_count += 1
            else:
                logger.info("NOVEL: '%s' -- adding as draft", draft['name'])
                draft["review_status"] = "draft"
                storage.insert_quality(draft)
                novel_qualities.append(draft)
                # Add to existing so subsequent dedup checks see it
                existing.append(draft)

        # 5. Record all new cases as processed
        for case in new_cases:
            storage.record_taxonomy_case_processed(case["case_id"])

        # 6. Report
        taxonomy = storage.get_taxonomy()
        logger.info("Stage 2 results:")
        logger.info("Cases processed:     %d", len(new_cases))
        logger.info("Draft qualities:     %d", len(refined_qualities))
        logger.info("Merged w/ existing:  %d", merged_count)
        logger.info("Novel (new drafts):  %d", len(novel_qualities))
        logger.info("Total taxonomy:      %d", len(taxonomy))

        # 7. Human gate only if new draft qualities need review
        if novel_qualities:
            storage.log_stage_pending_review(run_id, 2)
            logger.info("HUMAN REVIEW REQUIRED -- new draft qualities need approval:")
            for q in novel_qualities:
                logger.info("%s: %s", q['quality_id'], q['name'])
            logger.info("Approve with: python -m svap.orchestrator approve --stage 2")
        else:
            storage.log_stage_complete(run_id, 2, {
                "qualities_total": len(taxonomy),
                "cases_processed": len(new_cases),
                "merged": merged_count,
                "novel": 0,
            })
            logger.info("No new draft qualities -- stage complete, no review needed.")

    except Exception as e:
        storage.log_stage_failed(run_id, 2, str(e))
        raise


def load_seed_taxonomy(storage: SVAPStorage, seed_path: str):
    """Load a pre-built taxonomy from a seed JSON file."""
    with open(seed_path) as f:
        qualities = json.load(f)
    for q in qualities:
        q.setdefault("review_status", "approved")
        storage.insert_quality(q)
    logger.info("Loaded %d seed taxonomy qualities.", len(qualities))
