# Where: tools/source_ops/run_refresh.py
# What: End-to-end orchestration for collection, diffing, staging, promotion, and reporting.
# Why: Give Codex automation one stable entrypoint for daily source refresh work.
from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

if __package__ in {None, ""}:
    import sys

    sys.path.append(str(Path(__file__).resolve().parents[2]))
    from tools.source_ops.apply_wiki import apply_wiki
    from tools.source_ops.collect import collect_source
    from tools.source_ops.common import dump_json, load_json, load_jsonl, slugify_source_id, utc_now, write_text
    from tools.source_ops.config import Settings, load_settings
    from tools.source_ops.diff import compute_diff
    from tools.source_ops.normalize import load_normalization_meta, normalize_source
    from tools.source_ops.registry import REQUIRED_COVERAGE_ROLES, load_registry, select_sources, validate_registry
    from tools.source_ops.smoke import smoke_source
    from tools.source_ops.validate import validate_source
else:
    from .apply_wiki import apply_wiki
    from .collect import collect_source
    from .common import dump_json, load_json, load_jsonl, slugify_source_id, utc_now, write_text
    from .config import Settings, load_settings
    from .diff import compute_diff
    from .normalize import load_normalization_meta, normalize_source
    from .registry import REQUIRED_COVERAGE_ROLES, load_registry, select_sources, validate_registry
    from .smoke import smoke_source
    from .validate import validate_source


def _load_state(settings: Settings) -> dict[str, object]:
    if not settings.state_path.exists():
        return {"last_run_at": None, "sources": {}}
    return load_json(settings.state_path)


def _snapshot_path(settings: Settings, source_id: str) -> Path:
    return settings.snapshots_dir / f"{slugify_source_id(source_id)}.json"


def _current_source_snapshot(source: dict[str, object], settings: Settings) -> dict[str, object]:
    return {
        "source": source,
        "payload_snapshot_path": str(_snapshot_path(settings, source["source_id"])),
    }


def _merge_state(previous: dict[str, object] | None, diff_result: dict[str, object]) -> dict[str, object]:
    merged = dict(previous or {})
    merged.update(diff_result)
    return merged


def _store_success_snapshot(settings: Settings, source: dict[str, object]) -> dict[str, object]:
    payloads = _normalized_rows(settings, source["source_id"])
    path = _snapshot_path(settings, source["source_id"])
    dump_json(path, payloads)
    snapshot = _current_source_snapshot(source, settings)
    snapshot["payload_snapshot_path"] = str(path)
    return snapshot


def _write_snapshot_jsonl(settings: Settings, snapshot: dict[str, object]) -> str:
    snapshot_path = Path(snapshot["payload_snapshot_path"])
    rows = load_json(snapshot_path)
    jsonl_path = settings.snapshots_dir / f"{snapshot_path.stem}.rollback.jsonl"
    write_text(
        jsonl_path,
        "\n".join(json.dumps(row, ensure_ascii=True, sort_keys=True) for row in rows) + ("\n" if rows else ""),
    )
    return str(jsonl_path)


def _rollback_source(
    source: dict[str, object],
    snapshot: dict[str, object] | None,
    settings: Settings,
    dry_run: bool,
) -> dict[str, object]:
    if snapshot is None:
        return {"status": "unavailable", "reason": "no previous successful snapshot"}

    previous_source = snapshot["source"]
    previous_payload_path = _write_snapshot_jsonl(settings, snapshot)
    wiki_write = apply_wiki(
        previous_source,
        settings,
        "prod",
        dry_run,
        payload_path_override=previous_payload_path,
        rollback=True,
    )
    smoke = smoke_source(previous_source, settings, "prod", dry_run)
    return {
        "status": "rolled_back"
        if all(step["status"] == "ok" for step in [wiki_write, smoke])
        else "rollback_failed",
        "wiki_write": wiki_write,
        "smoke": smoke,
    }


def _save_report(settings: Settings, report: dict[str, object]) -> None:
    timestamp = report["run_at"].replace(":", "-")
    json_path = settings.reports_dir / f"{timestamp}.json"
    md_path = settings.reports_dir / f"{timestamp}.md"
    dump_json(json_path, report)
    lines = [f"# Source refresh report {report['run_at']}", "", f"Status: {report['status']}", ""]
    for item in report["sources"]:
        lines.append(f"- {item['source_id']}: {item['status']}")
    write_text(md_path, "\n".join(lines) + "\n")


def _quality_gates(
    diff_result: dict[str, object],
    extraction_warnings: list[object],
    missing_required_roles: list[str],
) -> dict[str, object]:
    return {
        "added_records": diff_result["added_records"],
        "changed_records": diff_result["changed_records"],
        "removed_records": diff_result["removed_records"],
        "normalization_warning_count": len(extraction_warnings),
        "missing_required_roles": missing_required_roles,
        "needs_review": bool(diff_result["needs_review"] or extraction_warnings or missing_required_roles),
        "required_smoke": [
            "search-remote returns docs chunks under /Wiki/sources",
            "read-node-context exposes /Sources/raw/ evidence links",
        ],
    }


def _coverage_summary(source: dict[str, object], payloads: list[dict[str, object]]) -> dict[str, object]:
    breakdown: dict[str, int] = {}
    role_breakdown: dict[str, int] = {}
    for payload in payloads:
        source_type = str(payload.get("source_type", "docs_site"))
        breakdown[source_type] = breakdown.get(source_type, 0) + 1
        coverage_role = str(payload.get("coverage_role") or "")
        if coverage_role:
            role_breakdown[coverage_role] = role_breakdown.get(coverage_role, 0) + 1
    target_roles = {
        str(target.get("coverage_role"))
        for target in source.get("crawl_targets", [])
        if target.get("coverage_role")
    }
    required_roles = REQUIRED_COVERAGE_ROLES | target_roles
    missing_required_roles = sorted(role for role in required_roles if role_breakdown.get(role, 0) == 0)
    return {
        "target_count": len(source.get("crawl_targets", [])),
        "fetched_url_count": len({row.get("upstream_url") for row in payloads if row.get("upstream_url")}),
        "normalized_chunk_count": len(payloads),
        "source_type_breakdown": dict(sorted(breakdown.items())),
        "coverage_role_breakdown": dict(sorted(role_breakdown.items())),
        "missing_required_roles": missing_required_roles,
    }


def _diff_report(diff_result: dict[str, object]) -> dict[str, object]:
    return {key: value for key, value in diff_result.items() if key != "record_hashes"}


def _normalized_rows(settings: Settings, source_id: str) -> list[dict[str, object]]:
    path = settings.normalized_dir / f"{slugify_source_id(source_id)}.jsonl"
    return load_jsonl(path)


def run_refresh(settings: Settings, *, source_id: str | None, dry_run: bool) -> dict[str, object]:
    sources = load_registry(settings)
    registry_errors = validate_registry(sources)
    if registry_errors:
        return {"run_at": utc_now(), "status": "invalid_registry", "errors": registry_errors, "sources": []}

    state = _load_state(settings)
    report = {"run_at": utc_now(), "status": "ok", "sources": []}
    cadence = None if source_id else "daily"
    selected_sources = select_sources(sources, source_id=source_id, cadence=cadence)
    if source_id and not selected_sources:
        return {
            "run_at": report["run_at"],
            "status": "invalid_source",
            "errors": [f"source not found or disabled: {source_id}"],
            "sources": [],
        }

    for source in selected_sources:
        item = {"source_id": source["source_id"], "status": "pending"}
        previous_snapshot = state["sources"].get(source["source_id"], {}).get("success_snapshot")
        collect_source(source, settings.http_timeout_seconds, settings.raw_dir)
        try:
            normalize_source(source, settings.raw_dir, settings.normalized_dir)
        except ValueError as error:
            item["status"] = "failed"
            item["errors"] = [str(error)]
            report["sources"].append(item)
            report["status"] = "partial"
            continue
        validation_errors = validate_source(source, settings.normalized_dir)
        if validation_errors:
            item["status"] = "failed"
            item["errors"] = validation_errors
            report["sources"].append(item)
            report["status"] = "partial"
            continue

        normalized_rows = _normalized_rows(settings, source["source_id"])
        diff_result = compute_diff(
            source,
            normalized_rows,
            state["sources"].get(source["source_id"]),
            settings,
        )
        normalization_meta = load_normalization_meta(settings.normalized_dir, source["source_id"])
        extraction_warnings = normalization_meta.get("warnings", [])
        if extraction_warnings:
            diff_result["needs_review"] = True
            diff_result["extraction_warning_count"] = len(extraction_warnings)
        item["coverage"] = _coverage_summary(source, normalized_rows)
        missing_required_roles = item["coverage"]["missing_required_roles"]
        if missing_required_roles:
            diff_result["needs_review"] = True
            diff_result["missing_required_roles"] = missing_required_roles
        item["diff"] = _diff_report(diff_result)
        item["quality_gates"] = _quality_gates(diff_result, extraction_warnings, missing_required_roles)
        if extraction_warnings:
            item["warnings"] = extraction_warnings
        if diff_result["status"] == "noop" and not diff_result["metadata_changed"] and not diff_result["needs_review"]:
            item["status"] = "noop"
            report["sources"].append(item)
            state["sources"][source["source_id"]] = _merge_state(
                state["sources"].get(source["source_id"]),
                diff_result,
            )
            continue

        stage_wiki_write = apply_wiki(source, settings, "staging", dry_run)
        stage_smoke = smoke_source(source, settings, "staging", dry_run)
        item["staging"] = {"wiki_write": stage_wiki_write, "smoke": stage_smoke}
        if any(step["status"] != "ok" for step in item["staging"].values()):
            item["status"] = "failed"
            report["sources"].append(item)
            report["status"] = "partial"
            continue

        if diff_result["needs_review"] or dry_run:
            item["status"] = "needs_review" if diff_result["needs_review"] else "dry_run"
            report["sources"].append(item)
            if diff_result["needs_review"]:
                report["status"] = "partial"
            continue

        prod_wiki_write = apply_wiki(source, settings, "prod", dry_run)
        prod_smoke = smoke_source(source, settings, "prod", dry_run)
        item["prod"] = {"wiki_write": prod_wiki_write, "smoke": prod_smoke}
        if any(step["status"] != "ok" for step in item["prod"].values()):
            item["rollback"] = _rollback_source(source, previous_snapshot, settings, dry_run)
            item["status"] = item["rollback"]["status"]
            report["sources"].append(item)
            report["status"] = "partial"
            continue

        item["status"] = "applied"
        diff_result["success_snapshot"] = _store_success_snapshot(settings, source)
        state["sources"][source["source_id"]] = _merge_state(
            state["sources"].get(source["source_id"]),
            diff_result,
        )
        report["sources"].append(item)

    state["last_run_at"] = report["run_at"]
    if not dry_run and report["status"] != "invalid_registry":
        dump_json(settings.state_path, state)
    _save_report(settings, report)
    return report


def main() -> int:
    parser = argparse.ArgumentParser(description="Run one source refresh cycle")
    parser.add_argument("--source", help="Only run one source_id")
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args()

    report = run_refresh(load_settings(), source_id=args.source, dry_run=args.dry_run)
    stream = sys.stdout if report["status"] == "ok" else sys.stderr
    print(json.dumps(report, indent=2), file=stream)
    return 0 if report["status"] == "ok" else 1


if __name__ == "__main__":
    raise SystemExit(main())
