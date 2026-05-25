# Retrieval Phase 3 Plan

## 1. 比較対象
Phase 3 では、Phase 2 の heuristic を次の 3 系列で比較する。

- `Baseline`
  - `resolve -> max_sources fan-out -> fixed top_k=3`
- `Current deterministic`
  - fake provider / deterministic fixture 上の現行 policy
- `Current PocketIC`
  - PocketIC 上の現行 policy

追記:
- deterministic 側は `fake_memory_instance` を host から直接叩く retrieval benchmark を追加し、pack 層を通さず heuristic 自体を比較する

## 2. 検証ケース
必須ケース:
- `two-source-auth`
  - query: `protect route in next.js with supabase auth`
  - 期待: 2 source evidence を維持する
- `exact-middleware`
  - query: `middleware cookies`
  - 期待: top hit が `Next.js Middleware`
- `migration-version`
  - query: `migration breaking changes`
  - 期待: top hit が `Next.js Upgrade Guide`

比較強化ケース:
- `versioned-exact`
  - query: `middleware cookies v15`
- `vector-natural-language`
  - query: `how do i keep auth state fresh before rendering protected routes`
- `fallback-noise`
  - query: `next launchagent auth`
- `ambiguous-hooks`
- `budget-routing`

## 3. 判定基準
各検証ケースで次を確認する。

- `improved_canisters`
  - baseline 比で queried canisters が悪化していない
- `improved_tokens`
  - baseline 比で estimated tokens が悪化していない
- `quality_guard_passed`
  - expected top hit または expected evidence 条件を満たす

最低条件:
- 改善が 1 ケースだけでなく複数ケースで確認できる
- `exact / migration` の top hit が安定する
- `two-source-auth` の evidence が悪化しない

現状の確認済み改善:
- `vector-natural-language`
  - `section_candidate_count: 2 -> 1`
  - `document_candidate_count: 2 -> 1`
  - `estimated_pack_tokens: 76 -> 36`
- `fallback-noise`
  - `section_candidate_count: 2 -> 1`
  - `document_candidate_count: 2 -> 1`
  - `estimated_pack_tokens: 68 -> 32`
- `ambiguous-hooks`
  - `section_candidate_count: 2 -> 1`
  - `document_candidate_count: 2 -> 1`
  - `estimated_pack_tokens: 73 -> 37`

## 4. 移植条件
本番 canister へ進むのは次を満たした後に限る。

- deterministic と PocketIC で同じ report schema を生成できる
- PocketIC でも exact / migration / two-source-auth の quality guard が通る
- 複数ケースで `improved_canisters` または `improved_tokens` が確認できる
- schema 追加や section metadata 追加がなくても、現 heuristic の強みと弱みが report から読める

運用:
- tuning の各試行は [retrieval_tuning_log.md](/Users/0xhude/Desktop/work/KINIC%20Context%20Engine/retrieval_tuning_log.md) に `keep / revert / pending` で記録する
