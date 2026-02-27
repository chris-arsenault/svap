"""
Stage 0A: Case Discovery — Feed Monitoring

Checks configured enforcement source listing pages for new entries,
fetches each new entry, evaluates its structural richness using LLM,
and queues rich documents for ingestion into the pipeline.

Sources scoring >= 0.7 richness are auto-accepted and ingested.
Sources scoring 0.4-0.7 are queued for human review.
Sources below 0.4 are auto-rejected.

This is a standalone operation — not chained into stages 0-6. It writes to
source_candidates and enforcement_sources. Stage 0 picks up new sources on
the next pipeline run.

Input:  source_feeds table (listing page configurations)
Output: source_candidates (discovered + scored entries),
        enforcement_sources + documents/chunks (for accepted entries)
"""

import hashlib
import re
import time
from datetime import UTC, datetime
from html.parser import HTMLParser
from urllib.parse import urljoin

from svap.bedrock_client import BedrockClient
from svap.rag import DocumentIngester
from svap.stages.stage0_source_fetch import _extract_text, _fetch_url, _is_binary_content
from svap.storage import SVAPStorage

LINK_EXTRACTION_SYSTEM = (
    "You are an analyst identifying enforcement case document links "
    "on government agency listing pages. Be precise — only identify links "
    "that lead to individual case documents, not to other listing pages "
    "or general information."
)

RICHNESS_SYSTEM = (
    "You are an analyst evaluating enforcement documents for a healthcare fraud "
    "analysis pipeline. You assess whether a document contains sufficient structural "
    "detail about fraud schemes to be useful for case extraction."
)

# Stage number for logging (standalone, before stage 0)


class _HTMLLinkExtractor(HTMLParser):
    """Extract all links (<a href>) from HTML."""

    def __init__(self):
        super().__init__()
        self.links: list[dict] = []
        self._current_href: str | None = None
        self._current_text: list[str] = []

    def handle_starttag(self, tag, attrs):
        if tag == "a":
            href = dict(attrs).get("href")
            if href:
                self._current_href = href
                self._current_text = []

    def handle_data(self, data):
        if self._current_href is not None:
            self._current_text.append(data.strip())

    def handle_endtag(self, tag):
        if tag == "a" and self._current_href:
            self.links.append({
                "url": self._current_href,
                "text": " ".join(self._current_text).strip(),
            })
            self._current_href = None


def _extract_links(html: str) -> list[dict]:
    """Extract all links from HTML."""
    parser = _HTMLLinkExtractor()
    parser.feed(html)
    return parser.links


def _filter_links_by_selector(links: list[dict], base_url: str, link_selector: str | None) -> list[dict]:
    """Filter links using a regex selector pattern, resolving relative URLs."""
    result = []
    for link in links:
        url = urljoin(base_url, link["url"])
        link["url"] = url
        if link_selector and re.search(link_selector, url):
            result.append(link)
    return result


def _extract_links_via_llm(
    client: BedrockClient,
    links: list[dict],
    page_text: str,
    feed: dict,
) -> list[dict]:
    """Use LLM to identify enforcement document links from a listing page."""
    link_list = "\n".join(
        f"- [{link['text'][:80]}]({link['url']})" for link in links[:100]
    )
    prompt = client.render_prompt(
        "discovery_extract_links.txt",
        feed_name=feed["name"],
        content_type=feed["content_type"],
        base_url=feed["listing_url"],
        link_list=link_list,
        page_text=page_text[:3000],
    )
    result = client.invoke_json(prompt, system=LINK_EXTRACTION_SYSTEM, temperature=0.1, max_tokens=2000)
    if isinstance(result, list):
        return result
    return result.get("links", [])


def _check_feed(
    feed: dict, storage: SVAPStorage, client: BedrockClient, config: dict
) -> list[dict]:
    """Check a single feed listing page for new candidate URLs."""
    listing_url = feed["listing_url"]
    max_per_feed = config.get("discovery", {}).get("max_candidates_per_feed", 50)

    print(f"  Checking feed: {feed['name']}")
    try:
        html = _fetch_url(listing_url)
    except Exception as e:
        print(f"    Failed to fetch listing page: {e}")
        return []

    raw_links = _extract_links(html)
    page_text = _extract_text(html)

    # Try regex selector first for a fast path
    filtered = _filter_links_by_selector(raw_links, listing_url, feed.get("link_selector"))

    # If selector matched nothing or no selector, use LLM
    if not filtered:
        resolved = [{"url": urljoin(listing_url, lnk["url"]), "text": lnk["text"]} for lnk in raw_links]
        filtered = _extract_links_via_llm(client, resolved, page_text, feed)

    # Dedup against existing candidates
    new_candidates = []
    for entry in filtered[:max_per_feed]:
        url = entry.get("url", "")
        if not url:
            continue
        existing = storage.get_candidate_by_url(url)
        if existing:
            continue

        candidate_id = hashlib.sha256(url.encode()).hexdigest()[:12]
        candidate = {
            "candidate_id": candidate_id,
            "feed_id": feed["feed_id"],
            "title": entry.get("title") or entry.get("text", "Untitled")[:200],
            "url": url,
            "discovered_at": datetime.now(UTC).isoformat(),
            "published_date": entry.get("published_date"),
            "status": "discovered",
        }
        storage.insert_candidate(candidate)
        new_candidates.append(candidate)

    print(f"    Found {len(filtered)} links, {len(new_candidates)} new candidates")
    return new_candidates


def _evaluate_richness(
    client: BedrockClient, candidate: dict, text: str
) -> dict:
    """Score a document's structural richness via LLM."""
    prompt = client.render_prompt(
        "discovery_richness.txt",
        title=candidate["title"],
        url=candidate["url"],
        text_sample=text[:6000],
    )
    result = client.invoke_json(prompt, system=RICHNESS_SYSTEM, temperature=0.1, max_tokens=500)
    return {
        "richness_score": float(result.get("richness_score", 0.0)),
        "rationale": result.get("rationale", ""),
        "estimated_cases": int(result.get("estimated_cases", 1)),
        "scheme_types": result.get("scheme_types", []),
    }


def _apply_disposition(eval_result: dict, config: dict) -> str:
    """Determine accept/reject/scored based on richness thresholds."""
    score = eval_result["richness_score"]
    accept = config.get("discovery", {}).get("richness_accept_threshold", 0.7)
    review = config.get("discovery", {}).get("richness_review_threshold", 0.4)
    if score >= accept:
        return "accepted"
    if score >= review:
        return "scored"  # queued for human review
    return "rejected"


def _ingest_candidate(
    candidate: dict, text: str, storage: SVAPStorage, config: dict
) -> dict:
    """Ingest an accepted candidate into enforcement_sources and RAG.

    Checks for an existing enforcement source with the same URL to avoid
    duplicating documents when the same URL enters via multiple paths.
    """
    # Check if an enforcement source with this URL already exists
    existing = storage.get_enforcement_source_by_url(candidate["url"])
    if existing and existing.get("has_document") and existing.get("doc_id"):
        # Reuse existing source and document — no re-ingestion needed
        source_id = existing["source_id"]
        doc_id = existing["doc_id"]
        storage.update_candidate_ingested(candidate["candidate_id"], source_id, doc_id)
        return {"doc_id": doc_id, "n_chunks": 0, "source_id": source_id}

    source_id = existing["source_id"] if existing else f"disc_{candidate['candidate_id']}"
    ingester = DocumentIngester(storage, config)

    doc_id, n_chunks = ingester.ingest_text(
        text=text,
        filename=source_id,
        doc_type="enforcement",
        metadata={
            "url": candidate["url"],
            "source_name": candidate["title"],
            "candidate_id": candidate["candidate_id"],
            "discovery_date": datetime.now(UTC).isoformat(),
        },
    )

    storage.upsert_enforcement_source({
        "source_id": source_id,
        "name": candidate["title"],
        "url": candidate["url"],
        "source_type": "discovery",
        "description": f"Discovered from feed. Richness: {candidate.get('richness_score', 'N/A')}",
        "has_document": True,
        "doc_id": doc_id,
        "validation_status": "pending",
        "candidate_id": candidate["candidate_id"],
        "feed_id": candidate.get("feed_id"),
    })

    storage.update_candidate_ingested(candidate["candidate_id"], source_id, doc_id)
    return {"doc_id": doc_id, "n_chunks": n_chunks, "source_id": source_id}


def run(storage: SVAPStorage, client: BedrockClient, config: dict) -> dict:
    """Execute case discovery: check feeds for new enforcement documents.

    Discovery operates independently of pipeline runs — it writes to global
    tables (source_feeds, source_candidates, enforcement_sources, documents)
    that any subsequent pipeline run will pick up.

    Returns a summary dict of what was discovered/accepted/rejected.
    """
    print("Case Discovery — Feed Monitoring")

    feeds = storage.get_source_feeds(enabled_only=True)

    if not feeds:
        print("  No source feeds configured.")
        return {"feeds_checked": 0}

    total_discovered = 0
    total_accepted = 0
    total_rejected = 0
    total_review = 0

    for feed in feeds:
        new_candidates = _check_feed(feed, storage, client, config)
        total_discovered += len(new_candidates)

        for candidate in new_candidates:
            # Fetch full document
            try:
                html = _fetch_url(candidate["url"])
                text = _extract_text(html)
            except Exception as e:
                print(f"    Failed to fetch {candidate['url']}: {e}")
                storage.update_candidate_status(candidate["candidate_id"], "error")
                continue

            if len(text) < 200:
                print(f"    Text too short ({len(text)} chars), skipping")
                storage.update_candidate_status(candidate["candidate_id"], "error")
                continue

            if _is_binary_content(text):
                print("    Binary/non-readable content (PDF or image), skipping")
                storage.update_candidate_status(candidate["candidate_id"], "error")
                continue

            storage.update_candidate_status(candidate["candidate_id"], "fetched")

            # Evaluate richness
            eval_result = _evaluate_richness(client, candidate, text)
            storage.update_candidate_richness(
                candidate["candidate_id"],
                eval_result["richness_score"],
                eval_result["rationale"],
                eval_result["estimated_cases"],
            )
            candidate["richness_score"] = eval_result["richness_score"]

            # Apply disposition
            disposition = _apply_disposition(eval_result, config)
            print(
                f"    {candidate['title'][:60]}: "
                f"richness={eval_result['richness_score']:.2f} → {disposition}"
            )

            if disposition == "accepted":
                _ingest_candidate(candidate, text, storage, config)
                total_accepted += 1
            elif disposition == "rejected":
                storage.update_candidate_status(candidate["candidate_id"], "rejected")
                total_rejected += 1
            else:
                total_review += 1

            # Be polite to government servers
            time.sleep(1)

        storage.update_feed_last_checked(feed["feed_id"])

    summary = {
        "feeds_checked": len(feeds),
        "candidates_discovered": total_discovered,
        "candidates_accepted": total_accepted,
        "candidates_rejected": total_rejected,
        "candidates_pending_review": total_review,
    }
    print(
        f"  Discovery complete: {total_discovered} discovered, "
        f"{total_accepted} accepted, {total_rejected} rejected, "
        f"{total_review} pending review."
    )
    return summary
