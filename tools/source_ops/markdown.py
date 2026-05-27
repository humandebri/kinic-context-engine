# Where: tools/source_ops/markdown.py
# What: Dependency-free Markdown section and chunk extraction.
# Why: Keep docs/llms/repo payload normalization deterministic without adding parser dependencies.
from __future__ import annotations

import re

from .common import clean_text


def split_paragraph(paragraph: str, limit: int) -> list[str]:
    if len(paragraph) <= limit:
        return [paragraph]
    sentences = re.split(r"(?<=[.!?])\s+", paragraph)
    chunks: list[str] = []
    current = ""
    for sentence in sentences:
        candidate = f"{current} {sentence}".strip() if current else sentence
        if len(candidate) <= limit:
            current = candidate
            continue
        if current:
            chunks.append(current)
        current = sentence
    if current:
        chunks.append(current)
    return chunks


def heading_slug(value: str) -> str:
    slug = re.sub(r"[^a-z0-9]+", "-", clean_text(value).lower()).strip("-")
    return slug or "section"


def split_markdown_blocks(text: str) -> list[str]:
    blocks: list[str] = []
    current: list[str] = []
    in_fence = False
    fence_marker = ""
    for line in text.splitlines():
        stripped = line.strip()
        if stripped.startswith(("```", "~~~")):
            marker = stripped[:3]
            if not in_fence:
                in_fence = True
                fence_marker = marker
            elif marker == fence_marker:
                in_fence = False
                fence_marker = ""
        if not in_fence and not stripped:
            if current:
                blocks.append("\n".join(current).strip())
                current = []
            continue
        current.append(line)
    if current:
        blocks.append("\n".join(current).strip())
    return [block for block in blocks if block]


def split_markdown(text: str, limit: int) -> list[str]:
    chunks: list[str] = []
    current = ""
    for block in split_markdown_blocks(text):
        is_code_block = block.lstrip().startswith(("```", "~~~"))
        candidate = f"{current}\n\n{block}".strip() if current else block
        if len(candidate) <= limit:
            current = candidate
            continue
        if current:
            chunks.append(current)
            current = ""
        if is_code_block:
            chunks.append(block)
            continue
        parts = split_paragraph(block, limit)
        chunks.extend(parts[:-1])
        current = parts[-1] if parts else ""
    if current:
        chunks.append(current)
    return chunks


def markdown_sections(body: str, fallback_label: str) -> tuple[str | None, list[dict[str, object]], str, list[str]]:
    sections: list[dict[str, object]] = []
    title: str | None = None
    heading = fallback_label
    heading_id = heading_slug(fallback_label)
    current: list[str] = []
    in_fence = False
    fence_marker = ""
    for line in body.splitlines():
        stripped = line.strip()
        if stripped.startswith(("```", "~~~")):
            marker = stripped[:3]
            if not in_fence:
                in_fence = True
                fence_marker = marker
            elif marker == fence_marker:
                in_fence = False
                fence_marker = ""
            current.append(line)
            continue
        match = re.match(r"^(#{1,6})\s+(.+?)\s*#*$", stripped)
        if match and not in_fence:
            text = clean_text(match.group(2))
            if current:
                sections.append({"heading_id": heading_id, "heading": heading, "text": "\n".join(current).strip()})
            if title is None:
                title = text
            heading = text or fallback_label
            heading_id = heading_slug(heading)
            current = []
            continue
        current.append(line)
    if current:
        sections.append({"heading_id": heading_id, "heading": heading, "text": "\n".join(current).strip()})
    all_text = clean_text(body)
    warnings = ["section_fallback_used"] if not sections else []
    if not sections and all_text:
        sections = [{"heading_id": heading_slug(fallback_label), "heading": fallback_label, "text": body.strip()}]
    return title, sections, all_text, warnings
