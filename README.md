# KINIC Context Engine

Source collection and context packaging tools that write documentation payloads into an existing Kinic Wiki database.

The main user-facing binary is `kinic-context-cli`.

## What This Repo Contains

- `kinic-context-cli`: legacy read-only CLI for resolving sources and generating evidence packs
- `crates/kinic_context_core`: shared client, engine, config, and type logic
- `tools/source_ops`: source collection, normalization, wiki node writing, and smoke checks

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

- `SOURCE_OPS_STAGING_DATABASE_ID`: staging Kinic Wiki database id for source writes
- `SOURCE_OPS_PROD_DATABASE_ID`: production Kinic Wiki database id for source writes
- `SOURCE_OPS_WIKI_CLI_BIN`: optional `kinic-vfs-cli` command override; use a wrapper script when the executable path contains spaces
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

- Kinic Wiki canister and its existing database API are the storage/runtime boundary
- `tools/source_ops` converts normalized docs payloads into wiki nodes and writes them with `kinic-vfs-cli write-node`
- raw source nodes live under `/Sources/raw/<source_slug>/<source_slug>.md`
- searchable docs chunks live under `/Wiki/sources/<source_slug>/<version>/<citation-hash>-<section>-s<section>-c<chunk>.md`
- docs chunks link back to raw source nodes so existing wiki `source_evidence` can recover provenance
- search/context use existing `kinic-vfs-cli search-remote` and `read-node-context`
- curated playbook content should be absorbed into ordinary docs sources instead of using a dedicated source type
- retrieval 改善のフェーズ計画と受け入れ条件は [retrieval_improvement_plan.md](/Users/0xhude/Desktop/work/KINIC%20Context%20Engine/retrieval_improvement_plan.md) を参照

## Kinic Wiki Runtime

この repo は canister を deploy しません。既存の `llm-wiki` / `kinic-vfs-cli` を使います。

```bash
kinic-vfs-cli database link <database-id>
kinic-vfs-cli search-remote "next middleware" --prefix /Wiki/sources --json
kinic-vfs-cli read-node-context --path /Wiki/sources/<source_slug>/index.md --link-limit 20 --json
```

## Add A Docs Source

source 登録は `tools/source_ops/register_source.py` で registry に追記します。手動で JSON を編集しません。

```bash
python3 tools/source_ops/register_source.py \
  --source-id /tanstack/query \
  --title "TanStack Query Docs" \
  --url docs=https://tanstack.com/query/latest/docs/framework/react/overview \
  --alias "tanstack query" \
  --version latest
```

登録後、明示 source 指定で収集から staging smoke まで実行します。`--source` 指定時は `cadence: manual` の source も対象になります。

```bash
export SOURCE_OPS_STAGING_DATABASE_ID=<staging-wiki-database-id>
export SOURCE_OPS_PROD_DATABASE_ID=<prod-wiki-database-id>
python3 tools/source_ops/run_refresh.py --source /tanstack/query --dry-run
python3 tools/source_ops/run_refresh.py --source /tanstack/query
```

既定の write path は `payloads -> wiki nodes -> kinic-vfs-cli write-node* -> smoke` です。

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

### Wiki CLI verification

- required:
  - `SOURCE_OPS_STAGING_DATABASE_ID`
  - `SOURCE_OPS_PROD_DATABASE_ID`
  - `kinic-vfs-cli` installed or `SOURCE_OPS_WIKI_CLI_BIN` set
- run source_ops unit tests and a dry-run refresh before writing to a real database

## Efficiency Benchmarking

- deterministic benchmark の検証は `tests/benchmark_tests.rs` にあります
- benchmark report の検証は `tests/benchmark_runner_tests.rs` にあります
- benchmark の出力は `pack.metrics` JSON と共通の benchmark report JSON schema から参照します
- deterministic と PocketIC は同じ `BenchmarkSuiteReport` / `markdown_summary()` 経路で比較する
- docs と Markdown summary では `scenario` を `検証ケース` の意味で扱います
- 現在の比較で見ている点:
  - baseline の `resolve -> max_sources fan-out` より source 選定を絞れているか
  - 固定 `top_k=3` ではなく token budget に応じた per-source retrieval depth になっているか
  - queried source 数と推定 token 数を減らしつつ multi-source evidence の質を落としていないか
- 実行モード:
  - `deterministic only`: `cargo test --test benchmark_tests` と `cargo test --test benchmark_runner_tests`
- benchmark report の導線は現状 test-driven で、JSON/Markdown は test 内で生成し、repo tracked file にはデフォルトでは書き込みません
- benchmark report は本番移植前の gate として扱い、Phase 3 の判定基準は [retrieval_phase3_plan.md](/Users/0xhude/Desktop/work/KINIC%20Context%20Engine/retrieval_phase3_plan.md) を参照します

## Safety boundary

- read-only retrieval only
- no write/update/token/admin commands in the CLI
- JSON output by default

## OSS Release Checklist

- add a real GitHub repository at the `repository` URL declared in `Cargo.toml`
- keep `LICENSE` at the repo root
- document live environment values before asking users to run ignored acceptance tests
- avoid absolute local filesystem links in docs

## MVP sources

- `/vercel/next.js`
- `/supabase/docs`
- `/react/docs`
