#!/usr/bin/env python3
"""
Reset pipeline data.

Modes:
    --list              List all pipeline runs
    --all               Delete all per-run analysis data (keeps global corpus)
    --run-id ID         Delete a specific run
    --prefix PREFIX     Delete runs matching a prefix
    --corpus            Wipe the global corpus (cases, taxonomy, policies,
                        documents, chunks) AND all per-run data. Full reset.

Database connection is resolved automatically via svap.storage.resolve_database_url():
  1. DATABASE_URL environment variable (if set)
  2. Terraform state output `database_url` (from infrastructure/terraform/)

Usage:
    python scripts/reset_runs.py --list
    python scripts/reset_runs.py --all --dry-run
    python scripts/reset_runs.py --all
    python scripts/reset_runs.py --corpus
    python scripts/reset_runs.py --corpus --dry-run
"""

import argparse

import psycopg2
import psycopg2.extras

from svap.storage import SVAPStorage, resolve_database_url

# Per-run tables with run_id column, in deletion order (respects FK deps).
# Junction tables (prediction_qualities, assessment_findings) cascade
# automatically when their parent rows are deleted.
PER_RUN_TABLES = [
    "detection_patterns",
    "predictions",
    "policy_scores",
    "quality_assessments",
    "structural_findings",
    "triage_results",
    "research_sessions",
    "convergence_scores",
    "calibration",
    "stage_log",
    "pipeline_runs",
]

# Global corpus tables, in deletion order
CORPUS_TABLES = [
    "prediction_qualities",
    "assessment_findings",
    "stage_processing_log",
    "source_candidates",
    "chunks",
    "cases",
    "documents",
    "enforcement_sources",
    "source_feeds",
    "dimension_registry",
    "regulatory_sources",
    "taxonomy_case_log",
    "taxonomy",
    "policies",
]


def get_connection():
    url = resolve_database_url()
    # Run schema migrations before raw psycopg2 operations
    storage = SVAPStorage(url)
    storage.close()
    return psycopg2.connect(url)


def list_runs(conn):
    """List all pipeline runs with stage completion info."""
    with conn.cursor(cursor_factory=psycopg2.extras.RealDictCursor) as cur:
        cur.execute("""
            SELECT
                r.run_id,
                r.created_at,
                r.notes,
                COUNT(DISTINCT sl.stage) FILTER (WHERE sl.status = 'completed') AS stages_completed,
                COUNT(DISTINCT sl.stage) FILTER (WHERE sl.status = 'failed') AS stages_failed
            FROM pipeline_runs r
            LEFT JOIN stage_log sl ON sl.run_id = r.run_id
            GROUP BY r.run_id, r.created_at, r.notes
            ORDER BY r.created_at DESC
        """)
        runs = cur.fetchall()

    if not runs:
        print("No pipeline runs found.")
        return

    print(f"{'Run ID':<40} {'Created':<22} {'Done':>4} {'Fail':>4}  Notes")
    print("-" * 100)
    for r in runs:
        print(
            f"{r['run_id']:<40} {r['created_at']!s:<22} "
            f"{r['stages_completed']:>4} {r['stages_failed']:>4}  "
            f"{(r['notes'] or '')[:30]}"
        )
    print(f"\nTotal: {len(runs)} runs")


def find_runs(conn, run_id=None, prefix=None, all_runs=False):
    """Find run_ids matching the criteria."""
    with conn.cursor() as cur:
        if run_id:
            cur.execute("SELECT run_id FROM pipeline_runs WHERE run_id = %s", (run_id,))
        elif prefix:
            cur.execute(
                "SELECT run_id FROM pipeline_runs WHERE run_id LIKE %s ORDER BY created_at",
                (prefix + "%",),
            )
        elif all_runs:
            cur.execute("SELECT run_id FROM pipeline_runs ORDER BY created_at")
        else:
            return []
        return [row[0] for row in cur.fetchall()]


def count_per_run_data(conn, run_ids):
    """Count rows per table for the given run_ids."""
    counts = {}
    with conn.cursor() as cur:
        for table in PER_RUN_TABLES:
            cur.execute(
                f"SELECT COUNT(*) FROM {table} WHERE run_id = ANY(%s)",
                (run_ids,),
            )
            counts[table] = cur.fetchone()[0]
    return counts


def delete_runs(conn, run_ids, dry_run=False):
    """Delete all per-run data for the given run_ids."""
    counts = count_per_run_data(conn, run_ids)

    total = sum(counts.values())
    if total == 0:
        print("No per-run data to delete.")
        return

    print(f"\n{'Table':<30} {'Rows':>8}")
    print("-" * 40)
    for table in PER_RUN_TABLES:
        if counts[table] > 0:
            print(f"  {table:<28} {counts[table]:>8}")
    print(f"  {'TOTAL':<28} {total:>8}")

    if dry_run:
        print("\n[DRY RUN] No data was deleted.")
        return

    with conn.cursor() as cur:
        for table in PER_RUN_TABLES:
            if counts[table] > 0:
                cur.execute(
                    f"DELETE FROM {table} WHERE run_id = ANY(%s)",
                    (run_ids,),
                )
                print(f"  Deleted {cur.rowcount} rows from {table}")
    conn.commit()
    print(f"\nDone. Deleted data for {len(run_ids)} run(s).")


def _count_tables(conn, tables):
    """Count rows per table, tolerating missing tables."""
    counts = {}
    with conn.cursor() as cur:
        for table in tables:
            try:
                cur.execute(f"SELECT COUNT(*) FROM {table}")
                counts[table] = cur.fetchone()[0]
            except Exception:
                conn.rollback()
                counts[table] = 0
    return counts


def _terminate_blocking_connections(conn):
    """Kill other connections on this database that might hold locks."""
    with conn.cursor() as cur:
        cur.execute(
            "SELECT pg_terminate_backend(pid) FROM pg_stat_activity "
            "WHERE pid != pg_backend_pid() AND datname = current_database() "
            "AND state != 'idle'"
        )
        terminated = cur.fetchall()
    if terminated:
        print(f"  Terminated {len(terminated)} blocking connection(s)")


def delete_corpus(conn, dry_run=False):
    """Delete ALL data: per-run tables + global corpus.

    Truncates per-run tables first (they FK into corpus tables),
    then truncates the global corpus.
    """
    all_tables = PER_RUN_TABLES + CORPUS_TABLES
    counts = _count_tables(conn, all_tables)

    total = sum(counts.values())
    if total == 0:
        print("Database is already empty.")
        return

    print(f"\n{'Table':<30} {'Rows':>8}")
    print("-" * 40)
    for table in all_tables:
        if counts[table] > 0:
            print(f"  {table:<28} {counts[table]:>8}")
    print(f"  {'TOTAL':<28} {total:>8}")

    if dry_run:
        print("\n[DRY RUN] No data was deleted.")
        return

    _terminate_blocking_connections(conn)

    with conn.cursor() as cur:
        for table in all_tables:
            if counts[table] > 0:
                cur.execute(f"TRUNCATE {table} CASCADE")
                print(f"  Truncated {table} ({counts[table]} rows)")

    conn.commit()
    print("\nDone. Full corpus reset complete.")


def main():
    parser = argparse.ArgumentParser(
        description="Reset pipeline data. Use --all for runs only, --corpus for everything.",
    )
    group = parser.add_mutually_exclusive_group()
    group.add_argument("--list", action="store_true", help="List all runs")
    group.add_argument("--run-id", help="Delete a specific run by ID")
    group.add_argument("--prefix", help="Delete runs matching a prefix")
    group.add_argument("--all", action="store_true", help="Delete ALL per-run data (keeps corpus)")
    group.add_argument("--corpus", action="store_true", help="Full reset: wipe corpus + all runs")
    parser.add_argument("--dry-run", action="store_true", help="Show what would be deleted")

    args = parser.parse_args()

    if not any([args.list, args.run_id, args.prefix, args.all, args.corpus]):
        parser.print_help()
        return

    conn = get_connection()

    try:
        if args.list:
            list_runs(conn)
            return

        if args.corpus:
            delete_corpus(conn, dry_run=args.dry_run)
            return

        run_ids = find_runs(conn, run_id=args.run_id, prefix=args.prefix, all_runs=args.all)

        if not run_ids:
            print("No matching runs found.")
            return

        print(f"Matching runs ({len(run_ids)}):")
        for rid in run_ids:
            print(f"  {rid}")

        delete_runs(conn, run_ids, dry_run=args.dry_run)

    finally:
        conn.close()


if __name__ == "__main__":
    main()
