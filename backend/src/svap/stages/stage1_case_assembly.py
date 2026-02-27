"""
Stage 1: Case Corpus Assembly

Reads enforcement documents (press releases, settlement agreements, OIG reports)
and extracts structured case data using LLM-assisted extraction.

Input:  Documents in the RAG store (doc_type='enforcement')
Output: Structured cases in the `cases` table
"""

import hashlib
import json
import re

from svap.bedrock_client import BedrockClient
from svap.storage import SVAPStorage

SYSTEM_PROMPT = """You are an analyst extracting structured information from enforcement
documents. You extract the mechanical details of how schemes operated, not just legal
conclusions. Be precise about the enabling policy structure — identify the specific
design feature that was exploited, not generic labels like "weak oversight"."""


def run(storage: SVAPStorage, client: BedrockClient, run_id: str, config: dict):
    """
    Execute Stage 1: Extract cases from all enforcement documents in the RAG store.

    For each enforcement document, sends it to Claude with the extraction prompt,
    parses the structured output, and stores each case in the database.

    Incremental: documents that already have extracted cases in the database
    are skipped.
    """
    print("Stage 1: Case Corpus Assembly")
    storage.log_stage_start(run_id, 1)

    try:
        docs = storage.get_all_documents(doc_type="enforcement")
        if not docs:
            print("  No enforcement documents found. Load documents first.")
            print("  Use: orchestrator.py ingest --path /your/docs --type enforcement")
            storage.log_stage_complete(run_id, 1, {"cases_extracted": 0, "note": "no documents"})
            return

        # Skip documents that already have extracted cases
        new_docs = []
        skipped = 0
        for doc in docs:
            if storage.cases_exist_for_document(doc["doc_id"]):
                print(f"  Skipping (cases exist): {doc['filename']}")
                skipped += 1
            else:
                new_docs.append(doc)

        total_cases = 0
        for doc in new_docs:
            print(f"  Processing: {doc['filename']}")
            prompt = client.render_prompt(
                "stage1_extract.txt",
                document_text=_truncate(doc["full_text"], 12000),
            )

            response = client.invoke_json(prompt, system=SYSTEM_PROMPT, max_tokens=4096)
            cases = response if isinstance(response, list) else response.get("cases", [response])

            for case_data in cases:
                case_id = hashlib.sha256(
                    f"{doc['filename']}:{case_data.get('case_name', '')}".encode()
                ).hexdigest()[:12]

                case = {
                    "case_id": case_id,
                    "source_doc_id": doc["doc_id"],
                    "case_name": case_data.get("case_name", "Unknown"),
                    "scheme_mechanics": case_data.get("scheme_mechanics", ""),
                    "exploited_policy": case_data.get("exploited_policy", ""),
                    "enabling_condition": case_data.get("enabling_condition", ""),
                    "scale_dollars": _parse_dollars(case_data.get("scale_dollars")),
                    "scale_defendants": case_data.get("scale_defendants"),
                    "scale_duration": case_data.get("scale_duration"),
                    "detection_method": case_data.get("detection_method"),
                    "raw_extraction": case_data,
                }
                storage.insert_case(case)
                total_cases += 1
                print(f"    Extracted: {case['case_name']}")

        storage.log_stage_complete(run_id, 1, {
            "cases_extracted": total_cases,
            "documents_processed": len(new_docs),
            "documents_skipped": skipped,
        })
        print(
            f"  Stage 1 complete: {total_cases} cases from {len(new_docs)} new documents "
            f"({skipped} unchanged, skipped)."
        )

    except Exception as e:
        storage.log_stage_failed(run_id, 1, str(e))
        raise


def load_seed_cases(storage: SVAPStorage, seed_path: str):
    """Load pre-extracted cases from a seed JSON file (bypasses LLM extraction)."""
    with open(seed_path) as f:
        cases = json.load(f)

    for case_data in cases:
        storage.insert_case(case_data)
    print(f"  Loaded {len(cases)} seed cases.")


def _truncate(text: str, max_chars: int) -> str:
    if len(text) <= max_chars:
        return text
    return text[:max_chars] + "\n\n[TRUNCATED — document continues]"


def _parse_dollars(val) -> float | None:
    """Parse dollar amounts from LLM output, tolerating messy text."""
    if val is None:
        return None
    if isinstance(val, (int, float)):
        return float(val)
    text = str(val).lower().replace(",", "").replace("$", "").strip()
    multipliers = {"billion": 1e9, "million": 1e6, "thousand": 1e3}
    for word, mult in multipliers.items():
        if word in text:
            match = re.search(r"[\d.]+", text)
            return float(match.group()) * mult if match else None
    match = re.search(r"[\d.]+", text)
    if not match:
        return None
    return float(match.group())


