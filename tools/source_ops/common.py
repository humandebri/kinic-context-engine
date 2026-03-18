# Where: tools/source_ops/common.py
# What: Shared filesystem, JSON, hashing, and subprocess helpers for source automation.
# Why: Keep the operational scripts small, deterministic, and dependency-free.
from __future__ import annotations

import hashlib
import json
import os
import re
import shlex
import subprocess
from datetime import UTC, datetime
from html import unescape
from pathlib import Path
from typing import Any


ROOT_DIR = Path(__file__).resolve().parents[2]
TOOLS_DIR = ROOT_DIR / "tools"
SOURCE_OPS_DIR = TOOLS_DIR / "source_ops"


def utc_now() -> str:
    return datetime.now(UTC).replace(microsecond=0).isoformat().replace("+00:00", "Z")


def slugify_source_id(source_id: str) -> str:
    return source_id.strip("/").replace("/", "__").replace(".", "_") or "root"


def ensure_dir(path: Path) -> Path:
    path.mkdir(parents=True, exist_ok=True)
    return path


def read_text(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def write_text(path: Path, value: str) -> None:
    ensure_dir(path.parent)
    path.write_text(value, encoding="utf-8")


def load_json(path: Path) -> Any:
    return json.loads(read_text(path))


def dump_json(path: Path, value: Any) -> None:
    write_text(path, json.dumps(value, indent=2, ensure_ascii=True, sort_keys=True) + "\n")


def load_yaml_like_json(path: Path) -> Any:
    # Registry files are stored as JSON-compatible YAML so stdlib json is sufficient.
    return json.loads(read_text(path))


def load_jsonl(path: Path) -> list[dict[str, Any]]:
    if not path.exists():
        return []
    lines = [line for line in read_text(path).splitlines() if line.strip()]
    return [json.loads(line) for line in lines]


def dump_jsonl(path: Path, rows: list[dict[str, Any]]) -> None:
    lines = [json.dumps(row, ensure_ascii=True, sort_keys=True) for row in rows]
    write_text(path, "\n".join(lines) + ("\n" if lines else ""))


def canonical_hash(value: Any) -> str:
    payload = json.dumps(value, ensure_ascii=True, sort_keys=True, separators=(",", ":"))
    return hashlib.sha256(payload.encode("utf-8")).hexdigest()


def clean_text(value: str) -> str:
    no_tags = re.sub(r"<[^>]+>", " ", value)
    normalized = re.sub(r"\s+", " ", unescape(no_tags)).strip()
    return normalized


def summarize_text(value: str, limit: int = 240) -> str:
    text = clean_text(value)
    if len(text) <= limit:
        return text
    return text[: limit - 3].rstrip() + "..."


def run_command(
    command: list[str] | str,
    *,
    cwd: Path | None = None,
    env: dict[str, str] | None = None,
    dry_run: bool = False,
    timeout: int = 120,
) -> dict[str, Any]:
    argv = shlex.split(command) if isinstance(command, str) else command
    if dry_run:
        return {
            "command": argv,
            "exit_code": 0,
            "stdout": "",
            "stderr": "",
            "dry_run": True,
        }

    completed = subprocess.run(
        argv,
        cwd=cwd or ROOT_DIR,
        env={**os.environ, **(env or {})},
        capture_output=True,
        text=True,
        timeout=timeout,
        check=False,
    )
    return {
        "command": argv,
        "exit_code": completed.returncode,
        "stdout": completed.stdout,
        "stderr": completed.stderr,
        "dry_run": False,
    }
