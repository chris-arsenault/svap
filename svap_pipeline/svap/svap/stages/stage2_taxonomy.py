"""
Stage 2: Vulnerability Taxonomy Extraction

Takes the enabling conditions from Stage 1 cases and abstracts them into
a reusable taxonomy of structural vulnerability qualities. This is the
intellectual core of the pipeline.

Two-pass process:
  Pass 1 (Clustering):  Group enabling conditions by structural similarity
  Pass 2 (Refinement):  Refine each quality with definition, recognition test, etc.

HUMAN GATE: This stage ends in 'pending_review' status. An SME must review
and approve the taxonomy before Stage 3 can run.

Input:  Cases from Stage 1 (specifically the enabling_condition field)
Output: Taxonomy of vulnerability qualities in the `taxonomy` table
"""

import json
from svap.storage import SVAPStorage
from svap.bedrock_client import BedrockClient
from svap.rag import ContextAssembler


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


def run(storage: SVAPStorage, client: BedrockClient, run_id: str, config: dict):
    """Execute Stage 2: Extract taxonomy from case enabling conditions."""
    print("Stage 2: Vulnerability Taxonomy Extraction")
    storage.log_stage_start(run_id, 2)

    try:
        cases = storage.get_cases(run_id)
        if not cases:
            raise ValueError("No cases found. Run Stage 1 first.")

        ctx = ContextAssembler(storage, config)

        # ── Pass 1: Clustering ──────────────────────────────────────
        print("  Pass 1: Clustering enabling conditions...")
        enabling_conditions = "\n\n".join(
            f"CASE: {c['case_name']}\nENABLING CONDITION: {c['enabling_condition']}"
            for c in cases
        )

        cluster_prompt = client.render_prompt(
            "stage2_cluster.txt",
            enabling_conditions=enabling_conditions,
            num_cases=str(len(cases)),
        )

        clusters = client.invoke_json(
            cluster_prompt, system=SYSTEM_PROMPT_CLUSTER, max_tokens=4096
        )
        qualities_draft = clusters if isinstance(clusters, list) else clusters.get("qualities", [])
        print(f"    Identified {len(qualities_draft)} draft qualities.")

        # ── Pass 2: Refinement ──────────────────────────────────────
        print("  Pass 2: Refining each quality...")
        all_quality_names = [q.get("name", "") for q in qualities_draft]

        for i, draft in enumerate(qualities_draft):
            quality_id = f"V{i+1}"
            print(f"    Refining {quality_id}: {draft.get('name', 'unnamed')}")

            other_qualities = [n for n in all_quality_names if n != draft.get("name")]
            refine_prompt = client.render_prompt(
                "stage2_refine.txt",
                quality_name=draft.get("name", ""),
                quality_definition=draft.get("definition", ""),
                example_conditions=json.dumps(draft.get("enabling_conditions", []), indent=2),
                other_quality_names=", ".join(other_qualities),
            )

            refined = client.invoke_json(
                refine_prompt, system=SYSTEM_PROMPT_REFINE, max_tokens=2048
            )

            quality = {
                "quality_id": quality_id,
                "name": refined.get("name", draft.get("name", f"Quality {i+1}")),
                "definition": refined.get("definition", draft.get("definition", "")),
                "recognition_test": refined.get("recognition_test", ""),
                "exploitation_logic": refined.get("exploitation_logic", ""),
                "canonical_examples": refined.get("canonical_examples", draft.get("enabling_conditions", [])),
            }
            storage.insert_quality(run_id, quality)

        # ── Human gate ──────────────────────────────────────────────
        taxonomy = storage.get_taxonomy(run_id)
        storage.log_stage_pending_review(run_id, 2)

        print(f"\n  Stage 2 complete: {len(taxonomy)} qualities extracted.")
        print("  ⚠ HUMAN REVIEW REQUIRED before proceeding to Stage 3.")
        print("    Review the taxonomy and approve/revise each quality:")
        for q in taxonomy:
            print(f"      {q['quality_id']}: {q['name']}")
        print(f"\n    Approve with: python -m svap.orchestrator approve --stage 2")
        print(f"    Export for review: python -m svap.orchestrator export --stage 2")

    except Exception as e:
        storage.log_stage_failed(run_id, 2, str(e))
        raise


def load_seed_taxonomy(storage: SVAPStorage, run_id: str, seed_path: str):
    """Load a pre-built taxonomy from a seed JSON file."""
    with open(seed_path) as f:
        qualities = json.load(f)
    for q in qualities:
        q["review_status"] = "approved"
        storage.insert_quality(run_id, q)
    print(f"  Loaded {len(qualities)} seed taxonomy qualities.")
