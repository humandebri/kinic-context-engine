# Where: tools/source_ops/apply_wiki.py
# What: Apply normalized payloads to an existing Kinic Wiki database.
# Why: Keep writes on the existing wiki database instead of dedicated source backends.
from __future__ import annotations

import argparse
import json

if __package__ in {None, ""}:
    import sys
    from pathlib import Path

    sys.path.append(str(Path(__file__).resolve().parents[2]))
    from tools.source_ops.common import slugify_source_id
    from tools.source_ops.config import Settings, load_settings
    from tools.source_ops.kinic_writer import write_batch
    from tools.source_ops.registry import load_registry, select_sources
else:
    from .common import slugify_source_id
    from .config import Settings, load_settings
    from .kinic_writer import write_batch
    from .registry import load_registry, select_sources


def build_writer_commands(
    source: dict[str, object],
    settings: Settings,
    environment: str,
    *,
    payload_path_override: str | None = None,
    rollback: bool = False,
) -> list[list[str]]:
    database_id = getattr(settings, f"{environment}_database_id")
    if not database_id:
        raise ValueError(f"SOURCE_OPS_{environment.upper()}_DATABASE_ID is required")
    payload_path = payload_path_override or str(
        settings.normalized_dir / f"{slugify_source_id(source['source_id'])}.jsonl"
    )
    return [[
        "python3",
        "tools/source_ops/kinic_writer.py",
        "--env",
        environment,
        "--source-json",
        "<registry-entry>",
        "--payload-path",
        payload_path,
    ]]


def apply_wiki(
    source: dict[str, object],
    settings: Settings,
    environment: str,
    dry_run: bool,
    *,
    payload_path_override: str | None = None,
    rollback: bool = False,
) -> dict[str, object]:
    build_writer_commands(
        source,
        settings,
        environment,
        payload_path_override=payload_path_override,
        rollback=rollback,
    )
    return write_batch(
        source,
        settings,
        environment,
        dry_run,
        payload_path_override=payload_path_override,
        rollback=rollback,
    )


def main() -> int:
    parser = argparse.ArgumentParser(description="Apply normalized payloads to a Kinic Wiki database")
    parser.add_argument("--source", required=True, help="source_id to apply")
    parser.add_argument("--env", choices=["staging", "prod"], required=True)
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args()

    settings = load_settings()
    source = select_sources(load_registry(settings), source_id=args.source)[0]
    report = apply_wiki(source, settings, args.env, args.dry_run)
    print(json.dumps(report, indent=2))
    return 0 if report["status"] == "ok" else 1


if __name__ == "__main__":
    raise SystemExit(main())
