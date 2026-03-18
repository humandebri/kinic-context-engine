# Where: tools/source_ops/diff.py
# What: Source-level diffing and threshold evaluation against the last successful state.
# Why: Gate production updates when upstream changes are too large or destructive.
from __future__ import annotations

import argparse
import json

if __package__ in {None, ""}:
    import sys
    from pathlib import Path

    sys.path.append(str(Path(__file__).resolve().parents[2]))
    from tools.source_ops.common import canonical_hash, dump_json, load_json, load_jsonl, slugify_source_id
    from tools.source_ops.config import Settings, load_settings
    from tools.source_ops.registry import load_registry, select_sources
else:
    from .common import canonical_hash, dump_json, load_json, load_jsonl, slugify_source_id
    from .config import Settings, load_settings
    from .registry import load_registry, select_sources


def load_state(settings: Settings) -> dict[str, object]:
    if not settings.state_path.exists():
        return {"last_run_at": None, "sources": {}}
    return load_json(settings.state_path)


def metadata_fingerprint(source: dict[str, object]) -> str:
    return canonical_hash(source["catalog_metadata"])


def _record_key(row: dict[str, object]) -> str:
    section_index = int(row.get("section_index", 0))
    chunk_index = int(row.get("chunk_index", 0))
    return f"{row['citation']}#{section_index}:{chunk_index}"


def compute_diff(
    source: dict[str, object],
    payloads: list[dict[str, object]],
    previous: dict[str, object] | None,
    settings: Settings,
) -> dict[str, object]:
    previous = previous or {}
    old_by_record = previous.get("record_hashes", {})
    new_by_record = {_record_key(row): canonical_hash(row) for row in payloads}
    added = sorted(set(new_by_record) - set(old_by_record))
    removed = sorted(set(old_by_record) - set(new_by_record))
    changed = sorted(
        record_key
        for record_key in set(new_by_record) & set(old_by_record)
        if new_by_record[record_key] != old_by_record[record_key]
    )
    status = "noop"
    if not old_by_record and new_by_record:
        status = "new"
    elif old_by_record and not new_by_record:
        status = "deleted"
    elif added or removed or changed:
        status = "updated"

    metadata_changed = previous.get("metadata_hash") != metadata_fingerprint(source)
    total_changes = len(added) + len(removed) + len(changed)
    needs_review = total_changes > settings.max_changed_records or len(removed) > settings.max_deleted_records
    return {
        "source_id": source["source_id"],
        "status": status,
        "metadata_changed": metadata_changed,
        "added_records": len(added),
        "removed_records": len(removed),
        "changed_records": len(changed),
        "total_changes": total_changes,
        "needs_review": needs_review,
        "normalized_fingerprint": canonical_hash(payloads),
        "metadata_hash": metadata_fingerprint(source),
        "record_hashes": new_by_record,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description="Diff normalized source payloads against state")
    parser.add_argument("--source", help="Only diff one source_id")
    args = parser.parse_args()

    settings = load_settings()
    state = load_state(settings)
    sources = load_registry(settings)
    results = []
    for source in select_sources(sources, source_id=args.source):
        payloads = load_jsonl(settings.normalized_dir / f"{slugify_source_id(source['source_id'])}.jsonl")
        previous = state["sources"].get(source["source_id"])
        results.append(compute_diff(source, payloads, previous, settings))

    report_path = settings.reports_dir / "latest-diff.json"
    dump_json(report_path, {"results": results})
    print(json.dumps({"status": "ok", "report_path": str(report_path), "results": results}, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
