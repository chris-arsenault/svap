"""
Stage 6: Detection Pattern Generation

Translates exploitation predictions into operationally actionable detection
patterns: what to look for in data, which data sources to query, what
constitutes anomalous vs. normal behavior, and how quickly detection is possible.

This is the most implementation-specific stage. The generic patterns generated
here should be refined by data engineers with access to actual system schemas.

Input:  Approved predictions (Stage 5) + optional data source catalog
Output: Detection patterns in the `detection_patterns` table
"""

import json
import hashlib
from svap.storage import SVAPStorage
from svap.bedrock_client import BedrockClient
from svap.rag import ContextAssembler


SYSTEM_PROMPT = """You are a fraud detection analyst designing monitoring rules. You translate 
predicted exploitation mechanics into specific, queryable anomaly signals. Every pattern 
must specify: what data source to query, what specific measurable condition to test, what 
the normal baseline looks like, what false positives to expect, and how quickly the signal 
becomes visible after exploitation begins.

Be concrete. "Monitor for unusual billing patterns" is useless. "Flag providers billing 
>16 hours/day of personal care services, where normal P95 is 10 hours/day" is actionable."""


def run(storage: SVAPStorage, client: BedrockClient, run_id: str, config: dict):
    """Execute Stage 6: Generate detection patterns for approved predictions."""
    print("Stage 6: Detection Pattern Generation")
    storage.log_stage_start(run_id, 6)

    try:
        # Verify Stage 5 approved
        stage5_status = storage.get_stage_status(run_id, 5)
        if stage5_status not in ("approved", "completed"):
            raise ValueError(
                f"Stage 5 status is '{stage5_status}'. Predictions must be approved first."
            )

        predictions = storage.get_predictions(run_id)
        if not predictions:
            raise ValueError("No predictions found. Run Stage 5 first.")

        ctx = ContextAssembler(storage, config)

        # Load data source catalog if available from RAG
        data_sources_context = ctx.retrieve("data sources claims enrollment provider", doc_type="other")
        if not data_sources_context:
            data_sources_context = _default_data_sources()

        total_patterns = 0

        for pred in predictions:
            print(f"    Generating patterns for: {pred['policy_name']} — {pred['mechanics'][:80]}...")

            prompt = client.render_prompt(
                "stage6_detect.txt",
                policy_name=pred["policy_name"],
                prediction_mechanics=pred["mechanics"],
                enabling_qualities=pred["enabling_qualities"],
                actor_profile=pred.get("actor_profile", "Unknown"),
                detection_difficulty=pred.get("detection_difficulty", "Unknown"),
                data_sources=data_sources_context,
            )

            result = client.invoke_json(
                prompt, system=SYSTEM_PROMPT, max_tokens=4096
            )

            patterns = result if isinstance(result, list) else result.get("patterns", [result])

            for i, pat_data in enumerate(patterns):
                pat_id = hashlib.sha256(
                    f"{pred['prediction_id']}:{i}:{pat_data.get('anomaly_signal', '')[:50]}".encode()
                ).hexdigest()[:12]

                pattern = {
                    "pattern_id": pat_id,
                    "prediction_id": pred["prediction_id"],
                    "data_source": pat_data.get("data_source", ""),
                    "anomaly_signal": pat_data.get("anomaly_signal", ""),
                    "baseline": pat_data.get("baseline", ""),
                    "false_positive_risk": pat_data.get("false_positive_risk", ""),
                    "detection_latency": pat_data.get("detection_latency", ""),
                    "priority": pat_data.get("priority", "medium"),
                    "implementation_notes": pat_data.get("implementation_notes", ""),
                }
                storage.insert_detection_pattern(run_id, pattern)
                total_patterns += 1

        storage.log_stage_complete(run_id, 6, {"patterns_generated": total_patterns})

        # ── Summary Report ──────────────────────────────────────────
        all_patterns = storage.get_detection_patterns(run_id)
        print(f"\n  Stage 6 complete: {total_patterns} detection patterns generated.")
        print(f"\n  Detection Pattern Summary:")

        by_priority = {"critical": [], "high": [], "medium": [], "low": []}
        for p in all_patterns:
            by_priority.get(p.get("priority", "medium"), by_priority["medium"]).append(p)

        for priority in ["critical", "high", "medium", "low"]:
            if by_priority[priority]:
                print(f"\n    [{priority.upper()}]")
                for p in by_priority[priority]:
                    print(f"      • {p['policy_name']}: {p['anomaly_signal'][:100]}")
                    print(f"        Data source: {p['data_source']}")

        print(f"\n  Export full results: python -m svap.orchestrator export")

    except Exception as e:
        storage.log_stage_failed(run_id, 6, str(e))
        raise


def _default_data_sources():
    """Default data source catalog for HHS context. Replace with your actual catalog."""
    return """Available data sources (replace with your actual data catalog):
- Claims Database: Medicare FFS claims (Part A, B, D), including procedure codes, 
  diagnosis codes, provider NPIs, beneficiary IDs, dates, amounts
- Enrollment Database: Medicare/Medicaid beneficiary enrollment, plan selections, 
  demographics, eligibility status
- Provider Enrollment: NPI registry, provider enrollment dates, specialty codes, 
  practice locations, ownership information
- MA Encounter Data: Medicare Advantage plan encounter submissions, risk adjustment 
  codes, plan identifiers
- EVV Data: Electronic Visit Verification records (Medicaid HCBS), GPS coordinates, 
  check-in/check-out times
- Marketplace Enrollment: ACA marketplace applications, APTC amounts, broker IDs, 
  plan selections, income attestations
- Exclusions Database: OIG exclusion list, state exclusion actions, CMS revocations
- Financial Data: Provider payment amounts, beneficiary cost-sharing, plan bid data"""
