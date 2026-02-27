"""
Stage 4B: Deep Structural Research

Per-policy investigation using authoritative regulatory sources (eCFR, Federal Register).
For each policy selected by triage, the agent:
1. Plans which regulatory sources to consult
2. Fetches regulatory text from eCFR and Federal Register APIs
3. Extracts discrete structural findings from each source

Findings are stored in the structural_findings table, tagged to dimensions
in the dimension_registry.

Input:  triage_results (top N policies), dimension_registry
Output: structural_findings, research_sessions
"""

import hashlib
import time

from svap.bedrock_client import BedrockClient
from svap.regulatory.ecfr_client import ECFRClient, parse_xml_sections
from svap.regulatory.federal_register_client import FederalRegisterClient
from svap.regulatory.source_router import get_sources_for_policy
from svap.storage import SVAPStorage

RESEARCH_PLAN_SYSTEM = (
    "You are a regulatory research planner. You identify which specific sections "
    "of the Code of Federal Regulations and Federal Register documents should be "
    "consulted to understand a policy's structural properties."
)

FINDING_EXTRACTION_SYSTEM = (
    "You are extracting factual structural observations from regulatory text. "
    "Each finding must be a single atomic observation about how a policy works "
    "mechanically — not evaluative, not speculative, and cited to the source."
)


def run(
    storage: SVAPStorage,
    client: BedrockClient,
    run_id: str,
    config: dict,
    policy_ids: list[str] | None = None,
):
    """Execute Pass 2: Deep structural research for top-priority policies."""
    print("Stage 4B: Deep Structural Research")
    storage.log_stage_start(run_id, 41)  # 41 = stage 4b

    try:
        storage.seed_dimensions_if_empty()
        dimensions = storage.get_dimensions()
        top_n = config.get("research", {}).get("top_n", 10)

        # Determine which policies to research
        if policy_ids:
            policies_to_research = [
                {"policy_id": pid} for pid in policy_ids
            ]
        else:
            triage = storage.get_triage_results(run_id)
            policies_to_research = triage[:top_n]

        if not policies_to_research:
            print("  No policies to research. Run triage first.")
            storage.log_stage_complete(run_id, 41, {"policies_researched": 0})
            return

        ecfr = ECFRClient()
        fr = FederalRegisterClient()
        researched = 0

        for entry in policies_to_research:
            ok = _research_one_policy(
                storage, client, ecfr, fr, run_id, entry, dimensions
            )
            if ok:
                researched += 1

        storage.log_stage_complete(run_id, 41, {"policies_researched": researched})
        print(f"  Research complete: {researched} policies.")

    except Exception as e:
        storage.log_stage_failed(run_id, 41, str(e))
        raise


def _research_one_policy(storage, client, ecfr, fr, run_id, entry, dimensions):
    """Research a single policy: plan, fetch sources, extract findings."""
    policy_id = entry["policy_id"]
    policies = storage.get_policies()
    policy = next((p for p in policies if p["policy_id"] == policy_id), None)
    if not policy:
        print(f"  Policy {policy_id} not found, skipping")
        return False

    policy_name = policy["name"]
    print(f"  Researching: {policy_name}")

    session_id = hashlib.sha256(f"{run_id}:{policy_id}".encode()).hexdigest()[:12]
    storage.create_research_session(run_id, policy_id, session_id)
    storage.update_research_session(session_id, "researching")
    storage.update_policy_lifecycle(policy_id, "research_in_progress")

    try:
        research_plan = _plan_research(client, policy, dimensions)
        sources_queried = []

        for ecfr_ref in research_plan.get("ecfr_queries", []):
            findings = _research_ecfr_source(ecfr, client, policy, ecfr_ref, dimensions, storage)
            for finding in findings:
                finding["policy_id"] = policy_id
                storage.insert_structural_finding(run_id, finding)
            sources_queried.append({
                "type": "ecfr",
                "title": ecfr_ref.get("title"),
                "part": ecfr_ref.get("part"),
            })
            time.sleep(0.5)

        for fr_search in research_plan.get("fr_searches", [])[:3]:
            findings = _research_fr_source(fr, client, policy, fr_search, dimensions, storage)
            for finding in findings:
                finding["policy_id"] = policy_id
                storage.insert_structural_finding(run_id, finding)
            sources_queried.append({
                "type": "federal_register",
                "term": fr_search.get("term"),
            })
            time.sleep(0.5)

        storage.update_research_session(
            session_id, "findings_complete", sources_queried=sources_queried
        )
        storage.update_policy_lifecycle(policy_id, "structurally_characterized")

        findings_count = len(storage.get_structural_findings(run_id, policy_id))
        print(f"    Complete: {findings_count} findings extracted")
        return True

    except Exception as e:
        print(f"    Research failed for {policy_name}: {e}")
        storage.update_research_session(session_id, "failed", error=str(e))
        return False


def _plan_research(
    client: BedrockClient, policy: dict, dimensions: list[dict]
) -> dict:
    """Ask the LLM to plan which regulatory sources to consult."""
    policy_name = policy["name"]
    policy_desc = policy.get("description") or ""

    # Get known CFR references from the source router
    known_sources = get_sources_for_policy(policy_name, policy_desc)
    known_refs = ""
    if known_sources["ecfr"]:
        known_refs = "\n".join(
            f"- Title {r['title']}, Part {r['part']}" + (f", Subpart {r['subpart']}" if r.get("subpart") else "")
            for r in known_sources["ecfr"]
        )
    else:
        known_refs = "None pre-mapped. Use your knowledge of federal healthcare regulations."

    dimensions_text = "\n".join(
        f"- {d['dimension_id']}: {d['name']} — {d['definition'][:100]}"
        for d in dimensions
    )

    prompt = client.render_prompt(
        "stage4b_plan_research.txt",
        policy_name=policy_name,
        policy_description=policy_desc,
        dimensions=dimensions_text,
        known_cfr_references=known_refs,
    )

    result = client.invoke_json(prompt, system=RESEARCH_PLAN_SYSTEM, max_tokens=2000)

    # Merge pre-mapped references with LLM-suggested ones
    ecfr_queries = result.get("ecfr_queries", [])
    if known_sources["ecfr"]:
        existing_parts = {(q.get("title"), q.get("part")) for q in ecfr_queries}
        for ref in known_sources["ecfr"]:
            if (ref["title"], ref["part"]) not in existing_parts:
                ecfr_queries.append({
                    "title": ref["title"],
                    "part": ref["part"],
                    "subpart": ref.get("subpart"),
                    "rationale": "Pre-mapped from policy catalog",
                })

    result["ecfr_queries"] = ecfr_queries
    return result


def _research_ecfr_source(
    ecfr: ECFRClient,
    client: BedrockClient,
    policy: dict,
    ecfr_ref: dict,
    dimensions: list[dict],
    storage: SVAPStorage,
) -> list[dict]:
    """Fetch a CFR part from eCFR and extract structural findings."""
    title = ecfr_ref.get("title", 42)
    part = ecfr_ref.get("part")
    if not part:
        return []

    source_id = f"ecfr_t{title}_p{part}"
    cached = storage.get_regulatory_source(source_id)

    if cached:
        xml_text = cached["full_text"]
    else:
        try:
            xml_text = ecfr.get_full_text(title=title, part=part)
            storage.insert_regulatory_source({
                "source_id": source_id,
                "source_type": "ecfr",
                "url": f"https://www.ecfr.gov/current/title-{title}/part-{part}",
                "title": f"Title {title} Part {part}",
                "cfr_reference": f"{title} CFR Part {part}",
                "full_text": xml_text,
            })
        except Exception as e:
            print(f"    Failed to fetch eCFR title {title} part {part}: {e}")
            return []

    sections = parse_xml_sections(xml_text)
    if not sections:
        return []

    print(f"    Processing eCFR {title} CFR Part {part}: {len(sections)} sections")

    dimensions_text = "\n".join(
        f"- {d['dimension_id']}: {d['name']} — {d['definition'][:100]}"
        for d in dimensions
    )

    all_findings = []
    for section in sections:
        # Skip very short sections
        if len(section["text"].strip()) < 100:
            continue

        cfr_ref = section.get("cfr_reference") or f"{title} CFR Part {part}"
        source_citation = f"{cfr_ref}"
        if section.get("heading"):
            source_citation += f" — {section['heading']}"

        prompt = client.render_prompt(
            "stage4b_extract_findings.txt",
            policy_name=policy["name"],
            source_citation=source_citation,
            dimensions_text=dimensions_text,
            source_text=section["text"][:4000],
        )

        result = client.invoke_json(
            prompt, system=FINDING_EXTRACTION_SYSTEM, temperature=0.1, max_tokens=2000
        )

        for f in result.get("findings", []):
            finding_id = hashlib.sha256(
                f"{policy['policy_id']}:{f.get('dimension_id', '')}:{f.get('source_citation', '')}".encode()
            ).hexdigest()[:12]
            all_findings.append({
                "finding_id": finding_id,
                "dimension_id": f.get("dimension_id"),
                "observation": f.get("observation", ""),
                "source_type": "ecfr",
                "source_citation": f.get("source_citation", source_citation),
                "source_text": f.get("source_text_excerpt", ""),
                "confidence": f.get("confidence", "medium"),
                "created_by": "stage4b_research",
            })

        time.sleep(0.3)  # rate limit

    return all_findings


def _research_fr_source(
    fr: FederalRegisterClient,
    client: BedrockClient,
    policy: dict,
    fr_search: dict,
    dimensions: list[dict],
    storage: SVAPStorage,
) -> list[dict]:
    """Search Federal Register and extract findings from top results."""
    term = fr_search.get("term", "")
    if not term:
        return []

    try:
        results = fr.search_documents(
            term=term,
            agency_ids=["centers-for-medicare-medicaid-services"],
            doc_type="RULE",
            per_page=3,
        )
    except Exception as e:
        print(f"    Failed to search Federal Register for '{term}': {e}")
        return []

    documents = results.get("results", [])
    if not documents:
        return []

    dimensions_text = "\n".join(
        f"- {d['dimension_id']}: {d['name']} — {d['definition'][:100]}"
        for d in dimensions
    )

    all_findings = []
    for doc in documents[:2]:  # process top 2 results
        doc_number = doc.get("document_number", "")
        raw_text_url = doc.get("raw_text_url")
        if not raw_text_url:
            continue

        source_id = f"fr_{doc_number}"
        cached = storage.get_regulatory_source(source_id)

        if cached:
            text = cached["full_text"]
        else:
            try:
                text = fr.get_document_text(raw_text_url)
                storage.insert_regulatory_source({
                    "source_id": source_id,
                    "source_type": "federal_register",
                    "url": doc.get("html_url", raw_text_url),
                    "title": doc.get("title", ""),
                    "full_text": text,
                    "metadata": {
                        "document_number": doc_number,
                        "publication_date": doc.get("publication_date"),
                        "type": doc.get("type"),
                    },
                })
            except Exception as e:
                print(f"    Failed to fetch FR doc {doc_number}: {e}")
                continue

        # Extract findings from the preamble (first 8000 chars)
        source_citation = f"Federal Register {doc_number}: {doc.get('title', '')[:80]}"
        prompt = client.render_prompt(
            "stage4b_extract_findings.txt",
            policy_name=policy["name"],
            source_citation=source_citation,
            dimensions_text=dimensions_text,
            source_text=text[:8000],
        )

        result = client.invoke_json(
            prompt, system=FINDING_EXTRACTION_SYSTEM, temperature=0.1, max_tokens=2000
        )

        for f in result.get("findings", []):
            finding_id = hashlib.sha256(
                f"{policy['policy_id']}:fr:{doc_number}:{f.get('dimension_id', '')}".encode()
            ).hexdigest()[:12]
            all_findings.append({
                "finding_id": finding_id,
                "dimension_id": f.get("dimension_id"),
                "observation": f.get("observation", ""),
                "source_type": "federal_register",
                "source_citation": f.get("source_citation", source_citation),
                "source_text": f.get("source_text_excerpt", ""),
                "confidence": f.get("confidence", "medium"),
                "created_by": "stage4b_research",
            })

        time.sleep(0.5)

    return all_findings
