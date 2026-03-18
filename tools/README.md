# Source Tools

This directory is intentionally separate from `src/`.

`kinic-context-cli` remains a read-only retrieval CLI. Source data preparation, validation, and ingestion are separate responsibilities because they mutate memory instance contents and require operational tooling.

## Purpose

Use the documents in this folder to standardize how source-oriented memory instances are populated so that `query`, `pack`, and `cite` can return stable titles, citations, and versions.

## Scope

This folder defines:

- the canonical payload schema for source chunks
- the build / validate / ingest workflow
- the expected future helper tool names
- the `source_ops/` automation entrypoints for collection, diffing, apply, and smoke

This folder does not add a new ingest CLI. Use existing `kinic-cli` or `kinic-py` to write into memory instances.

## v1 Source Set

The initial source logical IDs are fixed to:

- `/vercel/next.js`
- `/supabase/docs`
- `/react/docs`
- `/skills/nextjs/migration`

Each logical source maps to memory instance canister IDs through the catalog canister, not a local source map env var.

## Planned Helper Tools

These names are reserved for future implementation:

- `tools/build_source_payloads.*`
- `tools/validate_source_payloads.*`
- `tools/ingest_source_payloads.*`

They should remain outside the read-only CLI binary and should not be linked into `src/`.

## Existing Write Path

The current write path is expected to use one of:

- `kinic-cli insert`
- `kinic-py` / `KinicMemories.insert_markdown`

The helper tools in this folder should prepare and validate payloads, then hand them to the existing write APIs.
