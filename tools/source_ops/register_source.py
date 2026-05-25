# Where: tools/source_ops/register_source.py
# What: Small registry upsert helper for adding a documentation source.
# Why: Make source registration a repeatable operation instead of manual JSON editing.
from __future__ import annotations

import argparse
import json
from pathlib import Path
from urllib.parse import urlparse

if __package__ in {None, ""}:
    import sys

    sys.path.append(str(Path(__file__).resolve().parents[2]))
    from tools.source_ops.common import utc_now, write_text
    from tools.source_ops.config import Settings, load_settings
    from tools.source_ops.registry import load_registry, validate_registry
else:
    from .common import utc_now, write_text
    from .config import Settings, load_settings
    from .registry import load_registry, validate_registry


DEFAULT_DROP_SELECTORS = ["nav", "header", "footer", "aside"]
DEFAULT_CONTENT_ROOTS = ["main", "article"]


def parse_labeled_url(value: str, index: int) -> dict[str, str]:
    label = f"docs-{index}"
    url = value
    if "=" in value:
        candidate_label, candidate_url = value.split("=", 1)
        candidate_label = candidate_label.strip()
        candidate_url = candidate_url.strip()
        if not _is_absolute_url(candidate_label) and _is_absolute_url(candidate_url):
            label = candidate_label
            url = candidate_url
    if not label:
        raise ValueError("url label must not be empty")
    if not _is_absolute_url(url):
        raise ValueError(f"url must be absolute: {url}")
    return {"label": label, "url": url}


def build_source_entry(
    *,
    source_id: str,
    title: str,
    urls: list[str],
    aliases: list[str],
    versions: list[str],
    citations: list[str],
    trust: str,
    domain: str,
    cadence: str,
    chunk_target_chars: int,
) -> dict[str, object]:
    source_id = source_id.strip()
    title = title.strip()
    if not source_id.startswith("/"):
        raise ValueError("source_id must start with `/`")
    if not title:
        raise ValueError("title must not be empty")
    if not urls:
        raise ValueError("at least one --url is required")
    public_urls = [parse_labeled_url(value, index + 1) for index, value in enumerate(urls)]
    citation_values = citations or [item["url"] for item in public_urls]
    for citation in citation_values:
        if not _is_absolute_url(citation):
            raise ValueError(f"citation must be absolute: {citation}")

    return {
        "source_id": source_id,
        "kind": "docs",
        "enabled": True,
        "public_urls": public_urls,
        "discovery_urls": [],
        "normalization_profile": "docs_html",
        "catalog_metadata": {
            "title": title,
            "aliases": _dedupe([source_id.strip("/"), title, *aliases]),
            "domain": domain,
            "trust": trust,
            "supported_versions": _dedupe(versions),
            "citations": _dedupe(citation_values),
            "retrieved_at": utc_now(),
        },
        "cadence": cadence,
        "version_strategy": "latest_supported_version" if versions else "unversioned",
        "extraction_hints": {
            "content_roots": DEFAULT_CONTENT_ROOTS,
            "drop_selectors": DEFAULT_DROP_SELECTORS,
            "chunk_target_chars": chunk_target_chars,
        },
        "smoke_queries": {
            "resolve": aliases[0] if aliases else title,
            "query": aliases[0] if aliases else title,
            "pack": aliases[0] if aliases else title,
        },
    }


def upsert_source_entry(
    sources: list[dict[str, object]],
    entry: dict[str, object],
) -> list[dict[str, object]]:
    source_id = entry["source_id"]
    without_existing = [source for source in sources if source.get("source_id") != source_id]
    return [*without_existing, entry]


def save_registry(settings: Settings, sources: list[dict[str, object]]) -> None:
    errors = validate_registry(sources)
    if errors:
        raise ValueError("\n".join(errors))
    write_text(settings.registry_path, json.dumps(sources, indent=2, ensure_ascii=True) + "\n")


def register_source(settings: Settings, entry: dict[str, object]) -> dict[str, object]:
    sources = upsert_source_entry(load_registry(settings), entry)
    save_registry(settings, sources)
    return {"status": "ok", "source_id": entry["source_id"], "registry_path": str(settings.registry_path)}


def _dedupe(values: list[str]) -> list[str]:
    seen: set[str] = set()
    result = []
    for value in values:
        item = str(value).strip()
        if not item or item in seen:
            continue
        seen.add(item)
        result.append(item)
    return result


def _is_absolute_url(value: str) -> bool:
    parsed = urlparse(value)
    return parsed.scheme in {"http", "https", "file"} and bool(parsed.netloc or parsed.scheme == "file")


def main() -> int:
    parser = argparse.ArgumentParser(description="Register or replace one documentation source")
    parser.add_argument("--source-id", required=True, help="logical source id, e.g. /tanstack/query")
    parser.add_argument("--title", required=True, help="catalog title")
    parser.add_argument("--url", action="append", required=True, help="URL or label=URL; repeatable")
    parser.add_argument("--alias", action="append", default=[], help="source alias; repeatable")
    parser.add_argument("--version", action="append", default=[], help="supported version; repeatable")
    parser.add_argument("--citation", action="append", default=[], help="catalog citation; repeatable")
    parser.add_argument("--trust", default="official")
    parser.add_argument("--domain", default="code_docs")
    parser.add_argument("--cadence", default="manual", choices=["manual", "daily"])
    parser.add_argument("--chunk-target-chars", type=int, default=900)
    args = parser.parse_args()

    settings = load_settings()
    entry = build_source_entry(
        source_id=args.source_id,
        title=args.title,
        urls=args.url,
        aliases=args.alias,
        versions=args.version,
        citations=args.citation,
        trust=args.trust,
        domain=args.domain,
        cadence=args.cadence,
        chunk_target_chars=args.chunk_target_chars,
    )
    print(json.dumps(register_source(settings, entry), indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
