"""
Routes policy names to relevant regulatory sources (CFR titles/parts, FR search terms).

Loads the mapping from seed/policy_cfr_map.json. Given a policy name or description,
returns the eCFR references and Federal Register search terms to consult.
"""

import json
from pathlib import Path

_POLICY_CFR_MAP: dict | None = None


def _load_map() -> dict:
    global _POLICY_CFR_MAP
    if _POLICY_CFR_MAP is None:
        seed_path = Path(__file__).parent.parent / "seed" / "policy_cfr_map.json"
        with open(seed_path) as f:
            _POLICY_CFR_MAP = json.load(f)
    return _POLICY_CFR_MAP


def get_sources_for_policy(policy_name: str, policy_description: str = "") -> dict:
    """Return regulatory source references for a given policy.

    Matches policy name/description against keyword entries in the CFR map.
    Returns merged results for all matching categories.

    Returns:
        {
            "ecfr": [{"title": 42, "part": "418", ...}, ...],
            "fr_terms": ["hospice conditions participation", ...],
            "agency_ids": ["centers-for-medicare-medicaid-services"],
            "matched_categories": ["hospice"],
        }
    """
    cfr_map = _load_map()
    search_text = f"{policy_name} {policy_description}".lower()

    ecfr_refs = []
    fr_terms = []
    agency_ids = set()
    matched = []

    for category, entry in cfr_map.items():
        keywords = entry.get("keywords", [])
        if any(kw.lower() in search_text for kw in keywords):
            ecfr_refs.extend(entry.get("ecfr", []))
            fr_terms.extend(entry.get("fr_terms", []))
            if entry.get("agency_id"):
                agency_ids.add(entry["agency_id"])
            matched.append(category)

    return {
        "ecfr": ecfr_refs,
        "fr_terms": fr_terms,
        "agency_ids": sorted(agency_ids),
        "matched_categories": matched,
    }


def get_all_categories() -> list[str]:
    """Return all known policy categories in the CFR map."""
    return sorted(_load_map().keys())
