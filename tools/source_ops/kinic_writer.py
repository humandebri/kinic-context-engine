# Where: tools/source_ops/kinic_writer.py
# What: Convert normalized source payloads into Kinic Wiki nodes and write them with kinic-vfs-cli.
# Why: Reuse the existing wiki canister API instead of maintaining catalog/source canisters.
from __future__ import annotations

import argparse
import json
import shlex
from pathlib import Path
from typing import Any

if __package__ in {None, ""}:
    import sys

    sys.path.append(str(Path(__file__).resolve().parents[2]))
    from tools.source_ops.common import ensure_dir, load_json, load_jsonl, run_command, slugify_source_id, write_text
    from tools.source_ops.config import Settings, load_settings
    from tools.source_ops.wiki_chunks import chunk_content, chunk_id, content_sha256, path_segment, record_segment
else:
    from .common import ensure_dir, load_json, load_jsonl, run_command, slugify_source_id, write_text
    from .config import Settings, load_settings
    from .wiki_chunks import chunk_content, chunk_id, content_sha256, path_segment, record_segment


WIKI_SOURCES_ROOT = "/Wiki/sources"
RAW_SOURCES_ROOT = "/Sources/raw"


def source_slug(source_id: str) -> str:
    return slugify_source_id(source_id)


def build_wiki_nodes(source: dict[str, Any], payloads: list[dict[str, Any]]) -> list[dict[str, Any]]:
    if not payloads:
        raise ValueError(f"payload batch is empty for {source['source_id']}")
    slug = source_slug(str(source["source_id"]))
    raw_path = f"{RAW_SOURCES_ROOT}/{slug}/{slug}.md"
    nodes = [
        {
            "path": raw_path,
            "kind": "source",
            "content": _raw_source_content(source),
            "metadata_json": _metadata_json(source, None),
        },
        {
            "path": f"{WIKI_SOURCES_ROOT}/{slug}/index.md",
            "kind": "file",
            "content": _source_index_content(source, raw_path),
            "metadata_json": _metadata_json(source, None),
        },
    ]
    seen_paths = {str(node["path"]) for node in nodes}
    seen_chunk_ids: set[str] = set()
    for index, payload in enumerate(payloads):
        version = _version(source, payload)
        section = str(payload.get("section") or "docs")
        section_index = int(payload.get("section_index", index))
        chunk_index = int(payload.get("chunk_index", index))
        content = chunk_content(payload)
        next_chunk_id = chunk_id(source, payload, section_index, chunk_index, content)
        if next_chunk_id in seen_chunk_ids:
            raise ValueError(f"duplicate wiki chunk_id for {source['source_id']}: {next_chunk_id}")
        seen_chunk_ids.add(next_chunk_id)
        record = record_segment(payload, index)
        path = (
            f"{WIKI_SOURCES_ROOT}/{slug}/{path_segment(version)}/"
            f"{record}-{path_segment(section)}-s{section_index:04}-c{chunk_index:04}.md"
        )
        if path in seen_paths:
            raise ValueError(f"duplicate wiki node path for {source['source_id']}: {path}")
        seen_paths.add(path)
        nodes.append(
            {
                "path": path,
                "kind": "file",
                "content": _document_content(payload, raw_path, content),
                "metadata_json": _metadata_json(
                    source,
                    payload,
                    section_index=section_index,
                    chunk_index=chunk_index,
                    content=content,
                    chunk_id=next_chunk_id,
                ),
            }
        )
    return nodes


def write_batch(
    source: dict[str, Any],
    settings: Settings,
    environment: str,
    dry_run: bool,
    *,
    payload_path_override: str | None = None,
    rollback: bool = False,
) -> dict[str, Any]:
    database_id = getattr(settings, f"{environment}_database_id")
    if not database_id:
        raise ValueError(f"SOURCE_OPS_{environment.upper()}_DATABASE_ID is required")
    payload_path = Path(payload_path_override) if payload_path_override else settings.normalized_dir / f"{source_slug(str(source['source_id']))}.jsonl"
    payloads = load_jsonl(payload_path)
    nodes = build_wiki_nodes(source, payloads)
    materialized = materialize_nodes(settings, environment, source, nodes)
    commands = [_write_node_command(settings, database_id, node) for node in materialized]
    results = [
        run_command(command, dry_run=dry_run, timeout=settings.write_timeout_seconds)
        for command in commands
    ]
    failures = [result for result in results if result["exit_code"] != 0]
    return {
        "source_id": source["source_id"],
        "environment": environment,
        "rollback": rollback,
        "database_id": database_id,
        "node_count": len(nodes),
        "status": "ok" if not failures else "failed",
        "results": results,
    }


def materialize_nodes(
    settings: Settings,
    environment: str,
    source: dict[str, Any],
    nodes: list[dict[str, Any]],
) -> list[dict[str, Any]]:
    root = ensure_dir(settings.wiki_nodes_dir / environment / source_slug(str(source["source_id"])))
    materialized = []
    for index, node in enumerate(nodes):
        content_path = root / f"{index:04}-{_file_stem(str(node['path']))}.md"
        write_text(content_path, str(node["content"]))
        materialized.append({**node, "content_path": str(content_path)})
    return materialized


def _write_node_command(settings: Settings, database_id: str, node: dict[str, Any]) -> list[str]:
    return [
        *shlex.split(settings.wiki_cli_bin),
        "--database-id",
        database_id,
        "write-node",
        "--path",
        str(node["path"]),
        "--kind",
        str(node["kind"]),
        "--input",
        str(node["content_path"]),
        "--metadata-json",
        str(node["metadata_json"]),
        "--json",
    ]


def _raw_source_content(source: dict[str, Any]) -> str:
    metadata = source["catalog_metadata"]
    lines = [
        f"# {metadata['title']}",
        "",
        f"- Source ID: `{source['source_id']}`",
        f"- Trust: `{metadata['trust']}`",
        f"- Domain: `{metadata['domain']}`",
        "",
        "## Citations",
    ]
    lines.extend(f"- {citation}" for citation in metadata.get("citations", []))
    lines.extend(["", "## Public URLs"])
    lines.extend(f"- [{item['label']}]({item['url']})" for item in source.get("public_urls", []))
    return "\n".join(lines).rstrip() + "\n"


def _source_index_content(source: dict[str, Any], raw_path: str) -> str:
    metadata = source["catalog_metadata"]
    aliases = ", ".join(metadata.get("aliases", []))
    versions = ", ".join(metadata.get("supported_versions", [])) or "unversioned"
    return "\n".join(
        [
            f"# {metadata['title']}",
            "",
            f"Source ID: `{source['source_id']}`",
            f"Aliases: {aliases}",
            f"Versions: {versions}",
            "",
            f"Raw source: [{raw_path}]({raw_path})",
            "",
        ]
    )


def _document_content(payload: dict[str, Any], raw_path: str, content: str) -> str:
    title = str(payload.get("title") or "Untitled")
    citation = str(payload.get("citation") or "")
    snippet = str(payload.get("snippet") or "")
    return "\n".join(
        [
            f"# {title}",
            "",
            f"Source evidence: [{raw_path}]({raw_path})",
            f"Canonical citation: {citation}",
            "",
            "## Summary",
            snippet,
            "",
            "## Content",
            content,
            "",
        ]
    )


def _metadata_json(
    source: dict[str, Any],
    payload: dict[str, Any] | None,
    *,
    section_index: int | None = None,
    chunk_index: int | None = None,
    content: str | None = None,
    chunk_id: str | None = None,
) -> str:
    metadata = source["catalog_metadata"]
    value = {
        "source_id": source["source_id"],
        "title": metadata["title"] if payload is None else payload.get("title", metadata["title"]),
        "trust": metadata["trust"],
        "domain": metadata["domain"],
        "version": None if payload is None else payload.get("version"),
        "supported_versions": metadata.get("supported_versions", []),
        "aliases": metadata.get("aliases", []),
        "tags": [] if payload is None else payload.get("tags", []),
        "citation": metadata.get("citations", [""])[0] if payload is None else payload.get("citation", ""),
        "citations": metadata.get("citations", []),
        "retrieved_at": metadata.get("retrieved_at", ""),
    }
    if payload is not None:
        value["chunk_id"] = chunk_id
        value["section_index"] = section_index
        value["chunk_index"] = chunk_index
        value["content_sha256"] = content_sha256(str(content or ""))
    return json.dumps(value, ensure_ascii=True, sort_keys=True)


def _version(source: dict[str, Any], payload: dict[str, Any]) -> str:
    value = str(payload.get("version") or "").strip()
    if value:
        return value
    versions = source["catalog_metadata"].get("supported_versions", [])
    return versions[-1] if versions else "unversioned"


def _file_stem(path: str) -> str:
    return path_segment(path.rsplit("/", 1)[-1].removesuffix(".md"))


def main() -> int:
    parser = argparse.ArgumentParser(description="Write normalized source payloads into a Kinic Wiki database")
    parser.add_argument("--source-json", required=True, help="path to one registry source JSON object")
    parser.add_argument("--env", choices=["staging", "prod"], required=True)
    parser.add_argument("--payload-path", required=True, help="path to canonical payload JSONL")
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args()

    settings = load_settings()
    source = load_json(Path(args.source_json))
    report = write_batch(
        source,
        settings,
        args.env,
        args.dry_run,
        payload_path_override=args.payload_path,
    )
    print(json.dumps(report, indent=2))
    return 0 if report["status"] == "ok" else 1


if __name__ == "__main__":
    raise SystemExit(main())
