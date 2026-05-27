# source_ops

公開情報の収集、差分検知、payload 生成、Kinic Wiki 更新、read-path smoke をまとめる運用フォルダです。

## Entry points

- `python tools/source_ops/collect.py --source /vercel/next.js`
- `python tools/source_ops/register_source.py --source-id /tanstack/query --title "TanStack Query Docs" --url overview=https://tanstack.com/query/latest/docs/framework/react/overview --url api=https://tanstack.com/query/latest/docs/framework/react/reference/useQuery --url examples=https://tanstack.com/query/latest/docs/framework/react/examples/basic --alias "tanstack query" --version latest`
- `python tools/source_ops/normalize.py --source /vercel/next.js`
- `python tools/source_ops/validate.py --source /vercel/next.js`
- `python tools/source_ops/diff.py --source /vercel/next.js`
- `python tools/source_ops/apply_wiki.py --env staging --source /vercel/next.js --dry-run`
- `python tools/source_ops/smoke.py --env staging --source /vercel/next.js`
- `python tools/source_ops/run_refresh.py --dry-run`

## Registry

- `registry.yaml` は JSON 互換 YAML です
- 依存追加を避けるため、stdlib `json` で読める形を維持します
- source は `crawl_targets` を持ち、`explicit_urls` / `llms_full` / `sitemap` / `github_tree` で収集します
- `crawl_targets[].max_pages` は必須です。巨大sitemapやGitHub treeを無制限に取得しません
- `crawl_targets[].coverage_role` は必須です。source追加より `overview` / `api_reference` / `examples` を満たすtarget追加を優先します
- source 追加は `register_source.py` による registry 更新を入口にします

## Apply mode

- wiki 更新は既定で `tools/source_ops/kinic_writer.py` を使い、`kinic-vfs-cli write-nodes` を1 source 1 batchで実行します
- `SOURCE_OPS_STAGING_DATABASE_ID` / `SOURCE_OPS_PROD_DATABASE_ID` で対象 wiki database を指定します
- `SOURCE_OPS_WIKI_CLI_BIN` で `kinic-vfs-cli` の実行コマンドを上書きできます。path に空白がある場合は wrapper script を指定します
- raw source は `/Sources/raw/<source_slug>/<source_slug>.md`、docs chunk は `/Wiki/sources/<source_slug>/<version>/<citation-hash>-<section>-s<section>-c<chunk>.md` に保存します
- docs chunk 本文には raw source node への link を入れ、既存 `source_evidence` が根拠を拾える形にします
- prod 昇格は staging 成功後のみです

## Manual smoke

```bash
kinic-vfs-cli search-remote "middleware" --prefix /Wiki/sources --json
kinic-vfs-cli read-node-context --path /Wiki/sources/<source_slug>/index.md --link-limit 20 --json
```

`read-node-context` の `outgoing_links[].target_path` に `/Sources/raw/` が含まれることを確認します。

## Codex automation

日次 automation は `python tools/source_ops/run_refresh.py` を実行し、`artifacts/reports/` の結果を確認する前提です。

## Daily refresh gate

- まず `python3 tools/source_ops/run_refresh.py --dry-run` を実行します
- `coverage.target_count` / `fetched_url_count` / `normalized_chunk_count` / `source_type_breakdown` / `coverage_role_breakdown` を確認します
- `coverage.missing_required_roles` が空であることを確認します
- `quality_gates.added_records` / `changed_records` / `removed_records` と `normalization_warning_count` を確認します
- staging write 後は `search-remote` が `/Wiki/sources/<source_slug>/<version>/...` の docs chunk を返すことを確認します
- staging write 後は `read-node-context` の `outgoing_links[].target_path` に `/Sources/raw/` が含まれることを確認します
- prod 昇格は staging の wiki write と smoke が両方 `ok` の場合だけです
