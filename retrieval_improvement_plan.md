# Retrieval Improvement Plan

## 1. 目的
source canister 内の `Hybrid Retrieval` を段階的 narrowing 前提で詰め、fan-out 数と token 消費を減らしつつ evidence の質を維持する。
ここでいう改善対象は `L0 / L1 / L2` 命名そのものではなく、section 候補選定、document 候補絞り込み、vector rerank / fusion の実効性である。

## 2. 現状
現在の source canister は `hybrid_query` の内部で次の順に処理している。

1. `sections` / `sections_fts` を使って section 候補を選ぶ
2. `documents_fts` を使って document candidate を絞る
3. 候補 document のみに vector score を計算する
4. keyword rank と vector rank を合成して返す

これは段階的 retrieval の入口としては成立しているが、query の種類ごとの policy、段階ごとの budget、section index の使い分けはまだ弱い。

現時点の成果:
- `Task` / `Ambiguous` の subcase ごとに section cap を分ける first pass が入っている
- direct retrieval benchmark では次の改善を確認済み
  - `vector-natural-language`: `estimated_pack_tokens 76 -> 36`
  - `fallback-noise`: `estimated_pack_tokens 68 -> 32`
  - `ambiguous-hooks`: `estimated_pack_tokens 73 -> 37`
- `exact / migration / two-source-auth` の guard は維持している

## 3. 比較指標
以後の比較では次の指標を固定で見る。

- `resolved_sources_count`
- `queried_canisters_count`
- `selected_evidence_count`
- `estimated_pack_tokens`
- `empty_source_count`
- `source_error_count`
- `resolve_ms`
- `query_ms_total`
- `pack_ms_total`

補助指標:
- top 1 / top 3 の妥当性
- migration / exact lookup の ranking 安定性

## 4. 検証ケース taxonomy
表向きには `検証ケース` と呼ぶ。内部型名 `BenchmarkScenario` は変更しない。

- 単一 source 明確
  - 例: `middleware cookies`
- 2 source 必須
  - 例: `protect route in next.js with supabase auth`
- migration / version 指向
  - 例: `migration breaking changes`
- ambiguous / noise 混在
  - 例: `next react hooks`
- keyword 優位
  - exact term や title hit を強く使うケース
- vector 優位
  - 自然文で semantic similarity を補助に使うケース

## 5. フェーズ
### Phase 1: 計画と指標の固定
目的:
- 改善対象と比較条件を固定する

成果物:
- この計画 `md`
- README / `plan.md` からの導線
- 指標と検証ケース taxonomy の明文化

完了条件:
- 現状 pipeline、比較指標、受け入れ条件がこの文書に揃っている
- README / `plan.md` からこの文書へ辿れる

### Phase 2: source canister 内 retrieval 改善
目的:
- query-aware な narrowing と fusion を入れる

対象:
- query taxonomy ごとの retrieval policy
- `sections` の retrieval index 強化
- section candidate / document candidate / final evidence の段階別 budget
- fixed weight から query-aware policy への寄せ

方針:
- first pass では additive change を優先する
- `query <source_id>` は inspection path として維持する

完了条件:
- deterministic benchmark が全通
- 少なくとも 1 つ以上の検証ケースで baseline 比の改善が出る
- exact / migration 系 query の top hit が壊れない

現状:
- query-aware policy は導入済み
- `Task` と `Ambiguous` の一部 subcase では section cap による narrowing を採用済み
- 改善が見えなかった試行は `retrieval_tuning_log.md` 上で revert 済み

### Phase 3: 比較評価と移植判断
目的:
- deterministic と PocketIC の結果を同じ report で比較し、本番移植条件を明文化する

対象:
- `Baseline / Current deterministic / Current PocketIC` の比較
- 共通 report schema と Markdown summary
- 本番 canister へ移す判断基準の固定

完了条件:
- PocketIC ignored tests が全通
- deterministic と PocketIC を同じ report で比較できる
- 2 source 必須ケースで evidence が baseline より悪化しない

詳細:
- Phase 3 の比較対象と移植条件は [retrieval_phase3_plan.md](/Users/0xhude/Desktop/work/KINIC%20Context%20Engine/retrieval_phase3_plan.md) を参照
- pack 層の benchmark だけでなく、`fake_memory_instance` を直接叩く retrieval benchmark で heuristic 自体も評価する

## 6. 受け入れ条件
- queried canisters か estimated tokens のどちらかで改善が出る
- 2 source 必須ケースで `selected_evidence_count` と結果妥当性が悪化しない
- migration / exact lookup の top result が崩れない
- public CLI command は増やさない
- wire 型変更が必要でも additive に限定する

## 7. 前提
- 既存の `plan.md` は全体方針として維持する
- retrieval 改善はこの別文書で管理する
- フェーズ分割は 3 段階で進める
- source canister 内の正式名称として `L0 / L1 / L2` は導入しない
- 主眼は命名ではなく段階的 narrowing の強化に置く
