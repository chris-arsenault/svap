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
from datetime import UTC, datetime
from pathlib import Path

import yaml

from svap.stages import (
    stage1_case_assembly,
    stage2_taxonomy,
    stage3_scoring,
    stage4_scanning,
    stage5_prediction,
    stage6_detection,
)
from svap.storage import SVAPStorage

STAGES = {
    1: stage1_case_assembly,
    2: stage2_taxonomy,
    3: stage3_scoring,
    4: stage4_scanning,
    5: stage5_prediction,
    6: stage6_detection,
}

SEED_DIR = Path(__file__).parent / "seed"


def load_config(config_path: str = "config.yaml") -> dict:
    with open(config_path) as f:
        return yaml.safe_load(f)


def cmd_run(args, config):
    """Run one or more pipeline stages."""
    storage = SVAPStorage(os.environ.get("DATABASE_URL", config["storage"]["database_url"]))
    from svap.bedrock_client import BedrockClient

    client = BedrockClient(config)

    # Get or create run
    run_id = storage.get_latest_run()
    if not run_id:
        run_id = f"run_{datetime.now(UTC).strftime('%Y%m%d_%H%M%S')}"
        storage.create_run(run_id, config, notes="CLI run")
    print(f"Run ID: {run_id}")

    stages_to_run = [1, 2, 3, 4, 5, 6] if args.stage == "all" else [int(args.stage)]

    human_gates = config.get("pipeline", {}).get("human_gates", [2, 5])

    for stage_num in stages_to_run:
        # Check prerequisites
        if stage_num > 1:
            prev_status = storage.get_stage_status(run_id, stage_num - 1)
            if prev_status not in ("completed", "approved"):
                if prev_status == "pending_review":
                    print(f"\n⚠ Stage {stage_num - 1} is pending human review.")
                    print(
                        f"  Approve it first: python -m svap.orchestrator approve --stage {stage_num - 1}"
                    )
                    break
                elif prev_status is None:
                    print(f"\n⚠ Stage {stage_num - 1} has not been run yet. Run it first.")
                    break
                else:
                    print(f"\n⚠ Stage {stage_num - 1} status is '{prev_status}'. Cannot proceed.")
                    break

        print(f"\n{'=' * 60}")
        STAGES[stage_num].run(storage, client, run_id, config)

        # Check if this stage has a human gate
        stage_status = storage.get_stage_status(run_id, stage_num)
        if stage_status == "pending_review" and stage_num in human_gates and args.stage == "all":
            print(f"\n⏸ Pipeline paused at Stage {stage_num} human review gate.")
            print("  Review outputs, then resume:")
            print(f"    python -m svap.orchestrator approve --stage {stage_num}")
            print(f"    python -m svap.orchestrator run --stage {stage_num + 1}")
            break

    storage.close()


def cmd_seed(args, config):
    """Load example HHS OIG data to replicate the healthcare fraud analysis."""
    storage = SVAPStorage(os.environ.get("DATABASE_URL", config["storage"]["database_url"]))
    result = _seed(storage, config)
    print("\n  Seed data loaded successfully.")
    print(f"    Run ID:             {result['run_id']}")
    print(f"    Cases:              {result['cases']}")
    print(f"    Taxonomy qualities: {result['taxonomy']}")
    print(f"    Policies:           {result['policies']}")
    print(f"    Predictions:        {result['predictions']}")
    print(f"    Detection patterns: {result['detection_patterns']}")
    storage.close()


def _load_seed_json(filename):
    """Load a JSON file from the seed data directory."""
    with open(SEED_DIR / filename) as f:
        return json.load(f)


def _seed_convergence_scores(storage, run_id, cases):
    """Insert convergence scores from case qualities."""
    count = 0
    for case in cases:
        for quality_id in case.get("qualities", []):
            storage.insert_convergence_score(
                run_id, case["case_id"], quality_id, present=True, evidence="Seed data"
            )
            count += 1
    return count


def _seed_policy_scores(storage, run_id, policies):
    """Insert policy scores from policy qualities."""
    count = 0
    for policy in policies:
        for quality_id in policy.get("qualities", []):
            storage.insert_policy_score(
                run_id, policy["policy_id"], quality_id, present=True, evidence="Seed data"
            )
            count += 1
    return count


def _seed(storage, config=None):
    """Run the seed operation, populating ALL database tables.

    Returns a result dict with counts of seeded entities.
    Can be called from the CLI or programmatically (e.g. from the API).
    """
    if config is None:
        config = {}

    run_id = f"seed_{datetime.now(UTC).strftime('%Y%m%d_%H%M%S')}"
    storage.create_run(run_id, config, notes="Seeded with HHS OIG example data")
    print(f"Created run: {run_id}")

    # ── Stage 1: Cases ─────────────────────────────────────────────
    print("\n  Loading seed enforcement cases...")
    cases = _load_seed_json("cases.json")
    for case in cases:
        storage.insert_case(run_id, case)
    print(f"    Loaded {len(cases)} cases.")

    # ── Stage 2: Taxonomy ──────────────────────────────────────────
    print("  Loading seed vulnerability taxonomy...")
    taxonomy = _load_seed_json("taxonomy.json")
    for q in taxonomy:
        q["review_status"] = "approved"
        storage.insert_quality(run_id, q)
    print(f"    Loaded {len(taxonomy)} taxonomy qualities.")

    # ── Stage 3: Convergence scores (case x quality matrix) ────────
    print("  Loading seed convergence scores...")
    convergence_count = _seed_convergence_scores(storage, run_id, cases)
    print(f"    Loaded {convergence_count} convergence scores.")

    # ── Stage 3: Calibration ──────────────────────────────────────
    print("  Loading seed calibration...")
    storage.insert_calibration(
        run_id,
        threshold=3,
        notes="Seed calibration: policies scoring >= 3 qualities correspond to documented exploitation cases",
        freq={},
        combos={},
    )

    # ── Stage 4: Policies ─────────────────────────────────────────
    print("  Loading seed policies...")
    policies = _load_seed_json("policies.json")
    for p in policies:
        storage.insert_policy(run_id, p)
    print(f"    Loaded {len(policies)} policies.")

    # ── Stage 4: Policy scores (policy x quality matrix) ───────────
    print("  Loading seed policy scores...")
    policy_score_count = _seed_policy_scores(storage, run_id, policies)
    print(f"    Loaded {policy_score_count} policy scores.")

    # ── Stage 5: Predictions ──────────────────────────────────────
    print("  Loading seed predictions...")
    predictions = _load_seed_json("predictions.json")
    for pred in predictions:
        storage.insert_prediction(run_id, pred)
    print(f"    Loaded {len(predictions)} predictions.")

    # ── Stage 6: Detection patterns ───────────────────────────────
    print("  Loading seed detection patterns...")
    patterns = _load_seed_json("detection_patterns.json")
    for pattern in patterns:
        storage.insert_detection_pattern(run_id, pattern)
    print(f"    Loaded {len(patterns)} detection patterns.")

    # ── Stage log: mark all 6 stages completed ────────────────────
    _mark_seed_stages_complete(storage, run_id)

    return {
        "run_id": run_id,
        "cases": len(cases),
        "taxonomy": len(taxonomy),
        "policies": len(policies),
        "predictions": len(predictions),
        "detection_patterns": len(patterns),
    }


def _mark_seed_stages_complete(storage, run_id):
    """Mark all 6 stages as completed and approve human gates."""
    print("  Marking all stages as completed...")
    for stage in range(1, 7):
        storage.log_stage_start(run_id, stage)
        storage.log_stage_complete(run_id, stage, {"source": "seed_data"})
    for gate_stage in [2, 5]:
        storage.log_stage_start(run_id, gate_stage)
        storage.log_stage_pending_review(run_id, gate_stage)
        storage.approve_stage(run_id, gate_stage)


def _load_config(config_path: str = "config.yaml") -> dict:
    """Load a YAML config file and return a dict.

    Alias for :func:`load_config` — kept for API import convenience.
    """
    return load_config(config_path)


def _run_stage(storage, run_id: str, stage: int, config: dict):
    """Run a single pipeline stage.

    This is the programmatic entry-point used by the API layer.
    It handles prerequisite checks and human-gate pausing.
    """
    from svap.bedrock_client import BedrockClient

    client = BedrockClient(config)
    human_gates = config.get("pipeline", {}).get("human_gates", [2, 5])

    # Check prerequisites
    if stage > 1:
        prev_status = storage.get_stage_status(run_id, stage - 1)
        if prev_status not in ("completed", "approved"):
            raise RuntimeError(
                f"Stage {stage - 1} status is '{prev_status}'. "
                f"It must be 'completed' or 'approved' before running stage {stage}."
            )

    STAGES[stage].run(storage, client, run_id, config)

    stage_status = storage.get_stage_status(run_id, stage)
    return {
        "run_id": run_id,
        "stage": stage,
        "status": stage_status,
        "needs_approval": stage_status == "pending_review" and stage in human_gates,
    }


def cmd_status(args, config):
    """Display current pipeline status."""
    storage = SVAPStorage(os.environ.get("DATABASE_URL", config["storage"]["database_url"]))
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

    print("\n  Pipeline Status:")
    for stage_num in range(1, 7):
        s = latest.get(stage_num)
        icon = {
            "completed": "✅",
            "approved": "✅",
            "running": "🔄",
            "failed": "❌",
            "pending_review": "⏸",
        }.get(s["status"] if s else "not_started", "⬜")
        status_text = s["status"] if s else "not started"
        print(f"    {icon} Stage {stage_num}: {stage_names[stage_num]} — {status_text}")

    # Summary counts
    cases = storage.get_cases(run_id)
    taxonomy = storage.get_taxonomy(run_id)
    policies = storage.get_policies(run_id)
    predictions = storage.get_predictions(run_id)
    patterns = storage.get_detection_patterns(run_id)

    print("\n  Data Summary:")
    print(f"    Cases:             {len(cases)}")
    print(f"    Taxonomy qualities: {len(taxonomy)}")
    print(f"    Policies scanned:  {len(policies)}")
    print(f"    Predictions:       {len(predictions)}")
    print(f"    Detection patterns: {len(patterns)}")

    storage.close()


def cmd_approve(args, config):
    """Approve a human review gate."""
    storage = SVAPStorage(os.environ.get("DATABASE_URL", config["storage"]["database_url"]))
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
    storage = SVAPStorage(os.environ.get("DATABASE_URL", config["storage"]["database_url"]))
    from svap.rag import DocumentIngester

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
    storage = SVAPStorage(os.environ.get("DATABASE_URL", config["storage"]["database_url"]))
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

    lines = ["# SVAP Analysis Report\n", f"Run: {run_id}\n"]

    if taxonomy:
        lines.append("## Vulnerability Taxonomy\n")
        for q in taxonomy:
            lines.append(f"### {q['quality_id']}: {q['name']}\n")
            lines.append(f"**Definition:** {q['definition']}\n")
            lines.append(f"**Recognition Test:** {q['recognition_test']}\n")
            lines.append(f"**Exploitation Logic:** {q['exploitation_logic']}\n")

    if calibration:
        lines.append("\n## Calibration\n")
        lines.append(f"**Threshold:** {calibration['threshold']}\n")
        lines.append(f"**Notes:** {calibration['correlation_notes']}\n")

    if predictions:
        lines.append("\n## Exploitation Predictions (by priority)\n")
        for pred in predictions:
            lines.append(
                f"### {pred.get('policy_name', 'Unknown')} (score={pred['convergence_score']})\n"
            )
            lines.append(f"**Mechanics:** {pred['mechanics']}\n")
            lines.append(f"**Actor Profile:** {pred.get('actor_profile', 'N/A')}\n")
            lines.append(f"**Lifecycle Stage:** {pred.get('lifecycle_stage', 'N/A')}\n")
            lines.append(f"**Detection Difficulty:** {pred.get('detection_difficulty', 'N/A')}\n")

    if patterns:
        lines.append("\n## Detection Patterns\n")
        for pat in patterns:
            lines.append(
                f"### [{pat.get('priority', 'medium').upper()}] {pat.get('policy_name', '')}\n"
            )
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
    ingest_parser.add_argument(
        "--type",
        default="other",
        help="Document type: enforcement, policy, guidance, report, other",
    )
    ingest_parser.add_argument("--config", default="config.yaml")

    # Export
    export_parser = subparsers.add_parser("export", help="Export results")
    export_parser.add_argument(
        "--format", default="markdown", help="Export format: markdown or json"
    )
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
