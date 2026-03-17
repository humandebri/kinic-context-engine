# Source Ingest Spec

## Objective

Populate source-specific memory instances with canonical JSON payloads so that the read-only CLI can produce stable evidence packs and citations.

The ingest flow is intentionally external to `kinic-context-cli`.

## Source-to-Instance Model

The operational model is fixed for v1:

- one logical source ID
- one dedicated memory instance canister
- one consistent payload schema

v1 source set:

- `/vercel/next.js`
- `/supabase/docs`
- `/react/docs`
- `/skills/nextjs/migration`

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
- for skill sources, summarize decision rules in `snippet`

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
- skill sources may omit `version`

Failure policy:

- do not ingest invalid payloads
- fail the batch if any required field is missing

### 3. Ingest

Input:

- validated payload collection
- target memory instance canister ID

Output:

- payloads written to the existing memory instance

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
- for skill sources, set catalog `domain` to `skill_knowledge`

### Citation quality

- page-level URLs are acceptable for v1
- section/deep-link URLs are preferred
- skill payloads should cite the canonical repo URL for the skill origin, not an ad hoc temporary note

Recommended skill citation example:

- `https://github.com/<org>/<repo>/blob/<ref>/skills/<skill-name>/SKILL.md`

## Expected Future Tool Behavior

When implemented, helper tools should:

- accept source logical ID as first-class input
- validate against the canonical schema before any write call
- emit deterministic JSON artifacts for review
- call existing `kinic-cli` or `kinic-py` only after validation succeeds

## Acceptance Criteria

The source ingest setup is considered ready when:

- a source payload batch passes validation
- the batch is inserted into the intended memory instance
- `kinic-context-cli query <source_id> ...` returns stable `title`, `citation`, and `version`
- `pack` can merge multiple sources without falling back to `memory://<source_id>` for curated payloads
