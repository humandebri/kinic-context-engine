# kinic-context-cli

Read-only Rust CLI for source resolution, retrieval, and evidence pack generation on top of a catalog canister and existing KINIC memory instances.

## Commands

- `kinic-context-cli resolve "<query>"`
- `kinic-context-cli resolve "<query>" [--include-skills]`
- `kinic-context-cli query <source_id> "<query>" [--version <version>] [--top-k <n>]`
- `kinic-context-cli pack "<query>" [--max-sources <n>] [--max-tokens <n>] [--include-skills]`
- `kinic-context-cli cite <pack-json-or-path>`
- `kinic-context-cli list-sources [--include-skills]`
- `kinic-context-cli filter-sources [--domain <value>] [--trust <value>] [--version <value>] [--limit <n>] [--include-skills]`

## Environment

- `KINIC_CONTEXT_CATALOG_CANISTER_ID`: required catalog canister ID
- `KINIC_CONTEXT_IC_HOST`: optional IC host, defaults to `https://ic0.app`
- `KINIC_CONTEXT_LAUNCHER_CANISTER_ID`: optional launcher canister ID for live verification
- `KINIC_CONTEXT_FETCH_ROOT_KEY`: optional `true/1` for local replica reads
- `EMBEDDING_API_ENDPOINT`: optional embedding endpoint, defaults to `https://api.kinic.io`

## Architecture

- `service.did` is the existing launcher interface
- `instance.did` is the existing memory instance interface
- `tools/catalog_canister` is the new catalog-only canister
- source logical IDs such as `/vercel/next.js` are resolved by the catalog canister
- the CLI reads `canister_ids[]` from the catalog and runs memory instance `search(vec float32) -> vec (float32, text)` against those canisters
- skill knowledge can also be registered as structured sources such as `/skills/nextjs/migration`
- skill citations should use canonical repo URLs, not local file paths

## Deploy With `icp`

```bash
icp network start -d
icp deploy catalog_canister
```

catalog canister ID は `.icp/data/mappings/<environment>.ids.json` の `catalog_canister` から取得します。

```bash
export KINIC_CONTEXT_CATALOG_CANISTER_ID="$(jq -r '.catalog_canister' .icp/data/mappings/local.ids.json)"
```

memory instance を結びつけるには controller で `admin_upsert_source` または `admin_replace_catalog` を呼びます。

```bash
icp canister call -e local catalog_canister admin_upsert_source \
  '(record {
    source_id = "/vercel/next.js";
    title = "Next.js Docs";
    aliases = vec {"next"; "nextjs"; "next.js"; "middleware"};
    trust = "official";
    domain = "code_docs";
    canister_ids = vec {"aaaaa-aa"; "bbbbb-bb"};
    supported_versions = vec {"14"; "15"};
    retrieved_at = "2026-03-17T00:00:00Z";
    citations = vec {"https://nextjs.org/docs"};
  })'
```

CLI は catalog canister を起点に `memory instance canister` 群へ fan-out します。

```bash
kinic-context-cli resolve "next middleware"
kinic-context-cli resolve "next migration" --include-skills
kinic-context-cli list-sources
kinic-context-cli list-sources --include-skills
kinic-context-cli filter-sources --domain skill_knowledge
kinic-context-cli filter-sources --domain code_docs --trust official --version 15
kinic-context-cli query /skills/nextjs/migration "upgrade checklist"
kinic-context-cli query /vercel/next.js "middleware cookies" --version 15
kinic-context-cli pack "protect route in next.js with supabase auth"
kinic-context-cli pack "next migration auth changes" --include-skills
```

`filter-sources --domain skill_knowledge` は `--include-skills` なしでも直接問い合わせできます。

## Verification

### live ICP verification

- ignored live tests require `KINIC_CONTEXT_CATALOG_CANISTER_ID`
- launcher verification additionally requires `KINIC_CONTEXT_LAUNCHER_CANISTER_ID`
- `cargo test --workspace -- --ignored`

### PocketIC integration tests

- PocketIC tests are ignored by default and do not run in `cargo test --workspace`
- set `POCKET_IC_BIN=/path/to/pocket-ic-server`
- run `cargo test -p pocket_ic_tests -- --ignored`
- `resolve` is verified at the real CLI binary boundary
- `query/pack` and error contracts are verified at the engine-level E2E layer

## Safety boundary

- read-only retrieval only
- no write/update/token/admin commands in the CLI
- JSON output by default

## Catalog canister

- location: [`tools/catalog_canister`](/Users/0xhude/Desktop/work/KINIC%20Context%20Engine/tools/catalog_canister)
- storage: `ic-rusqlite`
- migrations: `ic-sql-migrate`
- project config: [`icp.yaml`](/Users/0xhude/Desktop/work/KINIC%20Context%20Engine/icp.yaml)
- read API:
  - `list_sources()`
  - `get_source(source_id)`
  - `resolve_sources(query, limit)`
  - `filter_sources(args)`

## MVP sources

- `/vercel/next.js`
- `/supabase/docs`
- `/react/docs`
- `/skills/nextjs/migration`
