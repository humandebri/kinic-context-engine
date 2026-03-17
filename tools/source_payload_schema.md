# Source Payload Schema

## Goal

A source-oriented memory instance must store JSON payloads that make `kinic-context-cli` retrieval stable and explainable. The primary consumers are:

- `query`
- `pack`
- `cite`

The payload format is canonical for source memory instances. Plain text payloads may still be readable by the CLI fallback path, but they are non-standard and should not be used for curated source instances.

The same schema is also used for curated skill knowledge sources such as `/skills/nextjs/migration`.

Catalog-only metadata such as `skill_kind`, `targets`, and `capabilities` is managed separately in the catalog canister and is not duplicated inside the payload JSON.

## Canonical Shape

```json
{
  "source_id": "/vercel/next.js",
  "title": "Next.js Middleware",
  "snippet": "Use middleware to inspect requests and redirect unauthenticated users.",
  "citation": "https://nextjs.org/docs/app/building-your-application/routing/middleware",
  "version": "15",
  "content": "Full chunk text here",
  "section": "middleware",
  "tags": ["auth", "cookies", "redirect"],
  "retrieved_at": "2026-03-17T00:00:00Z"
}
```

## Required Fields

### `source_id`

- Type: `string`
- Must exactly match one logical source ID used by the CLI
- Example: `"/vercel/next.js"`

### `title`

- Type: `string`
- Human-readable chunk title
- Should be stable across re-ingestion

### `snippet`

- Type: `string`
- Short retrieval-friendly summary
- Target length: 1-3 sentences
- Should be optimized for `pack` display, not raw storage

### `citation`

- Type: `string`
- Must be an absolute URL
- Must point to the source page or section used for the chunk

## Recommended Fields

### `version`

- Type: `string`
- Required for versioned sources such as framework docs
- Example: `"15"`

### `content`

- Type: `string`
- Full chunk text
- Can be longer than `snippet`

### `section`

- Type: `string`
- Logical subsection such as `"middleware"` or `"routing"`

### `tags`

- Type: `array<string>`
- Search-supporting tags that capture API/domain terms

### `retrieved_at`

- Type: `string`
- ISO-8601 timestamp for when this chunk was collected or normalized

## Validation Rules

### Global Rules

- Payload must be valid JSON object
- `source_id`, `title`, `snippet`, and `citation` must exist and be non-empty
- `citation` must begin with `http://` or `https://`
- `source_id` must match the target memory instance's logical source assignment

### Version Rules

For these v1 sources, `version` should be treated as required:

- `/vercel/next.js`
- `/supabase/docs`
- `/react/docs`

Skill sources under `/skills/...` do not require `version` in v1.

If a source has no meaningful version semantics in the future, `version` may be omitted, but that should be documented in the ingest spec before use.

### Content Rules

- `snippet` should not be identical to `content` when `content` is large
- `title` should be concise and section-specific
- `tags` should remain short and domain-relevant

## Source Semantics

Each source memory instance is single-purpose:

- one logical source ID
- one memory instance canister
- one consistent payload schema

Do not mix multiple `source_id` values in a single source memory instance.

For skill sources:

- use `domain = skill_knowledge` in the catalog
- keep `citation` pointed at the canonical repo URL for the skill origin
- prefer `snippet` for concise rules and `content` for the fuller skill guidance

Recommended skill citation shape:

- `https://github.com/<org>/<repo>/blob/<ref>/skills/<skill-name>/SKILL.md`

## Compatibility With Current CLI

`kinic-context-cli` currently reads payloads in this order:

1. `citation`
2. `url`
3. `source_url`

For canonical source payloads, always use `citation`.

`title` and `version` are also read directly when present. If omitted, the CLI falls back to heuristics, which is acceptable for legacy payloads but not for curated source instances.
