"""
Client for the Electronic Code of Federal Regulations (eCFR) public API.

Provides access to current regulatory text, search, and amendment history.
Uses stdlib only (urllib.request) — no external dependencies.

API docs: https://www.ecfr.gov/developers/documentation/api/v1
Rate limit: ~100 requests per 60 seconds.
"""

import json
import ssl
import urllib.parse
import urllib.request
import xml.etree.ElementTree as ET
from datetime import date

BASE_URL = "https://www.ecfr.gov"
_USER_AGENT = "SVAP-Pipeline/1.0 (Structural Vulnerability Analysis)"


class ECFRClient:
    """Client for the eCFR public API."""

    def __init__(self):
        self._ctx = ssl.create_default_context()

    def search(
        self,
        query: str,
        title: int | None = None,
        per_page: int = 20,
        page: int = 1,
    ) -> dict:
        """Search CFR sections by keyword.

        Returns search results with text excerpts, hierarchy info, and relevance scores.
        """
        params = {"query": query, "per_page": per_page, "page": page}
        if title:
            params["title"] = title
        url = f"{BASE_URL}/api/search/v1/results?{urllib.parse.urlencode(params)}"
        return self._get_json(url)

    def get_full_text(
        self,
        title: int,
        date_str: str | None = None,
        part: str | None = None,
        section: str | None = None,
        subpart: str | None = None,
    ) -> str:
        """Retrieve full regulatory XML for a specific CFR location.

        Returns XML text that should be parsed with parse_xml_sections().
        """
        d = date_str or date.today().isoformat()
        url = f"{BASE_URL}/api/versioner/v1/full/{d}/title-{title}.xml"
        params = {}
        if part:
            params["part"] = part
        if section:
            params["section"] = section
        if subpart:
            params["subpart"] = subpart
        if params:
            url += "?" + urllib.parse.urlencode(params)
        return self._get_text(url)

    def get_structure(
        self,
        title: int,
        date_str: str | None = None,
    ) -> dict:
        """Get the table of contents / hierarchy for a CFR title."""
        d = date_str or date.today().isoformat()
        url = f"{BASE_URL}/api/versioner/v1/structure/{d}/title-{title}.json"
        return self._get_json(url)

    def get_content_versions(
        self,
        title: int,
        part: str | None = None,
        since_date: str | None = None,
    ) -> dict:
        """Get amendment history for a title/part.

        Useful for delta detection — identifies when regulations changed.
        """
        url = f"{BASE_URL}/api/versioner/v1/versions/title-{title}.json"
        params = {}
        if part:
            params["part"] = part
        if since_date:
            params["issue_date[gte]"] = since_date
        if params:
            url += "?" + urllib.parse.urlencode(params)
        return self._get_json(url)

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
            headers={"User-Agent": _USER_AGENT, "Accept": "application/xml,text/xml,*/*"},
        )
        with urllib.request.urlopen(req, timeout=60, context=self._ctx) as resp:
            charset = resp.headers.get_content_charset() or "utf-8"
            return resp.read().decode(charset, errors="replace")


def parse_xml_sections(xml_text: str) -> list[dict]:
    """Parse eCFR XML into section-level chunks.

    Returns a list of dicts with keys: section_id, heading, text, cfr_reference.
    Each section is a manageable unit for LLM processing (~500-2000 tokens).
    """
    try:
        root = ET.fromstring(xml_text)
    except ET.ParseError:
        return _make_fallback_section(xml_text)

    # Try section-level elements first, then broader elements
    sections = _extract_at_level(root, ("DIV8", "SECTION", "SECTNO"), "section", min_len=20)
    if not sections:
        sections = _extract_at_level(root, ("DIV5", "DIV6", "DIV7", "PART", "SUBPART"), "div", min_len=100)
    if not sections:
        return _make_fallback_section(xml_text)
    return sections


def _extract_at_level(root, tag_names: tuple, id_prefix: str, min_len: int) -> list[dict]:
    """Extract sections from XML elements matching given tag names."""
    sections = []
    for elem in root.iter():
        tag = elem.tag.upper() if isinstance(elem.tag, str) else ""
        if tag not in tag_names:
            continue
        text = _element_text(elem)
        if len(text.strip()) < min_len:
            continue
        heading, cfr_ref = _extract_heading(elem)
        sections.append({
            "section_id": cfr_ref or f"{id_prefix}_{len(sections)}",
            "heading": heading,
            "text": text,
            "cfr_reference": cfr_ref,
        })
    return sections


def _extract_heading(elem) -> tuple[str, str]:
    """Extract heading and CFR reference from child elements."""
    heading = ""
    cfr_ref = ""
    for child in elem:
        child_tag = child.tag.upper() if isinstance(child.tag, str) else ""
        if child_tag in ("HEAD", "SUBJECT"):
            heading = _element_text(child).strip()
        elif child_tag == "SECTNO":
            cfr_ref = (child.text or "").strip()
    return heading, cfr_ref


def _make_fallback_section(xml_text: str) -> list[dict]:
    """Create a single-section fallback from raw text."""
    clean = _strip_xml_tags(xml_text)
    if not clean.strip():
        return []
    return [{
        "section_id": "full_text",
        "heading": "Full regulatory text",
        "text": clean,
        "cfr_reference": "",
    }]


def _element_text(elem) -> str:
    """Extract all text content from an XML element and its children."""
    parts = []
    if elem.text:
        parts.append(elem.text)
    for child in elem:
        parts.append(_element_text(child))
        if child.tail:
            parts.append(child.tail)
    return " ".join(parts)


def _strip_xml_tags(xml_text: str) -> str:
    """Crude XML tag removal for fallback cases."""
    import re
    text = re.sub(r"<[^>]+>", " ", xml_text)
    text = re.sub(r"\s+", " ", text)
    return text.strip()
