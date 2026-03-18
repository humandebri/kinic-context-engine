# Where: tools/source_ops/apply_memory.py
# What: Thin wrapper that delegates normalized payload writes to an external Kinic writer path.
# Why: Keep write-side concerns outside this repo while still letting automation orchestrate updates.
from __future__ import annotations

import argparse
import json

if __package__ in {None, ""}:
    import sys
    from pathlib import Path

    sys.path.append(str(Path(__file__).resolve().parents[2]))
    from tools.source_ops.common import run_command, slugify_source_id
    from tools.source_ops.config import Settings, load_settings
    from tools.source_ops.registry import load_registry, select_sources
else:
    from .common import run_command, slugify_source_id
    from .config import Settings, load_settings
    from .registry import load_registry, select_sources


def _latest_version(source: dict[str, object]) -> str:
    versions = source["catalog_metadata"].get("supported_versions", [])
    return versions[-1] if versions else "unversioned"


def build_writer_commands(
    source: dict[str, object],
    settings: Settings,
    environment: str,
    *,
    payload_path_override: str | None = None,
    rollback: bool = False,
) -> list[list[str]]:
    canister_ids = source["memory_targets"][f"{environment}_canister_ids"]
    if not canister_ids:
        raise ValueError(f"{source['source_id']}: no {environment} memory canister ids configured")
    template = settings.memory_rollback_template if rollback else settings.memory_writer_template
    if not template:
        raise ValueError(
            "SOURCE_OPS_MEMORY_ROLLBACK_TEMPLATE is required for rollback"
            if rollback
            else "SOURCE_OPS_MEMORY_WRITER_TEMPLATE is required for apply_memory"
        )

    payload_path = payload_path_override or str(
        settings.normalized_dir / f"{slugify_source_id(source['source_id'])}.jsonl"
    )
    tag = f"source:{source['source_id']}:{_latest_version(source)}"
    commands = []
    for memory_id in canister_ids:
        command = template.format(
            identity=settings.kinic_identity,
            memory_id=memory_id,
            payload_path=payload_path,
            source_id=source["source_id"],
            tag=tag,
            environment=environment,
        )
        commands.append(command)
    return commands


def apply_memory(
    source: dict[str, object],
    settings: Settings,
    environment: str,
    dry_run: bool,
    *,
    payload_path_override: str | None = None,
    rollback: bool = False,
) -> dict[str, object]:
    commands = build_writer_commands(
        source,
        settings,
        environment,
        payload_path_override=payload_path_override,
        rollback=rollback,
    )
    results = [
        run_command(command, dry_run=dry_run, timeout=settings.write_timeout_seconds)
        for command in commands
    ]
    failures = [result for result in results if result["exit_code"] != 0]
    return {
        "source_id": source["source_id"],
        "environment": environment,
        "rollback": rollback,
        "status": "ok" if not failures else "failed",
        "results": results,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description="Apply normalized payloads to memory canisters")
    parser.add_argument("--source", required=True, help="source_id to apply")
    parser.add_argument("--env", choices=["staging", "prod"], required=True)
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args()

    settings = load_settings()
    source = select_sources(load_registry(settings), source_id=args.source)[0]
    report = apply_memory(source, settings, args.env, args.dry_run)
    print(json.dumps(report, indent=2))
    return 0 if report["status"] == "ok" else 1


if __name__ == "__main__":
    raise SystemExit(main())
