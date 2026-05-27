# Where: tools/source_ops/collect.py
# What: Public-source collection step for source automation.
# Why: Snapshot raw upstream content before normalization so diffs are inspectable and repeatable.
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
from urllib.parse import quote, urlparse
from urllib.request import Request, urlopen
from xml.etree import ElementTree

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


def _fetch_url(url: str, timeout: int) -> dict[str, object]:
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
            body = response.read().decode("utf-8", errors="replace")
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


def _same_host(url: str, candidate: str) -> bool:
    return urlparse(url).netloc == urlparse(candidate).netloc


def _matches_prefixes(url: str, include_prefixes: list[str], exclude_prefixes: list[str]) -> bool:
    if include_prefixes and not any(url.startswith(prefix) for prefix in include_prefixes):
        return False
    return not any(url.startswith(prefix) for prefix in exclude_prefixes)


def _sitemap_locs(xml_body: str) -> list[str]:
    root = ElementTree.fromstring(xml_body)
    return [
        element.text.strip()
        for element in root.iter()
        if element.tag.endswith("loc") and element.text and element.text.strip()
    ]


def _sitemap_urls(target: dict[str, object], timeout: int) -> list[str]:
    sitemap = _fetch_url(str(target["url"]), timeout)
    locs = _sitemap_locs(str(sitemap["body"]))
    include_prefixes = [str(value) for value in target.get("include_prefixes", [])]
    exclude_prefixes = [str(value) for value in target.get("exclude_prefixes", [])]
    max_pages = int(target["max_pages"])
    page_urls: list[str] = []

    def append_matching(url: str) -> bool:
        if len(page_urls) >= max_pages:
            return True
        if _same_host(str(target["url"]), url) and _matches_prefixes(url, include_prefixes, exclude_prefixes):
            page_urls.append(url)
        return len(page_urls) >= max_pages

    for loc in locs:
        if len(page_urls) >= max_pages:
            break
        if loc.endswith(".xml") and _same_host(str(target["url"]), loc):
            nested = _fetch_url(loc, timeout)
            for nested_loc in _sitemap_locs(str(nested["body"])):
                if append_matching(nested_loc):
                    break
            continue
        append_matching(loc)
    return page_urls


def _github_tree_parts(url: str) -> tuple[str, str, str, str]:
    parsed = urlparse(url)
    parts = [part for part in parsed.path.split("/") if part]
    if parsed.netloc != "github.com" or len(parts) < 2:
        raise ValueError(f"github_tree url must be a GitHub repository URL: {url}")
    owner, repo = parts[0], parts[1]
    branch = "main"
    base_path = ""
    if len(parts) >= 4 and parts[2] == "tree":
        branch = parts[3]
        base_path = "/".join(parts[4:])
    return owner, repo, branch, base_path


def _github_tree_urls(target: dict[str, object], timeout: int) -> list[str]:
    owner, repo, branch, base_path = _github_tree_parts(str(target["url"]))
    api_url = f"https://api.github.com/repos/{owner}/{repo}/git/trees/{quote(branch, safe='')}?recursive=1"
    tree = json.loads(str(_fetch_url(api_url, timeout)["body"]))
    include_prefixes = [str(value).strip("/") for value in target.get("include_prefixes", [])]
    exclude_prefixes = [str(value).strip("/") for value in target.get("exclude_prefixes", [])]
    if base_path:
        include_prefixes = [base_path, *include_prefixes]
    max_pages = int(target["max_pages"])
    urls: list[str] = []
    for entry in tree.get("tree", []):
        path = str(entry.get("path", ""))
        if entry.get("type") != "blob" or not path.endswith((".md", ".mdx")):
            continue
        if include_prefixes and not any(path.startswith(prefix) for prefix in include_prefixes):
            continue
        if any(path.startswith(prefix) for prefix in exclude_prefixes):
            continue
        urls.append(f"https://raw.githubusercontent.com/{owner}/{repo}/{branch}/{path}")
    return urls[:max_pages]


def _target_urls(target: dict[str, object], timeout: int) -> list[str]:
    strategy = target["crawl_strategy"]
    if strategy in {"explicit_urls", "llms_full"}:
        return [str(target["url"])]
    if strategy == "sitemap":
        return _sitemap_urls(target, timeout)
    if strategy == "github_tree":
        return _github_tree_urls(target, timeout)
    raise ValueError(f"unknown crawl_strategy: {strategy}")


def _fetch_target(target: dict[str, object], timeout: int) -> list[dict[str, object]]:
    fetched = []
    urls = _target_urls(target, timeout)
    for index, url in enumerate(urls):
        entry = _fetch_url(url, timeout)
        entry["label"] = f"{target['label']}:{index + 1}" if len(urls) > 1 else target["label"]
        entry["role"] = "crawl_targets"
        entry["source_type"] = target["source_type"]
        entry["target_label"] = target["label"]
        entry["crawl_strategy"] = target["crawl_strategy"]
        entry["coverage_role"] = target["coverage_role"]
        fetched.append(entry)
    return fetched


def collect_source(source: dict[str, object], timeout: int, raw_dir: Path) -> dict[str, object]:
    fetched_items: list[dict[str, object]] = []
    for target in source.get("crawl_targets", []):
        fetched_items.extend(_fetch_target(target, timeout))

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
