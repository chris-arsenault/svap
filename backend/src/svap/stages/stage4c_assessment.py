"""
Stage 4C: Quality Assessment from Structural Findings

For each policy with completed structural research, evaluates every taxonomy
quality against the accumulated structural findings. Each assessment cites
specific findings as evidence.

Also syncs results to the legacy policy_scores table for backward compatibility
with stages 5 and 6.

Input:  structural_findings, taxonomy, research_sessions
Output: quality_assessments, policy_scores (backward compat)
"""

import hashlib

from svap.bedrock_client import BedrockClient
from svap.storage import SVAPStorage

ASSESSMENT_SYSTEM = (
    "You are assessing whether a structural vulnerability quality is present in a "
    "policy based on specific, cited structural findings. Be conservative — a quality "
    "is present only if findings directly support it. Cite specific finding IDs."
)


def run(storage: SVAPStorage, client: BedrockClient, run_id: str, config: dict):
    """Execute quality assessment against accumulated structural findings."""
    print("Stage 4C: Quality Assessment from Findings")
    storage.log_stage_start(run_id, 42)  # 42 = stage 4c

    try:
        taxonomy = storage.get_approved_taxonomy()
        if not taxonomy:
            print("  No taxonomy found. Run Stage 2 first.")
            storage.log_stage_complete(run_id, 42, {"policies_assessed": 0})
            return

        # Get policies with completed research
        sessions = storage.get_research_sessions(run_id, status="findings_complete")
        if not sessions:
            print("  No policies with completed research. Run Stage 4B first.")
            storage.log_stage_complete(run_id, 42, {"policies_assessed": 0})
            return

        assessed = 0
        for session in sessions:
            policy_id = session["policy_id"]
            findings = storage.get_structural_findings(run_id, policy_id)

            if not findings:
                print(f"  No findings for policy {policy_id}, skipping")
                continue

            policies = storage.get_policies()
            policy = next((p for p in policies if p["policy_id"] == policy_id), None)
            policy_name = policy["name"] if policy else policy_id

            print(f"  Assessing: {policy_name} ({len(findings)} findings)")

            findings_text = _format_findings(findings)

            for quality in taxonomy:
                assessment = _assess_quality(
                    client, quality, findings_text, findings, policy_id, policy_name, run_id
                )
                storage.upsert_quality_assessment(run_id, assessment)

                # Backward compat: sync to policy_scores
                present_bool = assessment["present"] == "yes"
                storage.insert_policy_score(
                    run_id, policy_id, quality["quality_id"],
                    present=present_bool,
                    evidence=assessment.get("rationale", ""),
                )

            storage.update_research_session(session["session_id"], "assessment_complete")
            storage.update_policy_lifecycle(policy_id, "fully_assessed")
            assessed += 1
            print(f"    Assessed {len(taxonomy)} qualities")

        storage.log_stage_complete(run_id, 42, {
            "policies_assessed": assessed,
            "qualities_per_policy": len(taxonomy),
        })
        print(f"  Assessment complete: {assessed} policies.")

    except Exception as e:
        storage.log_stage_failed(run_id, 42, str(e))
        raise


def _format_findings(findings: list[dict]) -> str:
    """Format findings into a readable text block for the LLM."""
    parts = []
    for f in findings:
        dim_name = f.get("dimension_name") or f.get("dimension_id") or "Unknown"
        parts.append(
            f"[{f['finding_id']}] ({dim_name}, {f.get('confidence', 'medium')} confidence)\n"
            f"  {f['observation']}\n"
            f"  Source: {f.get('source_citation', 'N/A')}"
        )
    return "\n\n".join(parts)


def _assess_quality(
    client: BedrockClient,
    quality: dict,
    findings_text: str,
    findings: list[dict],
    policy_id: str,
    policy_name: str,
    run_id: str,
) -> dict:
    """Assess one quality against the accumulated findings."""
    prompt = client.render_prompt(
        "stage4c_assess_quality.txt",
        quality_id=quality["quality_id"],
        quality_name=quality["name"],
        quality_definition=quality["definition"],
        quality_recognition_test=quality["recognition_test"],
        policy_name=policy_name,
        findings_text=findings_text,
    )

    result = client.invoke_json(prompt, system=ASSESSMENT_SYSTEM, temperature=0.1, max_tokens=1000)

    assessment_id = hashlib.sha256(
        f"{run_id}:{policy_id}:{quality['quality_id']}".encode()
    ).hexdigest()[:12]

    # Validate finding_ids against actual findings
    cited_ids = result.get("finding_ids", [])
    valid_ids = {f["finding_id"] for f in findings}
    validated_ids = [fid for fid in cited_ids if fid in valid_ids]

    return {
        "assessment_id": assessment_id,
        "policy_id": policy_id,
        "quality_id": quality["quality_id"],
        "taxonomy_version": run_id,
        "present": result.get("present", "uncertain"),
        "evidence_finding_ids": validated_ids,
        "confidence": result.get("confidence", "medium"),
        "rationale": result.get("reasoning", ""),
    }
