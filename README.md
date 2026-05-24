# KINIC Context Engine

Read-only Rust workspace for source resolution, retrieval, and evidence pack generation on top of a catalog canister and hybrid search source canisters.

The main user-facing binary is `kinic-context-cli`.

## What This Repo Contains

- `kinic-context-cli`: read-only CLI for resolving sources, querying hybrid search canisters, and generating evidence packs
- `crates/kinic_context_core`: shared client, engine, config, and type logic
- `tools/catalog_canister`: catalog canister that stores source metadata and resolution indices
- `tools/pocket_ic_tests`: PocketIC integration coverage for catalog and CLI flows

## Quick Start

```bash
cargo build
cargo run -- resolve "next middleware"
```

## Status

- workspace build and non-ignored tests pass locally
- PocketIC ignored tests require `POCKET_IC_BIN`
- live acceptance tests require real canister environment variables
- retrieval 改善の段階計画は [retrieval_improvement_plan.md](/Users/0xhude/Desktop/work/KINIC%20Context%20Engine/retrieval_improvement_plan.md) で管理する
- Phase 3 の比較評価と移植 gate は [retrieval_phase3_plan.md](/Users/0xhude/Desktop/work/KINIC%20Context%20Engine/retrieval_phase3_plan.md) で管理する
- 改善ループの試行記録は [retrieval_tuning_log.md](/Users/0xhude/Desktop/work/KINIC%20Context%20Engine/retrieval_tuning_log.md) に残す
- 採用済み tuning では、`vector-natural-language` を `76 -> 36` tokens、`fallback-noise` を `68 -> 32` tokens、`ambiguous-hooks` を `73 -> 37` tokens まで削減できている
- this repo ships under the MIT license

## Commands

- `kinic-context-cli resolve "<query>"`
- `kinic-context-cli query <source_id> "<query>" [--version <version>] [--top-k <n>]`
- `kinic-context-cli pack "<query>" [--max-sources <n>] [--max-tokens <n>]`
- `kinic-context-cli cite <pack-json-or-path>`
- `kinic-context-cli list-sources`
- `kinic-context-cli filter-sources [--domain <value>] [--trust <value>] [--version <value>] [--limit <n>]`

## Environment

- `KINIC_CONTEXT_CATALOG_CANISTER_ID`: required catalog canister ID
- `KINIC_CONTEXT_IC_HOST`: optional IC host, defaults to `https://ic0.app`
- `KINIC_CONTEXT_LAUNCHER_CANISTER_ID`: optional launcher canister ID for live verification
- `KINIC_CONTEXT_FETCH_ROOT_KEY`: optional `true/1` for local replica reads
- `EMBEDDING_API_ENDPOINT`: optional remote embedding endpoint override; unset means local Rust/ONNX mode
- `KINIC_CONTEXT_EMBEDDING_MODEL`: optional embedding model hint, defaults to `intfloat/multilingual-e5-large`
- `KINIC_CONTEXT_EMBEDDING_MODEL_DIR`: optional local model directory; defaults to `.local/models/multilingual-e5-large`
- `KINIC_CONTEXT_EMBEDDING_QUERY_PREFIX`: optional query prefix, defaults to `query: `
- `KINIC_CONTEXT_EMBEDDING_DOCUMENT_PREFIX`: optional document/section prefix, defaults to `passage: `
- `KINIC_CONTEXT_EMBEDDING_HELPER`: optional helper binary override for `tools/source_ops`, defaults to `target/debug/kinic-embed`

## Architecture

- `service.did` is the existing launcher interface
- `Catalog/Resolution Layer` (`tools/catalog_canister`) stores source metadata, aliases, and resolution indices, then narrows fan-out candidates before retrieval
- source logical IDs such as `/vercel/next.js` are resolved in the `Catalog/Resolution Layer`
- `Hybrid Retrieval & Pack Layer` queries source canisters, reranks cross-source results, and builds the final evidence pack
- source canisters remain separate execution targets; the retrieval layer reads `canister_ids[]` from the catalog and runs `hybrid_query(record { query_text; query_embedding; version; top_k })` against those canisters
- source canisters can expose a minimal `L0` section index through `insert_section(record { section_id; title; summary; version; embedding })`
- source canisters are responsible for `FTS5(trigram)`, vector similarity, and RRF fusion
- query/document embedding generation stays outside the canister boundary in the CLI and `tools/source_ops`
- local embedding generation uses Rust + ONNX with `multilingual-e5-large`; `tools/source_ops` calls the `kinic-embed` helper binary
- the pack path records efficiency metrics such as resolved source count, queried canister count, returned snippet count, estimated pack tokens, and stage latency
- curated migration and playbook content should be absorbed into ordinary docs sources instead of using a dedicated skill source type
- retrieval 改善のフェーズ計画と受け入れ条件は [retrieval_improvement_plan.md](/Users/0xhude/Desktop/work/KINIC%20Context%20Engine/retrieval_improvement_plan.md) を参照

## Deploy With `icp`

```bash
icp network start -d
icp deploy catalog_canister
```

local の catalog canister ID は `.icp/cache/mappings/local.ids.json` の `catalog_canister` から取得できます。

```bash
export KINIC_CONTEXT_CATALOG_CANISTER_ID="$(jq -r '.catalog_canister' .icp/cache/mappings/local.ids.json)"
export KINIC_CONTEXT_IC_HOST=http://127.0.0.1:8000
export KINIC_CONTEXT_FETCH_ROOT_KEY=true
```

`catalog_canister` だけを deploy しても `pack` は成功しません。各 source に少なくとも 1 つの source/memory canister を結びつける必要があります。controller で `admin_upsert_source` または `admin_replace_catalog` を呼んで `canister_ids` を更新してください。

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

通常の `resolve` / `pack` は `Catalog/Resolution -> Hybrid Retrieval & Pack` の流れで動作します。catalog canister が fan-out 対象を絞り込み、retrieval layer が hybrid source canister 群へ問い合わせます。`query <source_id>` の直指定では source 解決を省略し、retrieval layer が指定 source に対して直接 retrieval を実行します。

```bash
kinic-context-cli resolve "next middleware"
kinic-context-cli resolve "next migration"
kinic-context-cli list-sources
kinic-context-cli filter-sources --domain code_docs --trust official --version 15
kinic-context-cli query /vercel/next.js "middleware cookies" --version 15
kinic-context-cli pack "protect route in next.js with supabase auth"
kinic-context-cli pack "next migration auth changes"
```

## Verification

### local embedding setup

- build the helper binary:

```bash
cargo build --bin kinic-embed
```

- place the model assets under `.local/models/multilingual-e5-large/`. Expected layout:

```text
.local/models/multilingual-e5-large/
  config.json
  tokenizer.json
  onnx/
    model.onnx
```

- or point `KINIC_CONTEXT_EMBEDDING_MODEL_DIR` at another directory with the same layout:

```bash
export KINIC_CONTEXT_EMBEDDING_MODEL_DIR=/absolute/path/to/multilingual-e5-large
```

- validate the layout with:

```bash
bash scripts/setup_local_embedding.sh
```

- the CLI does not auto-download model weights during normal execution

### live ICP verification

- required:
  - `KINIC_CONTEXT_CATALOG_CANISTER_ID`
  - `KINIC_CONTEXT_IC_HOST` if not using `https://ic0.app`
  - `KINIC_CONTEXT_FETCH_ROOT_KEY=true` when targeting a local replica
  - `KINIC_CONTEXT_LAUNCHER_CANISTER_ID` for launcher verification
- run:

```bash
cargo test -p kinic-context-cli --test acceptance_live_tests -- --ignored
```

### PocketIC Integration Tests

- PocketIC の integration test はデフォルトで `ignored` で、`cargo test --workspace` には含まれません
- 実行前に `POCKET_IC_BIN=/absolute/path/to/pocket-ic-server` を設定します
- 例:

```bash
export POCKET_IC_BIN=/Users/you/path/to/pocket-ic-server
cargo test -p pocket_ic_tests -- --ignored
```

- binary はこの repository 配下や `icp` CLI 配下に置く必要はありません
- `resolve` は実際の CLI binary 境界で検証します
- `query/pack` と hybrid query の契約は engine-level E2E で検証します

## Efficiency Benchmarking

- deterministic benchmark の検証は `tests/benchmark_tests.rs` にあります
- benchmark report の検証は `tests/benchmark_runner_tests.rs` にあります
- benchmark の出力は `pack.metrics` JSON と共通の benchmark report JSON schema から参照します
- deterministic と PocketIC は同じ `BenchmarkSuiteReport` / `markdown_summary()` 経路で比較する
- docs と Markdown summary では `scenario` を `検証ケース` の意味で扱います
- 現在の比較で見ている点:
  - baseline の `resolve -> max_sources fan-out` より source 選定を絞れているか
  - 固定 `top_k=3` ではなく token budget に応じた per-source retrieval depth になっているか
  - queried canisters 数と推定 token 数を減らしつつ multi-source evidence の質を落としていないか
- 実行モード:
  - `deterministic only`: `cargo test --test benchmark_tests` と `cargo test --test benchmark_runner_tests`
  - `PocketIC enabled`: `cargo test -p pocket_ic_tests --test catalog_e2e -- --ignored`
- benchmark report の導線は現状 test-driven で、JSON/Markdown は test 内で生成し、repo tracked file にはデフォルトでは書き込みません
- benchmark report は本番移植前の gate として扱い、Phase 3 の判定基準は [retrieval_phase3_plan.md](/Users/0xhude/Desktop/work/KINIC%20Context%20Engine/retrieval_phase3_plan.md) を参照します
- この gate は pack 層だけでなく retrieval heuristic 自体の比較も含み、`fake_memory_instance` の direct benchmark と PocketIC を両方使います
- 現在 keep している direct retrieval 改善は、`vector-natural-language` / `fallback-noise` / `ambiguous-hooks` の 3 ケースで document candidate と token を削減しつつ guard を維持しています

## Safety boundary

- read-only retrieval only
- no write/update/token/admin commands in the CLI
- JSON output by default

## Catalog canister

- location: `tools/catalog_canister`
- storage: `ic-sqlite-vfs` on stable memory, fixed `MemoryId::new(120)`
- migrations: `ic_sqlite_vfs::db::migrate::Migration`
- wasm target: `wasm32-unknown-unknown`
- project config: `icp.yaml`
- read API:
  - `list_sources()`
  - `get_source(source_id)`
  - `resolve_sources(query, limit)`
  - `filter_sources(args)`

## OSS Release Checklist

- add a real GitHub repository at the `repository` URL declared in `Cargo.toml`
- keep `LICENSE` at the repo root
- document live environment values before asking users to run ignored acceptance tests
- avoid absolute local filesystem links in docs

## MVP sources

- `/vercel/next.js`
- `/supabase/docs`
- `/react/docs`
