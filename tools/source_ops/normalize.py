# Where: tools/source_ops/normalize.py
# What: Normalize raw collected artifacts into canonical source payload JSONL.
# Why: Keep memory payloads deterministic and aligned with the documented schema.
from __future__ import annotations

import argparse
import json
import re
from html.parser import HTMLParser
from pathlib import Path

if __package__ in {None, ""}:
    import sys

    sys.path.append(str(Path(__file__).resolve().parents[2]))
    from tools.source_ops.common import clean_text, dump_json, dump_jsonl, load_json, slugify_source_id, summarize_text
    from tools.source_ops.config import load_settings
    from tools.source_ops.registry import load_registry, select_sources
else:
    from .common import clean_text, dump_json, dump_jsonl, load_json, slugify_source_id, summarize_text
    from .config import load_settings
    from .registry import load_registry, select_sources


CHUNK_CHAR_LIMIT = 900
MIN_HTML_TEXT_RATIO = 0.02
MIN_EXTRACTED_CHARS = 160


def _extract_title(body: str, fallback: str) -> str:
    match = re.search(r"<title>(.*?)</title>", body, flags=re.IGNORECASE | re.DOTALL)
    if match:
        return clean_text(match.group(1)) or fallback
    first_line = body.strip().splitlines()[0] if body.strip() else ""
    return summarize_text(first_line or fallback, limit=80)


def _default_version(source: dict[str, object]) -> str | None:
    versions = source["catalog_metadata"].get("supported_versions", [])
    return versions[-1] if versions else None


def _normalized_meta_path(normalized_dir: Path, source_id: str) -> Path:
    return normalized_dir / f"{slugify_source_id(source_id)}.meta.json"


def load_normalization_meta(normalized_dir: Path, source_id: str) -> dict[str, object]:
    path = _normalized_meta_path(normalized_dir, source_id)
    if not path.exists():
        return {"warnings": []}
    return load_json(path)


def _selector_matches(tag: str, attrs: dict[str, str], selector: str) -> bool:
    if selector.startswith("."):
        classes = attrs.get("class", "").split()
        return selector[1:] in classes
    if selector.startswith("#"):
        return attrs.get("id") == selector[1:]
    return tag == selector.lower()


class StructuredHTMLExtractor(HTMLParser):
    def __init__(self, *, content_roots: list[str], drop_selectors: list[str]) -> None:
        super().__init__()
        self._skip_depth = 0
        self._capture_depth = 0
        self._current_heading: tuple[str, str] | None = None
        self._current_text: list[str] = []
        self.sections: list[dict[str, object]] = []
        self.title: str | None = None
        self.raw_text_parts: list[str] = []
        self.content_roots = content_roots
        self.drop_selectors = drop_selectors
        self.root_matches = 0

    def handle_starttag(self, tag: str, attrs: list[tuple[str, str | None]]) -> None:
        attributes = {key: value or "" for key, value in attrs}
        if tag in {"script", "style"} or any(
            _selector_matches(tag, attributes, selector) for selector in self.drop_selectors
        ):
            self._skip_depth += 1
            return
        if any(_selector_matches(tag, attributes, selector) for selector in self.content_roots):
            self.root_matches += 1
            self._capture_depth += 1
        if self._skip_depth:
            return
        if self.content_roots and self.root_matches and self._capture_depth == 0 and tag != "title":
            return
        if tag == "title":
            self._current_heading = ("__title__", "")
            self._current_text = []
            return
        if tag in {"h1", "h2", "h3", "h4", "h5", "h6"}:
            self._flush_body()
            self._current_heading = (attributes.get("id") or "", tag)
            self._current_text = []
            return
        if tag in {"p", "li", "pre", "code"}:
            self._flush_body()
            self._current_heading = self._current_heading or ("", "p")
            self._current_text = []

    def handle_endtag(self, tag: str) -> None:
        if tag in {"script", "style", "nav", "header", "footer", "aside"}:
            self._skip_depth = max(0, self._skip_depth - 1)
            return
        if self._skip_depth:
            return
        if self._capture_depth > 0 and tag in {selector for selector in self.content_roots if not selector.startswith((".", "#"))}:
            self._capture_depth = max(0, self._capture_depth - 1)
        if tag == "title":
            title = clean_text(" ".join(self._current_text))
            if title:
                self.title = title
            self._current_heading = None
            self._current_text = []
            return
        if tag in {"p", "li", "pre", "code"}:
            self._flush_body()
            return
        if tag in {"h1", "h2", "h3", "h4", "h5", "h6"}:
            heading = clean_text(" ".join(self._current_text))
            self._current_heading = ((self._current_heading or ("", ""))[0], heading or tag)
            self._current_text = []

    def handle_data(self, data: str) -> None:
        if self._skip_depth:
            return
        if self.content_roots and self.root_matches and self._capture_depth == 0:
            return
        text = clean_text(data)
        if not text:
            return
        self.raw_text_parts.append(text)
        self._current_text.append(text)

    def _flush_body(self) -> None:
        body = clean_text(" ".join(self._current_text))
        if body and self._current_heading != ("__title__", ""):
            heading_id, heading_text = self._current_heading or ("", "")
            self.sections.append(
                {
                    "heading_id": heading_id,
                    "heading": clean_text(heading_text),
                    "text": body,
                }
            )
        self._current_text = []


def _split_paragraph(paragraph: str, limit: int) -> list[str]:
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


def _structured_sections(
    body: str,
    *,
    content_roots: list[str],
    drop_selectors: list[str],
) -> tuple[str | None, list[dict[str, object]], str, list[str]]:
    parser = StructuredHTMLExtractor(content_roots=content_roots, drop_selectors=drop_selectors)
    parser.feed(body)
    parser._flush_body()
    warnings: list[str] = []
    if content_roots and parser.root_matches == 0:
        warnings.append("content_roots_unmatched")
    return parser.title, parser.sections, clean_text(" ".join(parser.raw_text_parts)), warnings


def _normalize_item(
    source: dict[str, object],
    item: dict[str, object],
    collected_at: str,
) -> tuple[list[dict[str, object]], list[str]]:
    body = item["body"]
    hints = source.get("extraction_hints", {})
    title, sections, all_text, warnings = _structured_sections(
        body,
        content_roots=hints.get("content_roots", []),
        drop_selectors=hints.get("drop_selectors", []),
    )
    if not all_text:
        all_text = clean_text(body)
        warnings.append("body_fallback_used")
    is_html = str(item.get("content_type", "")).startswith("text/html")
    suspicious_small = is_html and len(body) >= 2000 and len(all_text) < MIN_EXTRACTED_CHARS
    suspicious_ratio = is_html and len(body) >= 2000 and len(all_text) / max(len(body), 1) < MIN_HTML_TEXT_RATIO
    if suspicious_small or suspicious_ratio:
        raise ValueError(
            f"{source['source_id']}: extracted text from `{item['url']}` looks too small; upstream HTML may have changed"
        )
    if not is_html:
        warnings.append("non_html_content_type")

    payloads: list[dict[str, object]] = []
    fallback_title = title or _extract_title(body, source["catalog_metadata"]["title"])
    base_citation = item.get("final_url") or item["url"]
    if not sections:
        sections = [{"heading_id": "", "heading": item["label"], "text": all_text}]
        warnings.append("section_fallback_used")

    for section_index, section in enumerate(sections):
        heading = clean_text(str(section.get("heading", ""))) or fallback_title
        chunk_limit = int(hints.get("chunk_target_chars", CHUNK_CHAR_LIMIT))
        for chunk_index, chunk in enumerate(_split_paragraph(str(section["text"]), chunk_limit)):
            citation = base_citation
            heading_id = str(section.get("heading_id", "")).strip()
            if heading_id and "#" not in citation:
                citation = f"{citation}#{heading_id}"
            suffix = f"{heading} ({chunk_index + 1})" if len(chunk) < len(str(section["text"])) else heading
            payloads.append(
                {
                    "source_id": source["source_id"],
                    "title": summarize_text(suffix, limit=120),
                    "snippet": summarize_text(chunk),
                    "citation": citation,
                    "version": _default_version(source),
                    "content": chunk,
                    "section": heading.lower().replace(" ", "-")[:80],
                    "tags": source["catalog_metadata"]["aliases"][:3],
                    "retrieved_at": collected_at,
                    "chunk_index": chunk_index,
                    "section_index": section_index,
                }
            )
    return payloads, sorted(set(warnings))


def normalize_source(source: dict[str, object], raw_dir: Path, normalized_dir: Path) -> list[dict[str, object]]:
    latest_path = raw_dir / slugify_source_id(source["source_id"]) / "latest.json"
    raw = load_json(latest_path)
    payloads: list[dict[str, object]] = []
    warnings: list[dict[str, object]] = []
    for item in raw["items"]:
        if item.get("role") != "public_urls":
            continue
        item_payloads, item_warnings = _normalize_item(source, item, raw["collected_at"])
        payloads.extend(item_payloads)
        for warning in item_warnings:
            warnings.append({"url": item.get("final_url") or item["url"], "kind": warning})

    payloads.sort(key=lambda payload: (payload["citation"], payload["section_index"], payload["chunk_index"]))
    output_path = normalized_dir / f"{slugify_source_id(source['source_id'])}.jsonl"
    dump_jsonl(output_path, payloads)
    meta = {"source_id": source["source_id"], "warning_count": len(warnings), "warnings": warnings}
    dump_json(_normalized_meta_path(normalized_dir, source["source_id"]), meta)
    return payloads


def main() -> int:
    parser = argparse.ArgumentParser(description="Normalize collected source artifacts")
    parser.add_argument("--source", help="Only normalize one source_id")
    args = parser.parse_args()

    settings = load_settings()
    sources = load_registry(settings)
    results = []
    for source in select_sources(sources, source_id=args.source):
        payloads = normalize_source(source, settings.raw_dir, settings.normalized_dir)
        results.append({"source_id": source["source_id"], "payload_count": len(payloads)})
    print(json.dumps({"status": "ok", "sources": results}, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
