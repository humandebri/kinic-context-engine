# Source Ingest Spec

## Objective

Populate source-specific source/memory canisters with canonical JSON payloads so that the read-only CLI can produce stable evidence packs and citations.

The ingest flow is intentionally external to `kinic-context-cli`.

## Source-to-Instance Model

The operational model is fixed for v1:

- one logical source ID
- one dedicated source/memory canister
- one consistent payload schema

v1 source set:

- `/vercel/next.js`
- `/supabase/docs`
- `/react/docs`

The runtime mapping is provided by the catalog canister.

## Workflow

### 1. Build

Input:

- raw docs text
- crawled HTML/PDF extraction
- manual source notes if needed

Output:

- one JSON payload per chunk
- every payload follows the canonical schema in `tools/source_payload_schema.md`

Planned helper name:

- `tools/build_source_payloads.*`

Responsibilities:

- assign correct `source_id`
- choose stable `title`
- generate concise `snippet`
- preserve canonical `citation`
- attach `version`
- include full `content` when available

### 2. Validate

Input:

- generated payload collection

Output:

- pass/fail report

Planned helper name:

- `tools/validate_source_payloads.*`

Validation checks:

- payload is valid JSON
- required keys exist
- `citation` is absolute URL
- `source_id` matches target source
- `version` exists for v1 sources
- optional `content`, `section`, `tags`, `retrieved_at` are type-correct if present

Failure policy:

- do not ingest invalid payloads
- fail the batch if any required field is missing

### 3. Ingest

Input:

- validated payload collection
- target source/memory canister ID

Output:

- section summaries and payloads written to the existing source/memory canister

Planned helper name:

- `tools/ingest_source_payloads.*`

Write path:

- use existing `kinic-cli insert`
- or use existing `kinic-py` / `KinicMemories.insert_markdown`

No new write CLI is introduced in this repo.

## Ingest Contract

### Per-payload write behavior

- write one canonical JSON object as the stored text payload
- do not flatten to plain text before insert
- the searchable text should remain part of the JSON, typically through `snippet` and `content`

### Section summary behavior

- group payloads by `section` and `version`
- derive one section summary per group before document inserts
- call `insert_section` before `insert_document`
- use the same embedding model and prefix rule as document indexing

### Tagging

If the write API requires a tag, use a deterministic tag per source/version pair.

Recommended pattern:

- `source:<source_id>:<version>`

Examples:

- `source:/vercel/next.js:15`
- `source:/supabase/docs:2026`

## Operational Rules

### Re-ingestion

- re-ingestion should replace or supersede older payload batches in a controlled way
- do not mix old and new schema shapes inside the same source instance

### Source purity

- do not insert `/supabase/docs` payloads into the `/vercel/next.js` instance
- do not insert unversioned chunks into versioned source instances unless the source truly has no version semantics

### Citation quality

- page-level URLs are acceptable for v1
- section/deep-link URLs are preferred

## Expected Future Tool Behavior

When implemented, helper tools should:

- accept source logical ID as first-class input
- validate against the canonical schema before any write call
- emit deterministic JSON artifacts for review
- call existing `kinic-cli` or `kinic-py` only after validation succeeds

## Acceptance Criteria

The source ingest setup is considered ready when:

- a source payload batch passes validation
- the batch is inserted into the intended source/memory canister
- `kinic-context-cli query <source_id> ...` returns stable `title`, `citation`, and `version`
- `pack` can merge multiple sources without falling back to `memory://<source_id>` for curated payloads
