"""
HHS reference data and dashboard assembly.

This module provides:
- Static HHS reference data (policy catalog, enforcement sources, data sources)
- Dashboard data assembly function that queries storage and enriches responses
- Helper functions for policy context and data source lookups

This replaces the missing hhs_extension.py that server.py imported.
"""

import json
from pathlib import Path

from svap.api_schemas import (
    build_case_convergence,
    build_policy_convergence,
    enrich_cases,
    enrich_policies,
    enrich_predictions,
    enrich_taxonomy,
)

# Load static reference data from seed JSON files
_SEED_DIR = Path(__file__).parent / "seed"


def _load_json(filename):
    with open(_SEED_DIR / filename) as f:
        return json.load(f)


# These are loaded once at import time
_catalog_data = _load_json("policy_catalog.json")
HHS_POLICY_CATALOG = _catalog_data.get("policy_catalog", _catalog_data)
SCANNED_PROGRAMS = _catalog_data.get("scanned_programs", [])
ENFORCEMENT_SOURCES = _load_json("enforcement_sources.json")
HHS_DATA_SOURCES = _load_json("data_sources.json")


def get_dashboard_data(storage, run_id: str) -> dict:
    """
    Assemble the full dashboard payload for the React UI.

    All pipeline data is global/cumulative (iterative corpus model).
    run_id is only used for pipeline_status (per-run execution state).
    """
    # All pipeline data is global/cumulative — no run_id scoping
    cases = storage.get_cases()
    taxonomy = storage.get_taxonomy()
    policies = storage.get_policies()
    pipeline_status = storage.get_pipeline_status(run_id)  # genuinely per-run

    convergence_matrix = storage.get_convergence_matrix()
    policy_scores = storage.get_policy_scores()
    calibration = storage.get_calibration()
    predictions = storage.get_predictions()
    patterns = storage.get_detection_patterns()

    # Enrich with computed fields
    enriched_cases = enrich_cases(cases, convergence_matrix)
    enriched_taxonomy = enrich_taxonomy(taxonomy, convergence_matrix)
    enriched_policies = enrich_policies(policies, policy_scores, calibration)
    enriched_predictions = enrich_predictions(predictions)

    return {
        "run_id": run_id,
        "source": "api",
        "pipeline_status": pipeline_status,
        "counts": {
            "cases": len(cases),
            "taxonomy_qualities": len(taxonomy),
            "policies": len(policies),
            "predictions": len(predictions),
            "detection_patterns": len(patterns),
        },
        "calibration": calibration or {"threshold": 3},
        "cases": enriched_cases,
        "taxonomy": enriched_taxonomy,
        "policies": enriched_policies,
        "predictions": enriched_predictions,
        "detection_patterns": patterns,
        "case_convergence": build_case_convergence(cases, convergence_matrix),
        "policy_convergence": build_policy_convergence(enriched_policies),
        "policy_catalog": HHS_POLICY_CATALOG,
        "enforcement_sources": storage.get_enforcement_sources(),
        "data_sources": HHS_DATA_SOURCES,
        "scanned_programs": SCANNED_PROGRAMS,
    }


def flatten_policy_catalog() -> list[dict]:
    """Recursively flatten the policy catalog tree into a flat list of programs."""
    results: list[dict] = []
    _walk_catalog(HHS_POLICY_CATALOG, "", results)
    return results


def _walk_catalog(node, path: str, results: list[dict]):
    """Recursively walk a catalog node, appending flattened entries to results."""
    if not isinstance(node, dict):
        return
    for key, value in node.items():
        current_path = f"{path} > {key}" if path else key
        if isinstance(value, dict):
            _walk_catalog_dict(value, current_path, results)
        elif isinstance(value, list):
            _walk_catalog_list(value, current_path, results)


def _walk_catalog_dict(value: dict, path: str, results: list[dict]):
    """Handle a dict node — either a leaf with 'programs' or a subtree."""
    if "programs" in value:
        for program in value["programs"]:
            name = program if isinstance(program, str) else program.get("name", "")
            results.append({"category": path, "name": name})
    else:
        _walk_catalog(value, path, results)


def _walk_catalog_list(value: list, path: str, results: list[dict]):
    """Handle a list node — strings become entries, dicts get walked."""
    for item in value:
        if isinstance(item, str):
            results.append({"category": path, "name": item})
        elif isinstance(item, dict):
            _walk_catalog(item, path, results)


def get_policy_context(policy_name: str) -> dict:
    """Return HHS context information for a policy by name."""
    # Search the catalog for a matching policy
    flat = flatten_policy_catalog()
    for entry in flat:
        if policy_name.lower() in entry["name"].lower():
            return {
                "category": entry["category"],
                "name": entry["name"],
                "scanned": entry["name"] in SCANNED_PROGRAMS,
            }
    return {"category": "Unknown", "name": policy_name, "scanned": False}


def get_data_sources_for_policy(policy_name: str) -> list[dict]:
    """Return relevant HHS data sources for a policy based on keyword matching."""
    name_lower = policy_name.lower()
    results = []

    # Keyword mapping: policy name keywords -> data source categories
    keyword_map = {
        "medicare": ["claims", "enrollment", "provider", "financial"],
        "medicaid": ["claims", "enrollment", "provider"],
        "hcbs": ["claims", "enrollment", "program_integrity"],
        "hospice": ["claims", "enrollment", "program_integrity"],
        "home health": ["claims", "program_integrity"],
        "hospital": ["claims", "financial"],
        "drug": ["claims", "financial"],
        "340b": ["financial"],
        "pace": ["claims", "enrollment"],
        "aco": ["claims", "enrollment", "financial"],
        "marketplace": ["enrollment"],
        "telehealth": ["claims", "provider"],
    }

    relevant_categories = set()
    for keyword, categories in keyword_map.items():
        if keyword in name_lower:
            relevant_categories.update(categories)

    if not relevant_categories:
        relevant_categories = {"claims", "provider"}  # default

    for category_key in relevant_categories:
        category_data = HHS_DATA_SOURCES.get(category_key)
        if category_data and "sources" in category_data:
            results.extend(category_data["sources"])

    return results
