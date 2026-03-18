# Where: tools/source_ops/collect.py
# What: Public-source collection step for source automation.
# Why: Snapshot raw upstream content before normalization so diffs are inspectable and repeatable.
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
from urllib.parse import urlparse
from urllib.request import Request, urlopen

if __package__ in {None, ""}:
    import sys

    sys.path.append(str(Path(__file__).resolve().parents[2]))
    from tools.source_ops.common import dump_json, ensure_dir, read_text, slugify_source_id, utc_now
    from tools.source_ops.config import load_settings
    from tools.source_ops.registry import load_registry, select_sources, validate_registry
else:
    from .common import dump_json, ensure_dir, read_text, slugify_source_id, utc_now
    from .config import load_settings
    from .registry import load_registry, select_sources, validate_registry


def _fetch_url(url: str, timeout: int) -> dict[str, str]:
    parsed = urlparse(url)
    if parsed.scheme == "file":
        body = read_text(Path(parsed.path))
        content_type = "text/plain"
        status_code = 200
        final_url = url
        etag = None
        last_modified = None
    else:
        request = Request(url, headers={"User-Agent": "kinic-source-ops/1.0"})
        with urlopen(request, timeout=timeout) as response:
            content_type = response.headers.get_content_type()
            status_code = response.status
            final_url = response.geturl()
            etag = response.headers.get("ETag")
            last_modified = response.headers.get("Last-Modified")
            body = response.read().decode("utf-8")
    return {
        "url": url,
        "final_url": final_url,
        "content_type": content_type,
        "status_code": status_code,
        "etag": etag,
        "last_modified": last_modified,
        "body": body,
        "sha256": hashlib.sha256(body.encode("utf-8")).hexdigest(),
    }


def collect_source(source: dict[str, object], timeout: int, raw_dir: Path) -> dict[str, object]:
    fetched_items: list[dict[str, str]] = []
    for role_name in ["public_urls", "discovery_urls"]:
        for item in source.get(role_name, []):
            entry = _fetch_url(item["url"], timeout)
            entry["label"] = item["label"]
            entry["role"] = role_name
            fetched_items.append(entry)

    slug = slugify_source_id(source["source_id"])
    source_dir = ensure_dir(raw_dir / slug)
    collected = {
        "source_id": source["source_id"],
        "collected_at": utc_now(),
        "items": sorted(fetched_items, key=lambda item: (item["role"], item["label"], item["url"])),
    }
    timestamp = collected["collected_at"].replace(":", "-")
    dump_json(source_dir / f"{timestamp}.json", collected)
    dump_json(source_dir / "latest.json", collected)
    return collected


def main() -> int:
    parser = argparse.ArgumentParser(description="Collect raw source documents")
    parser.add_argument("--source", help="Only collect one source_id")
    args = parser.parse_args()

    settings = load_settings()
    sources = load_registry(settings)
    errors = validate_registry(sources)
    if errors:
        raise SystemExit(json.dumps({"status": "invalid_registry", "errors": errors}, indent=2))

    collected = [
        collect_source(source, settings.http_timeout_seconds, settings.raw_dir)
        for source in select_sources(sources, source_id=args.source)
    ]
    print(json.dumps({"status": "ok", "sources": collected}, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
