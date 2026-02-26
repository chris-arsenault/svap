"""
Stage 0: Enforcement Source Fetching

Fetches enforcement action documents (DOJ press releases, GAO reports) from
their official government URLs, extracts visible text from the HTML, and
ingests into the RAG store so Stage 1 can extract structured cases.

Sources are read from the enforcement_sources database table. Sources that
already have a document (has_document=True) are skipped. After fetching,
all pending documents are validated by the LLM to confirm they are relevant
enforcement documents, and a summary is generated.

Input:  Enforcement sources from the database (seeded from enforcement_sources.json)
Output: Documents in the RAG store (doc_type='enforcement'), summaries in enforcement_sources
"""

import os
import re
import ssl
import urllib.request
from datetime import UTC, datetime
from html.parser import HTMLParser

from svap.bedrock_client import BedrockClient
from svap.rag import DocumentIngester
from svap.storage import SVAPStorage

CONFIG_BUCKET = os.environ.get("SVAP_CONFIG_BUCKET", "")

# Tags whose content should be skipped entirely
_SKIP_TAGS = frozenset({"script", "style", "noscript", "svg", "head"})

VALIDATION_SYSTEM = (
    "You are an analyst evaluating enforcement documents for a healthcare fraud "
    "analysis pipeline. You determine whether a document describes a real enforcement "
    "action and provide a concise summary."
)


class _HTMLTextExtractor(HTMLParser):
    """Extract visible text from HTML, skipping scripts/styles."""

    def __init__(self):
        super().__init__()
        self._parts: list[str] = []
        self._skip_depth = 0

    def handle_starttag(self, tag, attrs):
        if tag in _SKIP_TAGS:
            self._skip_depth += 1

    def handle_endtag(self, tag):
        if tag in _SKIP_TAGS and self._skip_depth > 0:
            self._skip_depth -= 1

    def handle_data(self, data):
        if self._skip_depth == 0:
            self._parts.append(data)

    def get_text(self) -> str:
        raw = " ".join(self._parts)
        lines = [line.strip() for line in raw.splitlines()]
        text = "\n".join(line for line in lines if line)
        text = re.sub(r"\n{3,}", "\n\n", text)
        return text.strip()


def _fetch_url(url: str, timeout: int = 30) -> str:
    """Fetch a URL and return the response body as text."""
    ctx = ssl.create_default_context()
    req = urllib.request.Request(
        url,
        headers={
            "User-Agent": "SVAP-Pipeline/1.0 (Structural Vulnerability Analysis)",
            "Accept": "text/html,application/xhtml+xml,*/*",
        },
    )
    with urllib.request.urlopen(req, timeout=timeout, context=ctx) as resp:
        charset = resp.headers.get_content_charset() or "utf-8"
        return resp.read().decode(charset, errors="replace")


def _extract_text(html: str) -> str:
    """Extract visible text from HTML content."""
    parser = _HTMLTextExtractor()
    parser.feed(html)
    return parser.get_text()


def _store_to_s3(key: str, body_bytes: bytes, content_type: str):
    """Store a file to the data S3 bucket."""
    if not CONFIG_BUCKET:
        return
    import boto3

    boto3.client("s3").put_object(
        Bucket=CONFIG_BUCKET, Key=key, Body=body_bytes, ContentType=content_type
    )


def _validate_document(client: BedrockClient, source: dict, doc_text: str) -> tuple[str, bool]:
    """Use LLM to validate a document and generate a summary.

    Returns (summary, is_valid).
    """
    text_sample = doc_text[:4000]
    prompt = (
        f"Analyze this enforcement document and provide:\n"
        f"1. A 2-3 sentence summary of the enforcement action described\n"
        f"2. Whether this is a valid healthcare fraud enforcement document (true/false)\n\n"
        f"Document source: {source['name']}\n"
        f"Description: {source.get('description', 'N/A')}\n"
        f"URL: {source.get('url', 'uploaded')}\n\n"
        f"Document text (first portion):\n{text_sample}\n\n"
        f'Respond in JSON: {{"summary": "...", "is_valid": true/false}}'
    )
    result = client.invoke_json(prompt, system=VALIDATION_SYSTEM, temperature=0.1, max_tokens=500)
    return result.get("summary", ""), bool(result.get("is_valid", False))


def _fetch_missing_documents(storage, sources, config):
    """Phase 1: Fetch documents for sources that don't have one yet."""
    ingester = DocumentIngester(storage, config)
    fetched = 0
    skipped = 0
    failed = 0

    for source in sources:
        source_id = source["source_id"]
        name = source["name"]
        url = source.get("url")

        if source["has_document"]:
            print(f"  Skipping (has document): {name}")
            skipped += 1
            continue

        if not url:
            print(f"  Skipping (no URL): {name}")
            skipped += 1
            continue

        print(f"  Fetching: {name}")
        try:
            html = _fetch_url(url)
            text = _extract_text(html)

            if len(text) < 200:
                print(f"    Skipped: text too short ({len(text)} chars)")
                failed += 1
                continue

            doc_id, n_chunks = ingester.ingest_text(
                text=text,
                filename=source_id,
                doc_type="enforcement",
                metadata={
                    "url": url,
                    "source_name": name,
                    "source_id": source_id,
                    "fetch_date": datetime.now(UTC).isoformat(),
                },
            )

            s3_key = f"enforcement-sources/{source_id}/press_release.html"
            _store_to_s3(s3_key, html.encode("utf-8"), "text/html")

            storage.update_enforcement_source_document(source_id, s3_key=s3_key, doc_id=doc_id)
            print(f"    Ingested: {len(text)} chars, {n_chunks} chunks")
            fetched += 1

        except Exception as e:
            print(f"    Failed to fetch {url}: {e}")
            failed += 1

    return fetched, skipped, failed


def _validate_pending_documents(storage, client):
    """Phase 2: Validate all sources that have documents but no summary yet."""
    validated = 0
    sources = storage.get_enforcement_sources()
    docs = storage.get_all_documents(doc_type="enforcement")

    for source in sources:
        if not source["has_document"] or source["validation_status"] != "pending":
            continue

        print(f"  Validating: {source['name']}")
        try:
            doc = next((d for d in docs if d["doc_id"] == source["doc_id"]), None)
            if not doc:
                print("    Document not found in RAG store")
                storage.update_enforcement_source_summary(
                    source["source_id"], "Document not found in RAG store", "error"
                )
                continue

            summary, is_valid = _validate_document(client, source, doc["full_text"])
            status = "valid" if is_valid else "invalid"
            storage.update_enforcement_source_summary(source["source_id"], summary, status)
            print(f"    {status}: {summary[:80]}...")
            validated += 1

        except Exception as e:
            print(f"    Validation failed: {e}")
            storage.update_enforcement_source_summary(
                source["source_id"], f"Validation error: {e}", "error"
            )

    return validated


def run(storage: SVAPStorage, client: BedrockClient, run_id: str, config: dict):
    """
    Execute Stage 0: Fetch enforcement source documents and ingest into RAG store.

    Reads sources from the database. Skips sources that already have documents.
    After fetching, validates all pending documents with the LLM.
    """
    print("Stage 0: Enforcement Source Fetching")
    storage.log_stage_start(run_id, 0)

    try:
        storage.seed_enforcement_sources_if_empty()
        sources = storage.get_enforcement_sources()

        fetched, skipped, failed = _fetch_missing_documents(storage, sources, config)
        validated = _validate_pending_documents(storage, client)

        storage.log_stage_complete(
            run_id,
            0,
            {
                "documents_fetched": fetched,
                "documents_skipped": skipped,
                "documents_failed": failed,
                "documents_validated": validated,
            },
        )
        print(
            f"  Stage 0 complete: {fetched} fetched, {skipped} skipped, "
            f"{failed} failed, {validated} validated."
        )

    except Exception as e:
        storage.log_stage_failed(run_id, 0, str(e))
        raise
