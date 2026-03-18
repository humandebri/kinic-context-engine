# Where: tools/source_ops/validate.py
# What: Validation rules for canonical source payloads and registry-backed source semantics.
# Why: Fail fast before writing bad payloads into memory canisters or updating catalog metadata.
from __future__ import annotations

import argparse
import json
from pathlib import Path

if __package__ in {None, ""}:
    import sys

    sys.path.append(str(Path(__file__).resolve().parents[2]))
    from tools.source_ops.common import load_jsonl, slugify_source_id
    from tools.source_ops.config import load_settings
    from tools.source_ops.registry import load_registry, select_sources
else:
    from .common import load_jsonl, slugify_source_id
    from .config import load_settings
    from .registry import load_registry, select_sources


REQUIRED_FIELDS = ["source_id", "title", "snippet", "citation"]
VERSIONED_V1_SOURCES = {"/vercel/next.js", "/supabase/docs", "/react/docs"}


def validate_payloads(source: dict[str, object], payloads: list[dict[str, object]]) -> list[str]:
    errors: list[str] = []
    source_id = source["source_id"]
    if not payloads:
        errors.append(f"{source_id}: payload collection is empty")
        return errors

    for index, payload in enumerate(payloads):
        for field in REQUIRED_FIELDS:
            if not str(payload.get(field, "")).strip():
                errors.append(f"{source_id}[{index}]: missing `{field}`")
        if payload.get("source_id") != source_id:
            errors.append(f"{source_id}[{index}]: payload source_id mismatch")
        citation = str(payload.get("citation", ""))
        if not citation.startswith(("http://", "https://", "file://")):
            errors.append(f"{source_id}[{index}]: citation must be absolute")
        if source_id in VERSIONED_V1_SOURCES and not str(payload.get("version", "")).strip():
            errors.append(f"{source_id}[{index}]: version is required")
        content = str(payload.get("content", ""))
        snippet = str(payload.get("snippet", ""))
        if content and len(content) > 280 and content == snippet:
            errors.append(f"{source_id}[{index}]: snippet should not duplicate long content")
    return errors


def validate_source(source: dict[str, object], normalized_dir: Path) -> list[str]:
    payloads = load_jsonl(normalized_dir / f"{slugify_source_id(source['source_id'])}.jsonl")
    return validate_payloads(source, payloads)


def main() -> int:
    parser = argparse.ArgumentParser(description="Validate normalized source payloads")
    parser.add_argument("--source", help="Only validate one source_id")
    args = parser.parse_args()

    settings = load_settings()
    sources = load_registry(settings)
    errors: list[str] = []
    for source in select_sources(sources, source_id=args.source):
        errors.extend(validate_source(source, settings.normalized_dir))

    status = "ok" if not errors else "invalid"
    print(json.dumps({"status": status, "errors": errors}, indent=2))
    return 0 if not errors else 1


if __name__ == "__main__":
    raise SystemExit(main())
