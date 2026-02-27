"""
Client for the Federal Register public API.

Provides access to final rules, proposed rules, and notices.
Uses stdlib only (urllib.request) — no external dependencies.

API docs: https://www.federalregister.gov/developers/documentation/api/v1
"""

import json
import ssl
import urllib.parse
import urllib.request

BASE_URL = "https://www.federalregister.gov/api/v1"
_USER_AGENT = "SVAP-Pipeline/1.0 (Structural Vulnerability Analysis)"


class FederalRegisterClient:
    """Client for the Federal Register API."""

    def __init__(self):
        self._ctx = ssl.create_default_context()

    def search_documents(
        self,
        term: str,
        agency_ids: list[str] | None = None,
        doc_type: str | None = None,
        per_page: int = 20,
    ) -> dict:
        """Search Federal Register documents.

        doc_type: RULE (final rules), PRORULE (proposed rules), NOTICE
        agency_ids: e.g. ["centers-for-medicare-medicaid-services"]
        """
        params: list[tuple[str, str]] = [
            ("conditions[term]", term),
            ("per_page", str(per_page)),
        ]
        if agency_ids:
            for aid in agency_ids:
                params.append(("conditions[agencies][]", aid))
        if doc_type:
            params.append(("conditions[type][]", doc_type))
        # Request specific fields to keep response size manageable
        for field in [
            "title", "abstract", "document_number", "publication_date",
            "raw_text_url", "html_url", "type", "agencies",
        ]:
            params.append(("fields[]", field))

        url = f"{BASE_URL}/documents.json?{urllib.parse.urlencode(params)}"
        return self._get_json(url)

    def get_document(self, document_number: str) -> dict:
        """Get a single Federal Register document by document number."""
        url = f"{BASE_URL}/documents/{document_number}.json"
        return self._get_json(url)

    def get_document_text(self, raw_text_url: str) -> str:
        """Fetch the full text of a document via its raw_text_url."""
        return self._get_text(raw_text_url)

    def _get_json(self, url: str) -> dict:
        req = urllib.request.Request(
            url,
            headers={"User-Agent": _USER_AGENT, "Accept": "application/json"},
        )
        with urllib.request.urlopen(req, timeout=30, context=self._ctx) as resp:
            return json.loads(resp.read())

    def _get_text(self, url: str) -> str:
        req = urllib.request.Request(
            url,
            headers={"User-Agent": _USER_AGENT},
        )
        with urllib.request.urlopen(req, timeout=60, context=self._ctx) as resp:
            charset = resp.headers.get_content_charset() or "utf-8"
            return resp.read().decode(charset, errors="replace")
