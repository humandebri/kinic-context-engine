# Source Tools

This directory is intentionally separate from `src/`.

`kinic-context-cli` remains a read-only retrieval CLI. Source data preparation, validation, and ingestion are separate responsibilities because they mutate an existing Kinic Wiki database.

## Purpose

Use the documents in this folder to standardize how public documentation sources are collected, normalized, and written as Wiki nodes so that `query`, `pack`, and `cite` return stable titles, citations, and versions.

## Scope

This folder defines:

- canonical payload schema for source chunks
- build / validate / wiki write workflow
- `source_ops/` automation entrypoints for collection, diffing, apply, and smoke

This repo does not deploy a new backend. Runtime storage and search use the existing Kinic Wiki database through `kinic-vfs-cli`.

## v1 Source Set

The registry in `tools/source_ops/registry.yaml` is the source list. Each logical source is represented by:

- source metadata stored in the registry
- raw source and docs chunk Wiki nodes
- canonical identity in each node `metadata_json`

## Existing Write Path

The current write path is:

- collect upstream docs with `tools/source_ops/collect.py`
- normalize payload JSONL with `tools/source_ops/normalize.py`
- validate payloads with `tools/source_ops/validate.py`
- write Wiki nodes with `tools/source_ops/apply_wiki.py`
- verify read behavior with `tools/source_ops/smoke.py`
