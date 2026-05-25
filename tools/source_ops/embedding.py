# Where: tools/source_ops/embedding.py
# What: Shared remote/local embedding adapter for source_ops.
# Why: Reuse the Rust ONNX helper so Python write paths do not own model inference logic.
from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
from pathlib import Path
from typing import Any
from urllib.error import HTTPError, URLError
from urllib.request import Request, urlopen

DEFAULT_MODEL = "intfloat/multilingual-e5-large"
DEFAULT_QUERY_PREFIX = "query: "
DEFAULT_PASSAGE_PREFIX = "passage: "
ALLOWED_KINDS = {"query", "document", "section"}


def remote_embedding_endpoint() -> str | None:
    value = os.environ.get("EMBEDDING_API_ENDPOINT", "").strip()
    return value or None


def embedding_model() -> str:
    return os.environ.get("KINIC_CONTEXT_EMBEDDING_MODEL", DEFAULT_MODEL)


def embedding_model_dir() -> str | None:
    value = os.environ.get("KINIC_CONTEXT_EMBEDDING_MODEL_DIR", "").strip()
    return value or None


def helper_binary() -> str:
    value = os.environ.get("KINIC_CONTEXT_EMBEDDING_HELPER", "").strip()
    if value:
        return value
    root = Path(__file__).resolve().parents[2]
    return str(root / "target" / "debug" / "kinic-embed")


def query_prefix() -> str:
    return os.environ.get("KINIC_CONTEXT_EMBEDDING_QUERY_PREFIX", DEFAULT_QUERY_PREFIX)


def document_prefix() -> str:
    return os.environ.get("KINIC_CONTEXT_EMBEDDING_DOCUMENT_PREFIX", DEFAULT_PASSAGE_PREFIX)


def apply_prefix(prefix: str, text: str) -> str:
    return f"{prefix}{text.strip()}"


def query_input_text(text: str) -> str:
    return apply_prefix(query_prefix(), text)


def document_input_text(payload: dict[str, object]) -> str:
    parts = [
        apply_prefix(document_prefix(), str(payload.get("title", ""))),
        str(payload.get("snippet", "")).strip(),
        str(payload.get("content", "")).strip(),
    ]
    return "\n\n".join(part for part in parts if part)


def section_input_text(section_id: str, title: str, summary: str) -> str:
    parts = [
        apply_prefix(document_prefix(), title or section_id),
        summary.strip(),
    ]
    return "\n\n".join(part for part in parts if part)


def fetch_embedding(text: str, kind: str = "document") -> list[float]:
    if kind not in ALLOWED_KINDS:
        raise RuntimeError(f"unsupported embedding kind: {kind}")
    endpoint = remote_embedding_endpoint()
    if endpoint:
        return _fetch_remote_embedding(endpoint, text)
    return _run_local_helper(text, kind)


def encode_payload(payload: dict[str, Any]) -> dict[str, Any]:
    kind = str(payload.get("kind", "")).strip()
    text = str(payload.get("text", ""))
    if kind not in ALLOWED_KINDS:
        raise RuntimeError(f"unsupported embedding kind: {kind}")
    return {"embedding": fetch_embedding(text, kind=kind), "model": embedding_model()}


def _fetch_remote_embedding(endpoint: str, text: str) -> list[float]:
    request = Request(
        f"{endpoint}/embedding",
        data=json.dumps({"content": text, "model": embedding_model()}).encode("utf-8"),
        headers={"Content-Type": "application/json"},
        method="POST",
    )
    try:
        with urlopen(request, timeout=30) as response:
            payload = json.loads(response.read().decode("utf-8"))
            return [float(value) for value in payload["embedding"]]
    except (HTTPError, URLError, KeyError, ValueError) as error:
        raise RuntimeError(f"failed to fetch embedding: {error}") from error


def _run_local_helper(text: str, kind: str) -> list[float]:
    helper = helper_binary()
    env = os.environ.copy()
    if not env.get("KINIC_CONTEXT_EMBEDDING_MODEL_DIR"):
        env["KINIC_CONTEXT_EMBEDDING_MODEL_DIR"] = str(
            Path(__file__).resolve().parents[2] / ".local" / "models" / "multilingual-e5-large"
        )
    try:
        completed = subprocess.run(
            [helper],
            input=json.dumps({"kind": kind, "text": text}),
            text=True,
            capture_output=True,
            check=False,
            env=env,
        )
    except OSError as error:
        raise RuntimeError(f"failed to execute local embedding helper `{helper}`: {error}") from error
    if completed.returncode != 0:
        message = completed.stderr.strip() or "unknown local embedding helper error"
        raise RuntimeError(f"local embedding helper failed: {message}")
    try:
        payload = json.loads(completed.stdout)
        return [float(value) for value in payload["embedding"]]
    except (KeyError, ValueError, json.JSONDecodeError) as error:
        raise RuntimeError(f"failed to decode local embedding helper response: {error}") from error


def _read_stdin_payload() -> dict[str, Any]:
    raw = sys.stdin.read()
    if not raw.strip():
        raise RuntimeError("stdin JSON payload is required")
    try:
        return json.loads(raw)
    except json.JSONDecodeError as error:
        raise RuntimeError(f"failed to decode stdin JSON: {error}") from error


def main() -> int:
    parser = argparse.ArgumentParser(description="Source ops embedding adapter")
    parser.add_argument("--mode", choices=sorted(ALLOWED_KINDS), required=True)
    parser.add_argument("--stdin-json", action="store_true", help="Read {kind,text} payload from stdin")
    args = parser.parse_args()

    try:
        payload = _read_stdin_payload() if args.stdin_json else {"kind": args.mode, "text": sys.stdin.read()}
        payload["kind"] = args.mode
        print(json.dumps(encode_payload(payload)))
        return 0
    except RuntimeError as error:
        print(str(error), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
