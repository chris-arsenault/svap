"""
Delta processing utilities for incremental pipeline stages.

Each stage computes an input_hash for entities it would process, compares
against stored hashes in stage_processing_log, and skips unchanged entities.
"""

import hashlib


def compute_hash(*parts: str) -> str:
    """Compute a 12-char hex hash from concatenated parts."""
    combined = "|".join(str(p) for p in parts)
    return hashlib.sha256(combined.encode()).hexdigest()[:12]


def taxonomy_fingerprint(taxonomy: list[dict]) -> str:
    """Stable fingerprint of the approved taxonomy state.

    Changes when qualities are added or removed. Embedded in input hashes
    for all taxonomy-dependent stages, so a new approved quality invalidates
    all downstream processing logs.
    """
    ids = sorted(q["quality_id"] for q in taxonomy)
    return hashlib.sha256(":".join(ids).encode()).hexdigest()[:12]


def calibration_fingerprint(calibration: dict | None) -> str:
    """Fingerprint of the calibration threshold."""
    return str(calibration["threshold"]) if calibration else "3"


def policy_quality_profile(policy_id: str, policy_scores: list[dict]) -> str:
    """Fingerprint of which qualities are present for a policy.

    Used by Stage 5 to detect when a policy's scoring profile changes.
    """
    present_qualities = sorted(
        ps["quality_id"]
        for ps in policy_scores
        if ps["policy_id"] == policy_id and ps["present"]
    )
    return ":".join(present_qualities) if present_qualities else "none"
