# Where: tools/source_ops/wiki_chunks.py
# What: Stable wiki chunk identity and path segment helpers.
# Why: Keep chunk IDs deterministic and separate from the writer command runner.
from __future__ import annotations

import hashlib
import re
from typing import Any


def path_segment(value: object) -> str:
    text = str(value).strip().lower()
    text = re.sub(r"[^a-z0-9]+", "-", text)
    return text.strip("-") or "item"


def chunk_content(payload: dict[str, Any]) -> str:
    content = str(payload.get("content") or "").strip()
    if content:
        return content
    snippet = str(payload.get("snippet") or "").strip()
    if snippet:
        return snippet
    raise ValueError(f"empty wiki chunk content for {payload.get('citation') or payload.get('source_id')}")


def chunk_id(
    source: dict[str, Any],
    payload: dict[str, Any],
    section_index: int,
    chunk_index: int,
    content: str,
) -> str:
    seed = (
        str(payload.get("source_id") or source["source_id"])
        + str(payload.get("citation") or "")
        + str(section_index)
        + str(chunk_index)
        + content
    )
    return hashlib.sha256(seed.encode("utf-8")).hexdigest()[:16]


def content_sha256(content: str) -> str:
    return hashlib.sha256(content.encode("utf-8")).hexdigest()


def record_segment(payload: dict[str, Any], index: int) -> str:
    seed = str(payload.get("citation") or payload.get("source_id") or index)
    digest = hashlib.sha256(seed.encode("utf-8")).hexdigest()[:12]
    return f"{path_segment(seed)[:48]}-{digest}"
