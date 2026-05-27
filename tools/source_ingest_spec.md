# Source Ingest Spec

## Objective

Populate an existing Kinic Wiki database with canonical docs payloads so that the read-only CLI can produce stable evidence packs and citations.

The ingest flow is intentionally external to `kinic-context-cli`.

## Source Model

The operational model is fixed:

- one logical source ID
- one registry entry in `tools/source_ops/registry.yaml`
- one raw source Wiki node
- one source index Wiki node
- one docs chunk Wiki node per normalized payload

The Wiki node `metadata_json` is the canonical runtime identity for source and chunk metadata.

## Workflow

### 1. Register

Source registration uses `tools/source_ops/register_source.py`.

Each source must define crawl targets with:

- `source_type`
- `crawl_strategy`
- `coverage_role`
- `max_pages`
- include/exclude prefixes

Required v1 coverage roles are `overview`, `api_reference`, and `examples`.

### 2. Build

Input:

- upstream docs URLs
- llms-full files
- sitemap pages
- GitHub Markdown/MDX trees

Output:

- canonical JSONL payloads under `tools/source_ops/artifacts/normalized/`
- every payload follows `tools/source_payload_schema.md`

Responsibilities:

- assign correct `source_id`
- preserve canonical `citation`
- attach `source_type`, `target_label`, `coverage_role`, and `upstream_url`
- generate deterministic section/chunk indices

### 3. Validate

Validation uses `tools/source_ops/validate.py`.

Checks:

- payload is valid JSON
- required keys exist
- `citation` is absolute
- `source_id` matches the selected registry source
- versioned sources include `version`
- long `content` is not duplicated as `snippet`

Invalid payload batches are not written.

### 4. Wiki Write

Wiki writing uses `tools/source_ops/apply_wiki.py`, which delegates to `tools/source_ops/kinic_writer.py`.

Write path:

- materialize a single `write-nodes` JSON input per source
- call `kinic-vfs-cli write-nodes --input <nodes.json>`
- write raw source node under `/Sources/raw/<source_slug>/<source_slug>.md`
- write source index under `/Wiki/sources/<source_slug>/index.md`
- write docs chunk nodes under `/Wiki/sources/<source_slug>/<version>/<slug>.md`

Every docs chunk links to the raw source node so existing Wiki evidence traversal can recover provenance.

## Operational Rules

- Use `SOURCE_OPS_STAGING_DATABASE_ID` for staging writes.
- Promote to `SOURCE_OPS_PROD_DATABASE_ID` only after staging write and smoke both pass.
- Store source tags and chunk identity in node `metadata_json`.
- Do not infer source identity from paths when `metadata_json` is available.
- Do not mix old and new payload schema shapes in one refresh batch.

## Acceptance Criteria

The source ingest setup is ready when:

- registry validation passes
- normalized payload validation passes
- dry-run report includes coverage and quality gates
- staging `wiki_write` succeeds
- smoke finds docs chunks under `/Wiki/sources`
- smoke confirms `/Sources/raw/` evidence links through `read-node-context`
- `kinic-context-cli query <source_id> ...` returns stable `title`, `citation`, and `version`
