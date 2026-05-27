# Source Payload Schema

## Goal

`tools/source_ops` turns upstream docs into canonical payload JSONL, then writes wiki nodes into the existing Kinic Wiki database. The payload is the stable boundary between collection/normalization and `kinic-vfs-cli write-nodes`.

## Canonical Shape

```json
{
  "source_id": "/vercel/next.js",
  "title": "Next.js Middleware",
  "snippet": "Use middleware to inspect requests and redirect unauthenticated users.",
  "citation": "https://nextjs.org/docs/app/building-your-application/routing/middleware",
  "version": "16",
  "content": "Full chunk text here",
  "section": "middleware",
  "tags": ["next", "middleware"],
  "retrieved_at": "2026-05-25T00:00:00Z",
  "section_index": 0,
  "chunk_index": 0,
  "source_type": "docs_site",
  "target_label": "docs-sitemap",
  "coverage_role": "api_reference",
  "upstream_url": "https://nextjs.org/docs/app/building-your-application/routing/middleware"
}
```

## Required Fields

- `source_id`: logical source ID from `registry.yaml`.
- `title`: human-readable chunk title.
- `snippet`: short retrieval/display summary.
- `citation`: absolute upstream URL, optionally with a heading anchor.

## Recommended Fields

- `version`: required for versioned docs sources.
- `content`: full chunk text used for wiki node content.
- `section`: normalized section label.
- `tags`: short source/domain aliases.
- `retrieved_at`: collection timestamp.
- `section_index` / `chunk_index`: deterministic chunk identity inputs.
- `source_type`: `docs_site`, `llms_full`, `repo_docs`, `changelog`, or `examples`.
- `target_label`: matching `crawl_targets[].label`.
- `coverage_role`: `overview`, `api_reference`, `integration`, `migration`, `troubleshooting`, `examples`, `release_notes`, or `sdk_reference`.
- `upstream_url`: fetched URL before heading anchor expansion.

## Validation Rules

- Payload must be a JSON object.
- `source_id`, `title`, `snippet`, and `citation` must be non-empty.
- `citation` must start with `http://`, `https://`, or `file://`.
- `source_id` must match the registry source being normalized.
- `snippet` must not duplicate long `content`.
- Versioned v1 sources still require `version`.

## Wiki Storage Semantics

- All source payloads can share one wiki database.
- Raw source node: `/Sources/raw/<source_slug>/<source_slug>.md`.
- Source index node: `/Wiki/sources/<source_slug>/index.md`.
- Docs chunk nodes: `/Wiki/sources/<source_slug>/<version>/<citation-hash>-<section>-s<section>-c<chunk>.md`.
- Canonical chunk identity is stored in wiki node `metadata_json.chunk_id`, not inferred from path.
