"""
SVAP Pipeline Orchestrator

Main entry point for running the Structural Vulnerability Analysis Pipeline.
Handles stage sequencing, human review gates, data seeding, export, and status.

Usage:
    python -m svap.orchestrator run --stage 1          # Run single stage
    python -m svap.orchestrator run --stage all         # Run all stages
    python -m svap.orchestrator seed                    # Load HHS OIG example data
    python -m svap.orchestrator status                  # View pipeline status
    python -m svap.orchestrator approve --stage 2       # Approve human gate
    python -m svap.orchestrator export --format md      # Export results
    python -m svap.orchestrator ingest --path ./docs --type enforcement
"""

import argparse
import json
import os
import sys
from datetime import datetime, timezone
from pathlib import Path
import uuid

import yaml

from svap.storage import SVAPStorage
from svap.bedrock_client import BedrockClient
from svap.rag import DocumentIngester, ContextAssembler
from svap.stages import (
    stage1_case_assembly,
    stage2_taxonomy,
    stage3_scoring,
    stage4_scanning,
    stage5_prediction,
    stage6_detection,
)

STAGES = {
    1: stage1_case_assembly,
    2: stage2_taxonomy,
    3: stage3_scoring,
    4: stage4_scanning,
    5: stage5_prediction,
    6: stage6_detection,
}

EXAMPLES_DIR = Path(__file__).parent / "examples"


def load_config(config_path: str = "config.yaml") -> dict:
    with open(config_path) as f:
        return yaml.safe_load(f)


def cmd_run(args, config):
    """Run one or more pipeline stages."""
    storage = SVAPStorage(config["storage"]["db_path"])
    client = BedrockClient(config)

    # Get or create run
    run_id = storage.get_latest_run()
    if not run_id:
        run_id = f"run_{datetime.now(timezone.utc).strftime('%Y%m%d_%H%M%S')}"
        storage.create_run(run_id, config, notes="CLI run")
    print(f"Run ID: {run_id}")

    if args.stage == "all":
        stages_to_run = [1, 2, 3, 4, 5, 6]
    else:
        stages_to_run = [int(args.stage)]

    human_gates = config.get("pipeline", {}).get("human_gates", [2, 5])

    for stage_num in stages_to_run:
        # Check prerequisites
        if stage_num > 1:
            prev_status = storage.get_stage_status(run_id, stage_num - 1)
            if prev_status not in ("completed", "approved"):
                if prev_status == "pending_review":
                    print(f"\n⚠ Stage {stage_num - 1} is pending human review.")
                    print(f"  Approve it first: python -m svap.orchestrator approve --stage {stage_num - 1}")
                    break
                elif prev_status is None:
                    print(f"\n⚠ Stage {stage_num - 1} has not been run yet. Run it first.")
                    break
                else:
                    print(f"\n⚠ Stage {stage_num - 1} status is '{prev_status}'. Cannot proceed.")
                    break

        print(f"\n{'='*60}")
        STAGES[stage_num].run(storage, client, run_id, config)

        # Check if this stage has a human gate
        stage_status = storage.get_stage_status(run_id, stage_num)
        if stage_status == "pending_review" and stage_num in human_gates:
            if args.stage == "all":
                print(f"\n⏸ Pipeline paused at Stage {stage_num} human review gate.")
                print(f"  Review outputs, then resume:")
                print(f"    python -m svap.orchestrator approve --stage {stage_num}")
                print(f"    python -m svap.orchestrator run --stage {stage_num + 1}")
                break

    storage.close()


def cmd_seed(args, config):
    """Load example HHS OIG data to replicate the healthcare fraud analysis."""
    storage = SVAPStorage(config["storage"]["db_path"])

    run_id = f"seed_{datetime.now(timezone.utc).strftime('%Y%m%d_%H%M%S')}"
    storage.create_run(run_id, config, notes="Seeded with HHS OIG example data")
    print(f"Created run: {run_id}")

    # Load seed cases (Stage 1 output)
    print("\n  Loading seed enforcement cases...")
    stage1_case_assembly.load_seed_cases(
        storage, run_id, str(EXAMPLES_DIR / "seed_cases.json")
    )
    storage.log_stage_start(run_id, 1)
    storage.log_stage_complete(run_id, 1, {"source": "seed_data"})

    # Load seed taxonomy (Stage 2 output)
    print("  Loading seed vulnerability taxonomy...")
    stage2_taxonomy.load_seed_taxonomy(
        storage, run_id, str(EXAMPLES_DIR / "seed_taxonomy.json")
    )
    storage.log_stage_start(run_id, 2)
    storage.log_stage_complete(run_id, 2, {"source": "seed_data"})
    storage.approve_stage(run_id, 2)  # Pre-approved since it's curated seed data

    # Load seed policies (Stage 4 input)
    print("  Loading seed policies for scanning...")
    stage4_scanning.load_seed_policies(
        storage, run_id, str(EXAMPLES_DIR / "seed_policies.json")
    )

    print(f"\n  Seed data loaded successfully.")
    print(f"  To replicate the full analysis, run:")
    print(f"    python -m svap.orchestrator run --stage 3   # Score known cases")
    print(f"    python -m svap.orchestrator run --stage 4   # Scan policies")
    print(f"    python -m svap.orchestrator approve --stage 5")
    print(f"    python -m svap.orchestrator run --stage 6   # Generate detection patterns")

    storage.close()


def cmd_status(args, config):
    """Display current pipeline status."""
    storage = SVAPStorage(config["storage"]["db_path"])
    run_id = storage.get_latest_run()

    if not run_id:
        print("No pipeline runs found. Start with:")
        print("  python -m svap.orchestrator seed        # Load example data")
        print("  python -m svap.orchestrator run --stage 1  # Start fresh")
        storage.close()
        return

    print(f"Run ID: {run_id}")
    status = storage.get_pipeline_status(run_id)

    stage_names = {
        1: "Case Assembly",
        2: "Taxonomy Extraction",
        3: "Convergence Scoring",
        4: "Policy Scanning",
        5: "Exploitation Prediction",
        6: "Detection Patterns",
    }

    # Deduplicate to latest status per stage
    latest = {}
    for s in status:
        stage = s["stage"]
        if stage not in latest:
            latest[stage] = s

    print(f"\n  Pipeline Status:")
    for stage_num in range(1, 7):
        s = latest.get(stage_num)
        icon = {"completed": "✅", "approved": "✅", "running": "🔄",
                "failed": "❌", "pending_review": "⏸"}.get(
            s["status"] if s else "not_started", "⬜"
        )
        status_text = s["status"] if s else "not started"
        print(f"    {icon} Stage {stage_num}: {stage_names[stage_num]} — {status_text}")

    # Summary counts
    cases = storage.get_cases(run_id)
    taxonomy = storage.get_taxonomy(run_id)
    policies = storage.get_policies(run_id)
    predictions = storage.get_predictions(run_id)
    patterns = storage.get_detection_patterns(run_id)

    print(f"\n  Data Summary:")
    print(f"    Cases:             {len(cases)}")
    print(f"    Taxonomy qualities: {len(taxonomy)}")
    print(f"    Policies scanned:  {len(policies)}")
    print(f"    Predictions:       {len(predictions)}")
    print(f"    Detection patterns: {len(patterns)}")

    storage.close()


def cmd_approve(args, config):
    """Approve a human review gate."""
    storage = SVAPStorage(config["storage"]["db_path"])
    run_id = storage.get_latest_run()
    if not run_id:
        print("No pipeline runs found.")
        storage.close()
        return

    stage = int(args.stage)
    current = storage.get_stage_status(run_id, stage)

    if current != "pending_review":
        print(f"Stage {stage} status is '{current}', not 'pending_review'. Nothing to approve.")
        storage.close()
        return

    storage.approve_stage(run_id, stage)
    print(f"✅ Stage {stage} approved. You can now run Stage {stage + 1}:")
    print(f"   python -m svap.orchestrator run --stage {stage + 1}")
    storage.close()


def cmd_ingest(args, config):
    """Ingest documents into the RAG store."""
    storage = SVAPStorage(config["storage"]["db_path"])
    ingester = DocumentIngester(storage, config)

    path = Path(args.path)
    doc_type = args.type or "other"

    if path.is_dir():
        results = ingester.ingest_directory(str(path), doc_type)
        print(f"Ingested {len(results)} files:")
        for r in results:
            print(f"  {r['file']}: {r['chunks']} chunks")
    elif path.is_file():
        doc_id, n_chunks = ingester.ingest_file(str(path), doc_type)
        print(f"Ingested {path.name}: {n_chunks} chunks (doc_id={doc_id})")
    else:
        print(f"Path not found: {path}")

    storage.close()


def cmd_export(args, config):
    """Export pipeline results."""
    storage = SVAPStorage(config["storage"]["db_path"])
    run_id = storage.get_latest_run()
    if not run_id:
        print("No pipeline runs found.")
        storage.close()
        return

    export_dir = Path(args.output or config.get("pipeline", {}).get("export_dir", "./results"))
    export_dir.mkdir(parents=True, exist_ok=True)

    fmt = args.format or "markdown"

    if fmt == "json":
        _export_json(storage, run_id, export_dir)
    else:
        _export_markdown(storage, run_id, export_dir)

    storage.close()


def _export_json(storage, run_id, export_dir):
    """Export all pipeline data as JSON files."""
    data = {
        "cases": storage.get_cases(run_id),
        "taxonomy": storage.get_taxonomy(run_id),
        "convergence_matrix": storage.get_convergence_matrix(run_id),
        "calibration": storage.get_calibration(run_id),
        "policies": storage.get_policies(run_id),
        "policy_scores": storage.get_policy_scores(run_id),
        "predictions": storage.get_predictions(run_id),
        "detection_patterns": storage.get_detection_patterns(run_id),
    }
    out_path = export_dir / f"svap_export_{run_id}.json"
    with open(out_path, "w") as f:
        json.dump(data, f, indent=2, default=str)
    print(f"Exported to {out_path}")


def _export_markdown(storage, run_id, export_dir):
    """Export results as a readable Markdown report."""
    taxonomy = storage.get_taxonomy(run_id)
    predictions = storage.get_predictions(run_id)
    patterns = storage.get_detection_patterns(run_id)
    calibration = storage.get_calibration(run_id)

    lines = [f"# SVAP Analysis Report\n", f"Run: {run_id}\n"]

    if taxonomy:
        lines.append("## Vulnerability Taxonomy\n")
        for q in taxonomy:
            lines.append(f"### {q['quality_id']}: {q['name']}\n")
            lines.append(f"**Definition:** {q['definition']}\n")
            lines.append(f"**Recognition Test:** {q['recognition_test']}\n")
            lines.append(f"**Exploitation Logic:** {q['exploitation_logic']}\n")

    if calibration:
        lines.append(f"\n## Calibration\n")
        lines.append(f"**Threshold:** {calibration['threshold']}\n")
        lines.append(f"**Notes:** {calibration['correlation_notes']}\n")

    if predictions:
        lines.append("\n## Exploitation Predictions (by priority)\n")
        for pred in predictions:
            lines.append(f"### {pred.get('policy_name', 'Unknown')} (score={pred['convergence_score']})\n")
            lines.append(f"**Mechanics:** {pred['mechanics']}\n")
            lines.append(f"**Actor Profile:** {pred.get('actor_profile', 'N/A')}\n")
            lines.append(f"**Lifecycle Stage:** {pred.get('lifecycle_stage', 'N/A')}\n")
            lines.append(f"**Detection Difficulty:** {pred.get('detection_difficulty', 'N/A')}\n")

    if patterns:
        lines.append("\n## Detection Patterns\n")
        for pat in patterns:
            lines.append(f"### [{pat.get('priority', 'medium').upper()}] {pat.get('policy_name', '')}\n")
            lines.append(f"**Data Source:** {pat['data_source']}\n")
            lines.append(f"**Anomaly Signal:** {pat['anomaly_signal']}\n")
            lines.append(f"**Baseline:** {pat.get('baseline', 'N/A')}\n")
            lines.append(f"**False Positive Risk:** {pat.get('false_positive_risk', 'N/A')}\n")
            lines.append(f"**Detection Latency:** {pat.get('detection_latency', 'N/A')}\n")

    out_path = export_dir / f"svap_report_{run_id}.md"
    with open(out_path, "w") as f:
        f.write("\n".join(lines))
    print(f"Exported to {out_path}")


def main():
    parser = argparse.ArgumentParser(description="SVAP Pipeline Orchestrator")
    subparsers = parser.add_subparsers(dest="command")

    # Run
    run_parser = subparsers.add_parser("run", help="Run pipeline stages")
    run_parser.add_argument("--stage", required=True, help="Stage number (1-6) or 'all'")
    run_parser.add_argument("--config", default="config.yaml")

    # Seed
    seed_parser = subparsers.add_parser("seed", help="Load example HHS OIG data")
    seed_parser.add_argument("--config", default="config.yaml")

    # Status
    status_parser = subparsers.add_parser("status", help="View pipeline status")
    status_parser.add_argument("--config", default="config.yaml")

    # Approve
    approve_parser = subparsers.add_parser("approve", help="Approve human review gate")
    approve_parser.add_argument("--stage", required=True, help="Stage to approve")
    approve_parser.add_argument("--config", default="config.yaml")

    # Ingest
    ingest_parser = subparsers.add_parser("ingest", help="Ingest documents")
    ingest_parser.add_argument("--path", required=True, help="File or directory to ingest")
    ingest_parser.add_argument("--type", default="other", help="Document type: enforcement, policy, guidance, report, other")
    ingest_parser.add_argument("--config", default="config.yaml")

    # Export
    export_parser = subparsers.add_parser("export", help="Export results")
    export_parser.add_argument("--format", default="markdown", help="Export format: markdown or json")
    export_parser.add_argument("--output", default=None, help="Output directory")
    export_parser.add_argument("--stage", default=None, help="Export specific stage")
    export_parser.add_argument("--config", default="config.yaml")

    args = parser.parse_args()

    if not args.command:
        parser.print_help()
        return

    config_path = getattr(args, "config", "config.yaml")
    if not os.path.exists(config_path):
        print(f"Config file not found: {config_path}")
        print("Copy config.yaml.example to config.yaml and edit it.")
        return

    config = load_config(config_path)

    commands = {
        "run": cmd_run,
        "seed": cmd_seed,
        "status": cmd_status,
        "approve": cmd_approve,
        "ingest": cmd_ingest,
        "export": cmd_export,
    }

    commands[args.command](args, config)


if __name__ == "__main__":
    main()
