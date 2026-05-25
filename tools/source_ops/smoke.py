# Where: tools/source_ops/smoke.py
# What: Read-path smoke checks for wiki-backed source refreshes.
# Why: Block promotion when existing Kinic Wiki search/context paths regress.
from __future__ import annotations

import argparse
import json
import shlex

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


def _cli_env(settings: Settings, environment: str) -> dict[str, str]:
    return {
        "VFS_DATABASE_ID": getattr(settings, f"{environment}_database_id"),
    }


def _command(settings: Settings, args: list[str]) -> list[str]:
    return shlex.split(settings.wiki_cli_bin) + args


def _source_slug(source_id: object) -> str:
    return slugify_source_id(str(source_id))


def smoke_source(source: dict[str, object], settings: Settings, environment: str, dry_run: bool) -> dict[str, object]:
    env = _cli_env(settings, environment)
    queries = source["smoke_queries"]
    search = run_command(
        _command(settings, ["search-remote", queries["query"], "--prefix", "/Wiki/sources", "--json"]),
        env=env,
        dry_run=dry_run,
    )
    context = run_command(
        _command(
            settings,
            [
                "read-node-context",
                "--path",
                f"/Wiki/sources/{_source_slug(source['source_id'])}/index.md",
                "--link-limit",
                "20",
                "--json",
            ],
        ),
        env=env,
        dry_run=dry_run,
    )

    if dry_run:
        return {"source_id": source["source_id"], "environment": environment, "status": "ok", "checks": [search, context]}

    failures = []
    search_json = json.loads(search["stdout"]) if search["exit_code"] == 0 else []
    context_json = json.loads(context["stdout"]) if context["exit_code"] == 0 else {}
    if search["exit_code"] != 0 or not search_json:
        failures.append("search")
    if context["exit_code"] != 0 or context_json.get("node", {}).get("path") is None:
        failures.append("read-node-context")
    if context["exit_code"] != 0 or not any(
        str(item.get("target_path", "")).startswith("/Sources/raw/")
        for item in context_json.get("outgoing_links", [])
    ):
        failures.append("source-evidence")

    return {
        "source_id": source["source_id"],
        "environment": environment,
        "status": "ok" if not failures else "failed",
        "failures": failures,
        "checks": [search, context],
    }


def main() -> int:
    parser = argparse.ArgumentParser(description="Run read-path smoke checks")
    parser.add_argument("--source", required=True, help="source_id to smoke test")
    parser.add_argument("--env", choices=["staging", "prod"], required=True)
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args()

    settings = load_settings()
    source = select_sources(load_registry(settings), source_id=args.source)[0]
    report = smoke_source(source, settings, args.env, args.dry_run)
    print(json.dumps(report, indent=2))
    return 0 if report["status"] == "ok" else 1


if __name__ == "__main__":
    raise SystemExit(main())
