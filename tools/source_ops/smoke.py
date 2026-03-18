# Where: tools/source_ops/smoke.py
# What: Read-path smoke checks for staging and production source refreshes.
# Why: Block promotion when resolve/query/pack regress after source updates.
from __future__ import annotations

import argparse
import json
import shlex

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


def _cli_env(settings: Settings, environment: str) -> dict[str, str]:
    return {
        "KINIC_CONTEXT_CATALOG_CANISTER_ID": getattr(settings, f"{environment}_catalog_canister_id"),
        "KINIC_CONTEXT_IC_HOST": getattr(settings, f"{environment}_ic_host"),
        "KINIC_CONTEXT_FETCH_ROOT_KEY": "true"
        if getattr(settings, f"{environment}_fetch_root_key")
        else "false",
    }


def _command(settings: Settings, args: list[str]) -> list[str]:
    return shlex.split(settings.cli_bin) + args


def smoke_source(source: dict[str, object], settings: Settings, environment: str, dry_run: bool) -> dict[str, object]:
    env = _cli_env(settings, environment)
    queries = source["smoke_queries"]
    resolve = run_command(
        _command(settings, ["resolve", queries["resolve"]]),
        env=env,
        dry_run=dry_run,
    )
    query = run_command(
        _command(settings, ["query", source["source_id"], queries["query"]]),
        env=env,
        dry_run=dry_run,
    )
    pack = run_command(
        _command(settings, ["pack", queries["pack"]]),
        env=env,
        dry_run=dry_run,
    )

    if dry_run:
        return {"source_id": source["source_id"], "environment": environment, "status": "ok", "checks": [resolve, query, pack]}

    failures = []
    resolve_json = json.loads(resolve["stdout"]) if resolve["exit_code"] == 0 else {}
    query_json = json.loads(query["stdout"]) if query["exit_code"] == 0 else {}
    pack_json = json.loads(pack["stdout"]) if pack["exit_code"] == 0 else {}
    if resolve["exit_code"] != 0 or source["source_id"] not in [item["source_id"] for item in resolve_json.get("candidate_sources", [])]:
        failures.append("resolve")
    if query["exit_code"] != 0 or not query_json.get("snippets"):
        failures.append("query")
    if query_json.get("snippets"):
        snippet = query_json["snippets"][0]
        if not str(snippet.get("citation", "")).startswith(("http://", "https://", "file://")):
            failures.append("query-citation")
    if pack["exit_code"] != 0 or not pack_json.get("evidence"):
        failures.append("pack")

    return {
        "source_id": source["source_id"],
        "environment": environment,
        "status": "ok" if not failures else "failed",
        "failures": failures,
        "checks": [resolve, query, pack],
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
