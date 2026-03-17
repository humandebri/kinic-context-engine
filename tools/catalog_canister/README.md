# catalog_canister

Catalog-only canister for `kinic-context-cli`.

- storage: `ic-rusqlite`
- migrations: `ic-sql-migrate`
- purpose: map source logical IDs such as `/vercel/next.js` to source metadata and backing memory instance canister IDs
- skill sources can carry structured metadata such as `skill_kind`, `targets`, and `capabilities`
- non-goal: chunk storage or retrieval RAG

## Read API

- `list_sources()`
- `get_source(source_id)`
- `resolve_sources(query, limit)`
- `filter_sources(args)`

## Admin API

- `admin_upsert_source(source)`
- `admin_replace_catalog(sources)`

Admin methods are controller-only.

## Deploy

この repo ルートの [`icp.yaml`](/Users/0xhude/Desktop/work/KINIC%20Context%20Engine/icp.yaml) から build/deploy できます。

```bash
icp network start -d
icp deploy catalog_canister
```

SQLite runtime は `wasm32-wasip1 + wasi2ic` 前提です。事前に `wasi2ic` を入れてください。
`candid:service` metadata は `ic-wasm` で [`catalog_canister.did`](/Users/0xhude/Desktop/work/KINIC%20Context%20Engine/tools/catalog_canister/catalog_canister.did) から埋め込みます。

## Seed data

- 既定の builtin source は migration + seed で投入されます
- seed の雛形は [`catalog.seed.json`](/Users/0xhude/Desktop/work/KINIC%20Context%20Engine/tools/catalog_canister/catalog.seed.json) にあります
- 実運用では controller で `admin_upsert_source` か `admin_replace_catalog` を使って `canister_ids` を更新します

## CLI integration

deploy 後は catalog canister ID を CLI に渡します。

```bash
export KINIC_CONTEXT_CATALOG_CANISTER_ID="$(jq -r '.catalog_canister' .icp/data/mappings/local.ids.json)"
```

## Admin update

`catalog.seed.json` は雛形です。実運用では controller が `icp canister call` で `admin_upsert_source` か `admin_replace_catalog` を叩きます。

```bash
icp canister call -e local catalog_canister admin_replace_catalog --args-file tools/catalog_canister/catalog.seed.candid
```
