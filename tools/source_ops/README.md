# source_ops

公開情報の収集、差分検知、payload 生成、memory/catalog 更新、read-path smoke をまとめる運用フォルダです。

## Entry points

- `python tools/source_ops/collect.py --source /vercel/next.js`
- `python tools/source_ops/normalize.py --source /vercel/next.js`
- `python tools/source_ops/validate.py --source /vercel/next.js`
- `python tools/source_ops/diff.py --source /vercel/next.js`
- `python tools/source_ops/apply_memory.py --env staging --source /vercel/next.js --dry-run`
- `python tools/source_ops/apply_catalog.py --env staging --source /vercel/next.js --dry-run`
- `python tools/source_ops/smoke.py --env staging --source /vercel/next.js`
- `python tools/source_ops/run_refresh.py --dry-run`

## Registry

- `registry.yaml` は JSON 互換 YAML です
- 依存追加を避けるため、stdlib `json` で読める形を維持します
- source 追加は registry 更新を唯一の入口にします

## Apply mode

- memory 更新は既定で `tools/source_ops/kinic_writer.py` を使い、`reset -> exact payload insert` を行います
- 必要なら `SOURCE_OPS_MEMORY_WRITER_TEMPLATE` / `SOURCE_OPS_MEMORY_ROLLBACK_TEMPLATE` で上書きできます
- exact payload insert では payload JSON 自体を memory に保存し、embedding は `EMBEDDING_API_ENDPOINT/embedding` から取得します
- catalog 更新は `icp canister call ... admin_upsert_source` を使います
- prod 昇格は staging 成功後のみです

## Codex automation

日次 automation は `python tools/source_ops/run_refresh.py` を実行し、`artifacts/reports/` の結果を確認する前提です。
