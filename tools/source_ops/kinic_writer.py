# Where: tools/source_ops/kinic_writer.py
# What: Standard reset-and-reingest runner for source payload batches.
# Why: Preserve canonical payload JSON while giving source_ops a deterministic write and rollback path.
from __future__ import annotations

import argparse
import json
from pathlib import Path
from urllib.error import HTTPError, URLError
from urllib.request import Request, urlopen

if __package__ in {None, ""}:
    import sys

    sys.path.append(str(Path(__file__).resolve().parents[2]))
    from tools.source_ops.common import load_jsonl, run_command
    from tools.source_ops.config import load_settings
else:
    from .common import load_jsonl, run_command
    from .config import load_settings


def embedding_base_url() -> str:
    return __import__("os").environ.get("EMBEDDING_API_ENDPOINT", "https://api.kinic.io")


def embedding_input_text(payload: dict[str, object]) -> str:
    parts = [
        str(payload.get("title", "")).strip(),
        str(payload.get("snippet", "")).strip(),
        str(payload.get("content", "")).strip(),
    ]
    return "\n\n".join(part for part in parts if part)


def fetch_embedding(text: str) -> list[float]:
    request = Request(
        f"{embedding_base_url()}/embedding",
        data=json.dumps({"content": text}).encode("utf-8"),
        headers={"Content-Type": "application/json"},
        method="POST",
    )
    try:
        with urlopen(request, timeout=30) as response:
            payload = json.loads(response.read().decode("utf-8"))
            return [float(value) for value in payload["embedding"]]
    except (HTTPError, URLError, KeyError, ValueError) as error:
        raise RuntimeError(f"failed to fetch embedding: {error}") from error


def _format_float32_vec(embedding: list[float]) -> str:
    items = "; ".join(f"{value:.8g} : float32" for value in embedding)
    return f"vec {{ {items} }}"


def _format_insert_args(payload: dict[str, object], embedding: list[float]) -> str:
    return f"({_format_float32_vec(embedding)}, {json.dumps(json.dumps(payload, ensure_ascii=True, sort_keys=True))})"


def _reset_command(environment: str, identity: str, memory_id: str, dim: int) -> list[str]:
    return [
        "icp",
        "canister",
        "call",
        "-e",
        environment,
        "--identity",
        identity,
        memory_id,
        "reset",
        f"({dim} : nat)",
    ]


def _insert_command(environment: str, identity: str, memory_id: str, payload: dict[str, object], embedding: list[float]) -> list[str]:
    return [
        "icp",
        "canister",
        "call",
        "-e",
        environment,
        "--identity",
        identity,
        memory_id,
        "insert",
        _format_insert_args(payload, embedding),
    ]


def write_batch(environment: str, identity: str, memory_id: str, payload_path: Path, tag: str) -> dict[str, object]:
    settings = load_settings()
    payloads = load_jsonl(payload_path)
    if not payloads:
        raise ValueError(f"payload batch is empty: {payload_path}")

    results = [run_command(_reset_command(environment, identity, memory_id, settings.memory_reset_dim), timeout=settings.write_timeout_seconds)]
    for payload in payloads:
        payload = {**payload, "memory_tag": tag}
        embedding = fetch_embedding(embedding_input_text(payload))
        results.append(
            run_command(
                _insert_command(environment, identity, memory_id, payload, embedding),
                timeout=settings.write_timeout_seconds,
            )
        )
    failures = [result for result in results if result["exit_code"] != 0]
    return {
        "memory_id": memory_id,
        "payload_count": len(payloads),
        "status": "ok" if not failures else "failed",
        "results": results,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description="Reset and reingest a source payload batch into one memory canister")
    parser.add_argument("--env", required=True, help="icp environment, e.g. local or ic")
    parser.add_argument("--identity", required=True, help="icp identity name")
    parser.add_argument("--memory-id", required=True, help="target memory canister id")
    parser.add_argument("--payload-path", required=True, help="path to canonical payload JSONL")
    parser.add_argument("--tag", required=True, help="tag recorded with each payload")
    args = parser.parse_args()

    report = write_batch(args.env, args.identity, args.memory_id, Path(args.payload_path), args.tag)
    print(json.dumps(report, indent=2))
    return 0 if report["status"] == "ok" else 1


if __name__ == "__main__":
    raise SystemExit(main())
