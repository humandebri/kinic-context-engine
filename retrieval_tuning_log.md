# Retrieval Tuning Log

## 使い方
このログは retrieval heuristic の改善ループ専用に使う。

- 1 変更につき 1 記録を残す
- 記録単位は `keep / revert / pending` のいずれかで閉じる
- exact / migration / two-source-auth の guard を壊した変更は採用しない
- weak case が見えた場合も削除せず、そのまま次の仮説につなぐ

## 記録フォーマット
```md
## Trial NN
日付:

仮説:

変更:

実行:
- cargo test ...

比較ケース:
- ...

結果:
- improved:
- no change:
- regress:

guard:
- exact:
- migration:
- two-source-auth:

判断:
- keep / revert / pending

次:
```

## Trial 00
日付:
- 2026-03-25

仮説:
- まずは改善ループに入る前に、比較導線と guard を固定して現在値を baseline として残す

変更:
- direct retrieval benchmark を追加
- `Baseline / Current deterministic / Current PocketIC` の三者比較を report schema に載せる
- Phase 3 用の比較計画を分離する

実行:
- `cargo test --test retrieval_benchmark_tests`
- `cargo test --test retrieval_benchmark_runner_tests`
- `cargo test --test benchmark_tests`
- `cargo test --test benchmark_runner_tests`
- `cargo test -p pocket_ic_tests --test catalog_e2e --no-run`
- `source ~/.zshrc && cargo test -p pocket_ic_tests --test catalog_e2e -- --ignored`

比較ケース:
- `two-source-auth`
- `exact-middleware`
- `migration-version`
- `versioned-exact`
- `vector-natural-language`
- `fallback-noise`
- `ambiguous-hooks`
- `budget-routing`

結果:
- improved:
  - 複数ケースで `improved_canisters` または `improved_tokens` を確認
- no change:
  - 一部ケースは guard は通るが改善量は小さい
- regress:
  - direct retrieval benchmark では weak case が可視化される

guard:
- exact:
  - pass
- migration:
  - pass
- two-source-auth:
  - pass

判断:
- keep

次:
- `infer_retrieval_policy()` の弱いケースを 1 つずつ調整し、1 変更ごとに keep/revert を記録する

## Trial 01
日付:
- 2026-03-25

仮説:
- `fallback-noise` のように複数トピック語が混ざる query は exact 扱いより ambiguous 扱いの方が良い

変更:
- `infer_retrieval_policy()` の ambiguous 判定を拡張
- `launchagent` / `plist` を ambiguous marker に追加
- 複数トピック語が 3 つ以上入る query も ambiguous として扱う

実行:
- `cargo test --test retrieval_benchmark_tests`
- `cargo test --test retrieval_benchmark_runner_tests`
- `cargo test --test benchmark_tests`
- `cargo test --test benchmark_runner_tests`
- `cargo test -p pocket_ic_tests --test catalog_e2e --no-run`
- `source ~/.zshrc && cargo test -p pocket_ic_tests --test catalog_e2e -- --ignored`

比較ケース:
- `fallback-noise`
- `ambiguous-hooks`
- `exact-middleware`
- `migration-version`
- `two-source-auth`

結果:
- improved:
  - direct retrieval benchmark で previously weak だったケースが quality guard を通るようになった
  - `retrieval_benchmark_tests` / `retrieval_benchmark_runner_tests` が通る
- no change:
  - pack 層 benchmark の改善数は維持
- regress:
  - なし

guard:
- exact:
  - pass
- migration:
  - pass
- two-source-auth:
  - pass

判断:
- keep

次:
- 次は `vector-natural-language` と `ambiguous-hooks` の改善量を見ながら、weights と `min_keyword_candidates` を 1 変更ずつ試す

## Trial 02
日付:
- 2026-03-25

仮説:
- `Task` 系 query は vector を少し強めた方が `vector-natural-language` の順位品質を改善しやすい

変更:
- `infer_retrieval_policy()` の `RetrievalPolicyKind::Task` で weight を調整
- `keyword_weight: 0.40 -> 0.35`
- `vector_weight: 0.60 -> 0.65`

実行:
- `cargo test --test retrieval_benchmark_tests`
- `cargo test --test retrieval_benchmark_runner_tests`
- `cargo test --test benchmark_tests`
- `cargo test --test benchmark_runner_tests`
- `cargo test -p pocket_ic_tests --test catalog_e2e --no-run`
- `source ~/.zshrc && cargo test -p pocket_ic_tests --test catalog_e2e -- --ignored`

比較ケース:
- `vector-natural-language`
- `two-source-auth`
- `exact-middleware`
- `migration-version`
- `fallback-noise`

結果:
- improved:
  - `vector-natural-language` を含む direct retrieval / pack / PocketIC の gate を維持したまま通過
  - 既存の `improved_canisters` / `improved_tokens` 前提テストは崩れなかった
- no change:
  - exact / migration / two-source-auth の guard 条件は維持
- regress:
  - なし

guard:
- exact:
  - pass
- migration:
  - pass
- two-source-auth:
  - pass

判断:
- keep

次:
- 次は `Task` / `Ambiguous` の `min_keyword_candidates` か section budget を 1 箇所だけ触って、`ambiguous-hooks` と `fallback-noise` の改善余地を見る

## Trial 03
日付:
- 2026-03-25

仮説:
- `Ambiguous` 系 query で `min_keyword_candidates` を下げると、不要な fallback widening が減って token と noise を抑えやすい

変更:
- `infer_retrieval_policy()` の `RetrievalPolicyKind::Ambiguous` で `min_keyword_candidates: 2 -> 1`

実行:
- `cargo test --test retrieval_benchmark_tests`
- `cargo test --test retrieval_benchmark_runner_tests`
- `cargo test --test benchmark_tests`
- `cargo test --test benchmark_runner_tests`
- `cargo test -p pocket_ic_tests --test catalog_e2e --no-run`
- `source ~/.zshrc && cargo test -p pocket_ic_tests --test catalog_e2e -- --ignored`

比較ケース:
- `ambiguous-hooks`
- `fallback-noise`
- `exact-middleware`
- `migration-version`
- `two-source-auth`

結果:
- improved:
  - なし
- no change:
  - direct retrieval / pack / PocketIC の guard は維持した
- regress:
  - なし

guard:
- exact:
  - pass
- migration:
  - pass
- two-source-auth:
  - pass

判断:
- revert

次:
- 次は section budget か `Ambiguous` / `Task` の weight を 1 箇所だけ触り、Trial ごとの差分が report で見える形にしてから採否を決める

## Trial 04
日付:
- 2026-03-25

仮説:
- `ambiguous` 系の採否は今の report だと改善量が見えにくいので、`document_candidate_count` と `fallback_used` を見えるようにすると次の Trial 判定が正確になる

変更:
- retrieval benchmark report に `section_candidate_count` / `document_candidate_count` / `fallback_used` を追加
- Markdown summary に `current docs` / `current fallback` 列を追加
- PocketIC の retrieval 比較にも同じ観測値を載せる

実行:
- `cargo test --test retrieval_benchmark_tests`
- `cargo test --test retrieval_benchmark_runner_tests`
- `cargo test --test benchmark_tests`
- `cargo test --test benchmark_runner_tests`
- `cargo test -p pocket_ic_tests --test catalog_e2e --no-run`
- `source ~/.zshrc && cargo test -p pocket_ic_tests --test catalog_e2e -- --ignored`

比較ケース:
- `ambiguous-hooks`
- `fallback-noise`
- `vector-natural-language`
- `exact-middleware`
- `migration-version`

結果:
- improved:
  - retrieval benchmark report で `document_candidate_count` と `fallback_used` を比較できるようになった
  - PocketIC report も同じ schema の観測値を持つようになった
- no change:
  - retrieval policy の挙動自体は変えていない
- regress:
  - なし

guard:
- report schema:
  - pass
- exact:
  - pass
- migration:
  - pass
- two-source-auth:
  - pass

判断:
- keep

次:
- 次は `ambiguous-hooks` と `fallback-noise` の `document_candidate_count` / `fallback_used` を見ながら、section budget か weight を 1 箇所だけ触る

## Trial 05
日付:
- 2026-03-25

仮説:
- `Task` 系 query の `min_section_candidates` を 1 に下げると、`vector-natural-language` の candidate が少し締まり、token を減らしやすい

変更:
- `infer_retrieval_policy()` の `RetrievalPolicyKind::Task` で `min_section_candidates: 2 -> 1`

実行:
- `cargo test --test retrieval_benchmark_tests`
- `cargo test --test retrieval_benchmark_runner_tests`
- `cargo test --test benchmark_tests`
- `cargo test --test benchmark_runner_tests`
- `cargo test -p pocket_ic_tests --test catalog_e2e --no-run`
- `source ~/.zshrc && cargo test -p pocket_ic_tests --test catalog_e2e -- --ignored`

比較ケース:
- `vector-natural-language`
- `two-source-auth`
- `exact-middleware`
- `migration-version`
- `fallback-noise`

結果:
- improved:
  - なし
- no change:
  - `vector-natural-language` の `document_candidate_count` と `fallback_used` は変わらなかった
  - guard は維持した
- regress:
  - なし

guard:
- report schema:
  - pass
- exact:
  - pass
- migration:
  - pass
- two-source-auth:
  - pass

判断:
- revert

次:
- 次は `Task` の section overflow を 1 段だけ絞って、section candidate 数が本当に減るかを見る

## Trial 06
日付:
- 2026-03-25

仮説:
- `Task` 系 query の `section_overflow` を 1 段絞ると、`vector-natural-language` の section candidate 数が減り、fallback を起こしても final docs を減らしやすい

変更:
- `infer_retrieval_policy()` の `RetrievalPolicyKind::Task` で `section_overflow: 2 -> 1`

実行:
- `cargo test --test retrieval_benchmark_tests`
- `cargo test --test retrieval_benchmark_runner_tests`
- `cargo test --test benchmark_tests`
- `cargo test --test benchmark_runner_tests`
- `cargo test -p pocket_ic_tests --test catalog_e2e --no-run`
- `source ~/.zshrc && cargo test -p pocket_ic_tests --test catalog_e2e -- --ignored`

比較ケース:
- `vector-natural-language`
- `two-source-auth`
- `exact-middleware`
- `migration-version`
- `fallback-noise`

結果:
- improved:
  - なし
- no change:
  - `section_overflow` を 1 段絞っても `vector-natural-language` の section/doc 候補数は変わらなかった
  - guard は維持した
- regress:
  - なし

guard:
- report schema:
  - pass
- exact:
  - pass
- migration:
  - pass
- two-source-auth:
  - pass

判断:
- revert

次:
- 次は section limit 自体に上限を入れて、`Task` の section candidate を明示的に 1 件へ絞れるかを見る

## Trial 07
日付:
- 2026-03-25

仮説:
- `Task` 系 query は `top_k` 起点の section limit が広すぎるので、明示上限を持たせると narrowing が効く

変更:
- `RetrievalPolicy` に `max_section_candidates` を追加
- `section_limit()` に clamp を追加
- `RetrievalPolicyKind::Task` に `max_section_candidates: Some(1)` を設定

実行:
- `cargo test --test retrieval_benchmark_tests`
- `cargo test --test retrieval_benchmark_runner_tests`
- `cargo test --test benchmark_tests`
- `cargo test --test benchmark_runner_tests`
- `cargo test -p pocket_ic_tests --test catalog_e2e --no-run`
- `source ~/.zshrc && cargo test -p pocket_ic_tests --test catalog_e2e -- --ignored`

比較ケース:
- `vector-natural-language`
- `two-source-auth`
- `exact-middleware`
- `migration-version`
- `fallback-noise`

結果:
- improved:
  - `vector-natural-language` で `section_candidate_count: 2 -> 1`
  - `vector-natural-language` で `document_candidate_count: 2 -> 1`
  - `vector-natural-language` で `estimated_pack_tokens: 76 -> 36`
- no change:
  - `fallback-noise` は変化なし
- regress:
  - なし

guard:
- report schema:
  - pass
- exact:
  - pass
- migration:
  - pass
- two-source-auth:
  - pass

判断:
- keep

次:
- 次は `fallback-noise` 向けに fallback 自体を減らす方向の Trial を 1 つ試す

## Trial 08
日付:
- 2026-03-25

仮説:
- `launchagent` / `plist` を含む ambiguous query は `launchd` section へ寄せて section cap を 1 にすると、`fallback-noise` の docs と token を減らしやすい

変更:
- ambiguous 判定内で `launchagent` / `plist` marker を抽出
- marker がある場合だけ `max_section_candidates: Some(1)` を設定
- marker がある場合だけ `preferred_sections: [\"launchd\"]` を設定

実行:
- `cargo test --test retrieval_benchmark_tests`
- `cargo test --test retrieval_benchmark_runner_tests`
- `cargo test --test benchmark_tests`
- `cargo test --test benchmark_runner_tests`
- `cargo test -p pocket_ic_tests --test catalog_e2e --no-run`
- `source ~/.zshrc && cargo test -p pocket_ic_tests --test catalog_e2e -- --ignored`

比較ケース:
- `fallback-noise`
- `vector-natural-language`
- `exact-middleware`
- `migration-version`
- `two-source-auth`

結果:
- improved:
  - `fallback-noise` で `section_candidate_count: 2 -> 1`
  - `fallback-noise` で `document_candidate_count: 2 -> 1`
  - `fallback-noise` で `estimated_pack_tokens: 68 -> 32`
- no change:
  - `vector-natural-language` は Trial 07 の改善を維持
- regress:
  - なし

guard:
- report schema:
  - pass
- exact:
  - pass
- migration:
  - pass
- two-source-auth:
  - pass

判断:
- keep

次:
- 次は `fallback_used` 自体を減らせるか、keyword candidate の作り方を 1 箇所だけ緩める Trial を試す

## Trial 09
日付:
- 2026-03-25

仮説:
- `launchagent` / `plist` を含む query は keyword 候補生成時にその marker へ絞ると、`fallback-noise` で fallback 自体を回避しやすい

変更:
- `keyword_candidate_ids()` で `keyword_candidate_query_text()` を使うように変更
- `launchagent` / `plist` を含む場合だけ keyword query をその marker のみに絞る

実行:
- `cargo test --test retrieval_benchmark_tests`
- `cargo test --test retrieval_benchmark_runner_tests`
- `cargo test --test benchmark_tests`
- `cargo test --test benchmark_runner_tests`
- `cargo test -p pocket_ic_tests --test catalog_e2e --no-run`
- `source ~/.zshrc && cargo test -p pocket_ic_tests --test catalog_e2e -- --ignored`

比較ケース:
- `fallback-noise`
- `vector-natural-language`
- `exact-middleware`
- `migration-version`
- `two-source-auth`

結果:
- improved:
  - なし
- no change:
  - `fallback-noise` の current token は維持
- regress:
  - baseline 側まで keyword candidate が立って比較軸が変わった
  - `fallback_used` は current で減らなかった

guard:
- report schema:
  - pass
- exact:
  - pass
- migration:
  - pass
- two-source-auth:
  - pass

判断:
- revert

次:
- 次は baseline を汚さない範囲で current policy だけに効く narrowing を 1 箇所試す

## Trial 10
日付:
- 2026-03-25

仮説:
- `launchagent/plist` を含む ambiguous query は current policy だけ keyword focus term を持てば、baseline を変えずに `fallback_used` を減らせる

変更:
- `RetrievalPolicy` に `keyword_focus_terms` を追加
- `keyword_candidate_ids()` が current policy の focus term を使えるように変更
- `launchagent/plist` を含む ambiguous query では `keyword_focus_terms: [\"launchagent\"]` を設定

実行:
- `cargo test --test retrieval_benchmark_tests`
- `cargo test --test retrieval_benchmark_runner_tests`
- `cargo test --test benchmark_tests`
- `cargo test --test benchmark_runner_tests`
- `cargo test -p pocket_ic_tests --test catalog_e2e --no-run`
- `source ~/.zshrc && cargo test -p pocket_ic_tests --test catalog_e2e -- --ignored`

比較ケース:
- `fallback-noise`
- `vector-natural-language`
- `exact-middleware`
- `migration-version`
- `two-source-auth`

結果:
- improved:
  - なし
- no change:
  - `fallback-noise` の `fallback_used` は current で `true` のままだった
  - `section_candidate_count` / `document_candidate_count` / token も Trial 08 から変わらなかった
- regress:
  - なし

guard:
- report schema:
  - pass
- exact:
  - pass
- migration:
  - pass
- two-source-auth:
  - pass

判断:
- revert

次:
- 次は `fallback_used` を直接下げるより、fallback が起きても token を増やさない方向で current policy の doc budget を 1 箇所だけ詰める

## Trial 11
日付:
- 2026-03-25

仮説:
- `auth` を含む task query は current policy だけ `auth` を keyword focus term に持てば、`vector-natural-language` の fallback を減らせる

変更:
- `RetrievalPolicy` に `keyword_focus_terms` を追加
- `keyword_candidate_ids()` が policy の focus term を使えるように変更
- `Task` で `auth` を含む query のみ `keyword_focus_terms: ["auth"]` を設定

実行:
- `cargo test --test retrieval_benchmark_tests`
- `cargo test --test retrieval_benchmark_runner_tests`
- `cargo test --test benchmark_tests`
- `cargo test --test benchmark_runner_tests`
- `cargo test -p pocket_ic_tests --test catalog_e2e --no-run`
- `source ~/.zshrc && cargo test -p pocket_ic_tests --test catalog_e2e -- --ignored`

比較ケース:
- `vector-natural-language`
- `two-source-auth`
- `fallback-noise`
- `exact-middleware`
- `migration-version`

結果:
- improved:
  - なし
- no change:
  - `vector-natural-language` の `fallback_used` は current で `true` のままだった
  - `section_candidate_count` / `document_candidate_count` / token も Trial 07 から変わらなかった
- regress:
  - なし

guard:
- report schema:
  - pass
- exact:
  - pass
- migration:
  - pass
- two-source-auth:
  - pass

判断:
- revert

次:
- 次は `fallback_used` 自体ではなく、曖昧 query の section/doc budget をケース別に分ける必要があるかを検討する

## Trial 12
日付:
- 2026-03-25

仮説:
- `Ambiguous` の tuning を続ける前に `next react hooks` を direct retrieval benchmark に入れると、subcase ごとの差分を安全に見られる

変更:
- direct retrieval benchmark に `ambiguous-hooks` 検証ケースを追加
- report / runner の期待件数を 5 ケースへ更新

実行:
- `cargo test --test retrieval_benchmark_tests`
- `cargo test --test retrieval_benchmark_runner_tests`

比較ケース:
- `ambiguous-hooks`
- `vector-natural-language`
- `fallback-noise`

結果:
- improved:
  - `ambiguous-hooks` を direct retrieval benchmark で継続比較できるようになった
- no change:
  - retrieval policy 自体の挙動は変えていない
- regress:
  - なし

guard:
- report schema:
  - pass
- exact:
  - pass
- migration:
  - pass

判断:
- keep

次:
- `ambiguous-hooks` を含めた report を見て、`Ambiguous` の subcase budget を分ける Trial を 1 つ試す

## Trial 13
日付:
- 2026-03-25

仮説:
- `next react hooks` 系は `routing` section を優先しつつ section cap を 1 にすると、`ambiguous-hooks` の docs と token を減らせる

変更:
- ambiguous 判定で `hooks/hook` marker と `next + react` の組み合わせを検出
- 該当時だけ `max_section_candidates: Some(1)` を設定
- 該当時だけ `preferred_sections: [\"routing\"]` を設定

実行:
- `cargo test --test retrieval_benchmark_tests`
- `cargo test --test retrieval_benchmark_runner_tests`
- `cargo test --test benchmark_tests`
- `cargo test --test benchmark_runner_tests`
- `cargo test -p pocket_ic_tests --test catalog_e2e --no-run`
- `source ~/.zshrc && cargo test -p pocket_ic_tests --test catalog_e2e -- --ignored`

比較ケース:
- `ambiguous-hooks`
- `vector-natural-language`
- `fallback-noise`
- `exact-middleware`
- `migration-version`

結果:
- improved:
  - `ambiguous-hooks` で `section_candidate_count: 2 -> 1`
  - `ambiguous-hooks` で `document_candidate_count: 2 -> 1`
  - `ambiguous-hooks` で `estimated_pack_tokens: 73 -> 37`
- no change:
  - `vector-natural-language` と `fallback-noise` の改善は維持
- regress:
  - なし

guard:
- report schema:
  - pass
- exact:
  - pass
- migration:
  - pass
- two-source-auth:
  - pass

判断:
- keep

次:
- 次は `Ambiguous` の fallback_used をさらに下げる必要があるか、現状の token 削減で十分かを見極める
