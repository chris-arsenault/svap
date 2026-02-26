"""
RAG (Retrieval-Augmented Generation) utilities for SVAP pipeline.

Handles document ingestion, chunking, retrieval, and context assembly.
Uses keyword-based retrieval by default. Can be extended with vector
search by configuring an embedding model in config.yaml.
"""

import hashlib
import re
from pathlib import Path

try:
    import tiktoken

    _ENCODER = tiktoken.get_encoding("cl100k_base")

    def count_tokens(text: str) -> int:
        return len(_ENCODER.encode(text))
except ImportError:

    def count_tokens(text: str) -> int:
        return len(text.split()) * 4 // 3  # rough approximation


from svap.storage import SVAPStorage


class DocumentIngester:
    """Ingest documents into the storage layer, chunked for retrieval."""

    def __init__(self, storage: SVAPStorage, config: dict):
        self.storage = storage
        self.chunk_size = config.get("rag", {}).get("chunk_size", 1500)
        self.chunk_overlap = config.get("rag", {}).get("chunk_overlap", 200)

    def ingest_file(self, filepath: str, doc_type: str = "other", metadata: dict | None = None):
        """Read a file, store it, and chunk it for retrieval."""
        path = Path(filepath)
        text = path.read_text(encoding="utf-8", errors="replace")
        doc_id = hashlib.sha256(f"{path.name}:{text[:200]}".encode()).hexdigest()[:16]

        self.storage.insert_document(
            doc_id=doc_id,
            filename=path.name,
            doc_type=doc_type,
            full_text=text,
            metadata=metadata,
        )

        chunks = self._chunk_text(text)
        for i, chunk_text in enumerate(chunks):
            chunk_id = f"{doc_id}_c{i:04d}"
            self.storage.insert_chunk(
                chunk_id=chunk_id,
                doc_id=doc_id,
                chunk_index=i,
                text=chunk_text,
                token_count=count_tokens(chunk_text),
            )

        return doc_id, len(chunks)

    def ingest_text(
        self, text: str, filename: str, doc_type: str = "other", metadata: dict | None = None
    ):
        """Ingest raw text directly (no file)."""
        doc_id = hashlib.sha256(f"{filename}:{text[:200]}".encode()).hexdigest()[:16]
        self.storage.insert_document(doc_id, filename, doc_type, text, metadata)

        chunks = self._chunk_text(text)
        for i, chunk_text in enumerate(chunks):
            chunk_id = f"{doc_id}_c{i:04d}"
            self.storage.insert_chunk(chunk_id, doc_id, i, chunk_text, count_tokens(chunk_text))

        return doc_id, len(chunks)

    def ingest_directory(self, dirpath: str, doc_type: str = "other"):
        """Ingest all .txt, .md, and .json files from a directory."""
        results = []
        for ext in ("*.txt", "*.md", "*.json"):
            for path in Path(dirpath).glob(ext):
                doc_id, n_chunks = self.ingest_file(str(path), doc_type)
                results.append({"file": path.name, "doc_id": doc_id, "chunks": n_chunks})
        return results

    def _chunk_text(self, text: str) -> list[str]:
        """Split text into overlapping chunks by token count."""
        # Split on paragraph boundaries first
        paragraphs = re.split(r"\n\s*\n", text)
        chunks = []
        current_chunk = ""
        current_tokens = 0

        for para in paragraphs:
            para_tokens = count_tokens(para)
            if current_tokens + para_tokens > self.chunk_size and current_chunk:
                chunks.append(current_chunk.strip())
                # Overlap: keep the last portion of the current chunk
                overlap_text = self._get_overlap(current_chunk)
                current_chunk = overlap_text + "\n\n" + para
                current_tokens = count_tokens(current_chunk)
            else:
                current_chunk += "\n\n" + para if current_chunk else para
                current_tokens += para_tokens

        if current_chunk.strip():
            chunks.append(current_chunk.strip())

        return chunks if chunks else [text]

    def _get_overlap(self, text: str) -> str:
        """Extract the last ~chunk_overlap tokens of text."""
        words = text.split()
        overlap_words = max(1, self.chunk_overlap * 3 // 4)  # rough token-to-word
        return " ".join(words[-overlap_words:]) if len(words) > overlap_words else text


class ContextAssembler:
    """Assemble context from retrieved chunks for prompt injection."""

    def __init__(self, storage: SVAPStorage, config: dict):
        self.storage = storage
        self.max_chunks = config.get("rag", {}).get("max_context_chunks", 10)

    def retrieve(
        self, query: str, doc_type: str | None = None, max_chunks: int | None = None
    ) -> str:
        """Retrieve relevant chunks and format as context block."""
        limit = max_chunks or self.max_chunks
        chunks = self.storage.search_chunks(query, doc_type=doc_type, limit=limit)

        if not chunks:
            return ""

        context_parts = []
        for chunk in chunks:
            source = chunk.get("filename", "unknown")
            context_parts.append(f"[Source: {source}]\n{chunk['text']}")

        return "\n\n---\n\n".join(context_parts)

    def retrieve_all_of_type(self, doc_type: str) -> str:
        """Return all documents of a given type concatenated (for small corpora)."""
        docs = self.storage.get_all_documents(doc_type)
        return "\n\n===\n\n".join(f"[{d['filename']}]\n{d['full_text']}" for d in docs)

    def format_cases_context(self, cases: list[dict]) -> str:
        """Format case data as structured context for prompts."""
        parts = []
        for c in cases:
            parts.append(
                f"CASE: {c['case_name']}\n"
                f"  Scheme: {c['scheme_mechanics']}\n"
                f"  Exploited Policy: {c['exploited_policy']}\n"
                f"  Enabling Condition: {c['enabling_condition']}\n"
                f"  Scale: ${c.get('scale_dollars', 'unknown')}\n"
                f"  Detection: {c.get('detection_method', 'unknown')}"
            )
        return "\n\n".join(parts)

    def format_taxonomy_context(self, taxonomy: list[dict]) -> str:
        """Format taxonomy as structured context for prompts."""
        parts = []
        for q in taxonomy:
            parts.append(
                f"{q['quality_id']} — {q['name']}\n"
                f"  Definition: {q['definition']}\n"
                f"  Recognition Test: {q['recognition_test']}\n"
                f"  Exploitation Logic: {q['exploitation_logic']}"
            )
        return "\n\n".join(parts)
