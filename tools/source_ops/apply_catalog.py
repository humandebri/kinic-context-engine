# Where: tools/source_ops/apply_catalog.py
# What: Catalog update wrapper that applies one source via admin_upsert_source.
# Why: Keep source metadata and memory mappings synchronized after successful staging/prod writes.
from __future__ import annotations

import argparse
import json

if __package__ in {None, ""}:
    import sys
    from pathlib import Path

    sys.path.append(str(Path(__file__).resolve().parents[2]))
    from tools.source_ops.common import run_command
    from tools.source_ops.config import Settings, load_settings
    from tools.source_ops.registry import load_registry, select_sources
else:
    from .common import run_command
    from .config import Settings, load_settings
    from .registry import load_registry, select_sources


def _candid_text(value: str) -> str:
    return json.dumps(value, ensure_ascii=True)


def _vec_text(values: list[str]) -> str:
    return "vec {" + "; ".join(_candid_text(value) for value in values) + "}"


def _opt_text(value: str | None) -> str:
    return "null" if value is None else f"opt {_candid_text(value)}"


def build_upsert_args(source: dict[str, object], environment: str) -> str:
    metadata = source["catalog_metadata"]
    canister_ids = source["memory_targets"][f"{environment}_canister_ids"]
    return (
        "(record { "
        f"source_id = {_candid_text(source['source_id'])}; "
        f"title = {_candid_text(metadata['title'])}; "
        f"aliases = {_vec_text(metadata['aliases'])}; "
        f"trust = {_candid_text(metadata['trust'])}; "
        f"domain = {_candid_text(metadata['domain'])}; "
        f"skill_kind = {_opt_text(metadata.get('skill_kind'))}; "
        f"targets = {_vec_text(metadata.get('targets', []))}; "
        f"capabilities = {_vec_text(metadata.get('capabilities', []))}; "
        f"canister_ids = {_vec_text(canister_ids)}; "
        f"supported_versions = {_vec_text(metadata.get('supported_versions', []))}; "
        f"retrieved_at = {_candid_text(metadata.get('retrieved_at', '2026-03-18T00:00:00Z'))}; "
        f"citations = {_vec_text(metadata.get('citations', []))}; "
        "})"
    )


def apply_catalog(
    source: dict[str, object],
    settings: Settings,
    environment: str,
    dry_run: bool,
) -> dict[str, object]:
    catalog_id = getattr(settings, f"{environment}_catalog_canister_id")
    icp_environment = getattr(settings, f"{environment}_icp_environment")
    if not catalog_id:
        raise ValueError(f"SOURCE_OPS_{environment.upper()}_CATALOG_CANISTER_ID is required")

    command = [
        "icp",
        "canister",
        "call",
        "-e",
        icp_environment,
        catalog_id,
        "admin_upsert_source",
        build_upsert_args(source, environment),
    ]
    result = run_command(command, dry_run=dry_run, timeout=settings.write_timeout_seconds)
    return {
        "source_id": source["source_id"],
        "environment": environment,
        "status": "ok" if result["exit_code"] == 0 else "failed",
        "result": result,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description="Apply one source to the catalog canister")
    parser.add_argument("--source", required=True, help="source_id to apply")
    parser.add_argument("--env", choices=["staging", "prod"], required=True)
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args()

    settings = load_settings()
    source = select_sources(load_registry(settings), source_id=args.source)[0]
    report = apply_catalog(source, settings, args.env, args.dry_run)
    print(json.dumps(report, indent=2))
    return 0 if report["status"] == "ok" else 1


if __name__ == "__main__":
    raise SystemExit(main())
