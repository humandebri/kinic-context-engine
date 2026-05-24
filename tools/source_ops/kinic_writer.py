# Where: tools/source_ops/kinic_writer.py
# What: Standard reset-and-reingest runner for source payload batches.
# Why: Use canonical typed payload fields while giving source_ops a deterministic write and rollback path.
from __future__ import annotations

import argparse
import json
from pathlib import Path

if __package__ in {None, ""}:
    import sys

    sys.path.append(str(Path(__file__).resolve().parents[2]))
    from tools.source_ops.common import load_jsonl, run_command
    from tools.source_ops.config import load_settings
    from tools.source_ops.embedding import document_input_text, fetch_embedding, section_input_text
else:
    from .common import load_jsonl, run_command
    from .config import load_settings
    from .embedding import document_input_text, fetch_embedding, section_input_text


def _format_float32_vec(embedding: list[float]) -> str:
    items = "; ".join(f"{value:.8g} : float32" for value in embedding)
    return f"vec {{ {items} }}"


def _format_insert_args(payload: dict[str, object], embedding: list[float]) -> str:
    version = payload.get("version")
    section = payload.get("section")
    tags = payload.get("tags") or []
    tag_items = "; ".join(json.dumps(str(tag)) for tag in tags)
    version_text = "null" if version in {None, ""} else f'opt {json.dumps(str(version))}'
    section_text = "null" if section in {None, ""} else f'opt {json.dumps(str(section))}'
    return (
        "(record { "
        f'title = {json.dumps(str(payload.get("title", "")))}; '
        f'snippet = {json.dumps(str(payload.get("snippet", "")))}; '
        f'citation = {json.dumps(str(payload.get("citation", "")))}; '
        f"version = {version_text}; "
        f'content = {json.dumps(str(payload.get("content", "")))}; '
        f"section = {section_text}; "
        f"tags = vec {{ {tag_items} }}; "
        f"embedding = {_format_float32_vec(embedding)} "
        "})"
    )


def _format_insert_section_args(section: dict[str, object], embedding: list[float]) -> str:
    version = section.get("version")
    version_text = "null" if version in {None, ""} else f'opt {json.dumps(str(version))}'
    return (
        "(record { "
        f'section_id = {json.dumps(str(section["section_id"]))}; '
        f'title = {json.dumps(str(section["title"]))}; '
        f'summary = {json.dumps(str(section["summary"]))}; '
        f"version = {version_text}; "
        f"embedding = {_format_float32_vec(embedding)} "
        "})"
    )


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
        "insert_document",
        _format_insert_args(payload, embedding),
    ]


def _insert_section_command(
    environment: str,
    identity: str,
    memory_id: str,
    section: dict[str, object],
    embedding: list[float],
) -> list[str]:
    return [
        "icp",
        "canister",
        "call",
        "-e",
        environment,
        "--identity",
        identity,
        memory_id,
        "insert_section",
        _format_insert_section_args(section, embedding),
    ]


def build_sections(payloads: list[dict[str, object]]) -> list[dict[str, object]]:
    grouped: dict[tuple[str, str], dict[str, object]] = {}
    for payload in payloads:
        section_id = str(payload.get("section", "")).strip()
        if not section_id:
            continue
        version = str(payload.get("version", "")).strip()
        key = (section_id, version)
        bucket = grouped.setdefault(
            key,
            {
                "section_id": section_id,
                "title": section_id.replace("-", " ").replace("_", " ").title(),
                "version": version or None,
                "snippets": [],
                "citations": [],
            },
        )
        snippet = str(payload.get("snippet", "")).strip()
        citation = str(payload.get("citation", "")).strip()
        title = str(payload.get("title", "")).strip()
        if title and bucket["title"] == section_id.replace("-", " ").replace("_", " ").title():
            bucket["title"] = title
        if snippet and snippet not in bucket["snippets"]:
            bucket["snippets"].append(snippet)
        if citation and citation not in bucket["citations"]:
            bucket["citations"].append(citation)

    sections = []
    for item in grouped.values():
        summary_parts = list(item["snippets"][:3])
        if item["citations"]:
            summary_parts.append(f"References: {', '.join(item['citations'][:2])}")
        sections.append(
            {
                "section_id": item["section_id"],
                "title": item["title"],
                "summary": "\n\n".join(summary_parts).strip(),
                "version": item["version"],
            }
        )
    sections.sort(key=lambda item: (item["section_id"], item["version"] or ""))
    return sections


def write_batch(environment: str, identity: str, memory_id: str, payload_path: Path, tag: str) -> dict[str, object]:
    settings = load_settings()
    payloads = load_jsonl(payload_path)
    if not payloads:
        raise ValueError(f"payload batch is empty: {payload_path}")
    sections = build_sections(payloads)

    results = [run_command(_reset_command(environment, identity, memory_id, settings.memory_reset_dim), timeout=settings.write_timeout_seconds)]
    for section in sections:
        embedding = fetch_embedding(
            section_input_text(section["section_id"], section["title"], section["summary"]),
            kind="section",
        )
        results.append(
            run_command(
                _insert_section_command(environment, identity, memory_id, section, embedding),
                timeout=settings.write_timeout_seconds,
            )
        )
    for payload in payloads:
        payload = {**payload, "memory_tag": tag}
        embedding = fetch_embedding(document_input_text(payload), kind="document")
        results.append(
            run_command(
                _insert_command(environment, identity, memory_id, payload, embedding),
                timeout=settings.write_timeout_seconds,
            )
        )
    failures = [result for result in results if result["exit_code"] != 0]
    return {
        "memory_id": memory_id,
        "section_count": len(sections),
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
