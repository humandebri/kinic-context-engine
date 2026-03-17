# KINIC Context Runtime Plan

## 1. 目的
KINIC Memory を単なる個人向け RAG ではなく、`source` ごとに分離された canister 群から必要な文脈だけを集め、CLI / MCP から再利用できる `context runtime` に拡張する。
目標は「検索結果」ではなく、LLM や agent がそのまま使える `evidence pack` を返すこと。

## 2. 提供価値と非目標
提供価値:
- 質問に応じて参照すべき source を解決する
- 関連 canister だけを並列で呼び出す
- version / freshness / provenance 付きで根拠を返す
- private memory と public knowledge を安全に併用する
- CLI / MCP から同じ runtime を使えるようにする

非目標:
- 全 source への無差別 fan-out
- 単一巨大ベクトル DB への一本化
- citation のない要約専用 API
- private memory が public fact を上書きする設計

## 3. 設計原則
- canister は `source 単位` で分ける
- retrieval 前に resolver で候補を絞る
- 並列呼び出しは `top-k + exploratory overflow` に制限する
- source ごとに metadata と freshness ルールを持つ
- evidence pack を最終成果物にする
- provenance と contradiction warning を必須にする

## 4. システム構成
### 4.1 Registry Canister
責務:
- `source_id` と `canister_id` の対応管理
- alias 解決
- source 種別、trust level、version 対応状況の管理
- freshness policy、language、health 情報の公開

### 4.2 Resolver / Router Canister
責務:
- query の intent 判定
- entity 抽出
- source 候補の選定
- version hint / locale / time sensitivity の解決
- fan-out 対象の最終決定

重要な方針:
- ここは vector search ではなく routing を担当する
- 候補 canister は原則 `3〜5件`、探索枠を含めても `5〜7件` 程度に抑える

### 4.3 Source Canister
責務:
- source 内検索
- metadata / version filter
- chunk / snippet / citation の返却
- source 固有 schema の管理

例:
- `/vercel/next.js`
- `/supabase/docs`
- `/supabase/auth`
- `/react/docs`
- `/travel/japan/kyoto-city-official`

方針:
- `Next.js v15` のような version は canister を分けず metadata で扱う
- source ごとに parser と ranking signal を調整する

### 4.4 Aggregator / Pack Builder
責務:
- 並列 query 実行
- cross-source rerank
- dedup
- contradiction 検出
- stale warning 生成
- token budget 内への圧縮
- final evidence pack 生成

### 4.5 User Memory Canister
責務:
- 個人メモリの ingest / search / pin / forget
- namespace 管理
- private/public merge 用の補助文脈提供

重要な方針:
- private memory は補助文脈
- public official source と競合した場合は、private memory を警告付きで残しつつ public を優先する

## 5. 主要データモデル
### 5.1 Source Registry Entry
```json
{
  "source_id": "/vercel/next.js",
  "aliases": ["next", "nextjs", "next.js"],
  "domain_type": "code_docs",
  "supported_versions": ["13", "14", "15"],
  "trust_level": "official",
  "freshness_policy": "periodic",
  "canister_id": "aaaa-bbbb-cccc"
}
```

### 5.2 Retrieval Chunk
```json
{
  "chunk_id": "nextjs:middleware:15:0012",
  "source_id": "/vercel/next.js",
  "version": "15",
  "section": "middleware",
  "heading": "Matcher",
  "content": "...",
  "symbols": ["middleware", "matcher", "NextRequest"],
  "trust": "official",
  "retrieved_at": "2026-03-16T10:00:00Z",
  "source_url": "https://nextjs.org/docs/...",
  "language": "en"
}
```

### 5.3 Evidence Pack
```json
{
  "query": "protect route in next.js with supabase auth",
  "resolved_sources": ["/vercel/next.js", "/supabase/auth"],
  "evidence": [
    {
      "source_id": "/vercel/next.js",
      "title": "Middleware",
      "version": "15",
      "snippet": "...",
      "trust": "official",
      "retrieved_at": "2026-03-16T10:00:00Z",
      "citation": "https://nextjs.org/docs/...",
      "stale": false
    }
  ],
  "warnings": [],
  "pack_summary": "...",
  "token_budget": 3000
}
```

## 6. Query フロー
1. `resolve`: query から intent / entities / candidate sources を決める
2. `select`: top-k source と exploratory source を決める
3. `parallel query`: 選ばれた canister に並列問い合わせする
4. `merge`: rerank / dedup / contradiction check を行う
5. `pack`: evidence pack に整形して返す

## 7. CLI / MCP の最小スコープ
CLI:
- `kinic resolve "<query>"`
- `kinic query <source_id> "<query>"`
- `kinic pack "<query>"`
- `kinic cite <pack_id>`
- `kinic memory ingest <path> --namespace <ns>`
- `kinic memory search "<query>"`
- `kinic memory pin "<text>"`
- `kinic memory forget "<text>"`

MCP tools:
- `resolve_sources`
- `query_source`
- `build_context_pack`
- `search_memory`
- `explain_provenance`

## 8. Ranking / Merge ルール
code docs:
- exact source match を強く優先
- exact symbol match を優先
- version match を優先
- official docs を優先
- semantic similarity は補助に使う

travel / live facts:
- place/entity exactness を優先
- freshness を強く優先
- official source を優先
- geo relevance を加味する

private memory:
- namespace
- pin
- recency
- semantic similarity

競合時の原則:
- public official fact を優先
- private memory は補助情報として残す
- contradiction warning を返す

## 9. canister 配置方針
dedicated canister:
- 高頻度・高価値 source は専用 canister を持つ
- 例: Next.js、React、Supabase

shared canister:
- 低頻度 source は domain shared canister にまとめる
- 例: niche library 群、小規模 travel source 群

on-chain に置くもの:
- ownership
- access policy
- provenance metadata
- audit log
- namespace metadata
- index pointer

off-chain / edge に置くもの:
- bulky raw content
- crawl cache
- parsed HTML/PDF text
- rerank 用一時データ
- live API results

## 10. 実装フェーズ
### Phase 1: Code Context MVP
対象:
- Next.js
- React
- Supabase
- Tailwind
- TypeScript

作るもの:
- registry
- resolver
- source canister 共通 schema
- pack builder
- CLI / MCP 最小セット
- citation / stale warning

成功条件:
- source resolution accuracy が高い
- irrelevant chunk rate が低い
- version-aware retrieval ができる

### Phase 2: Private Memory Merge
作るもの:
- namespace
- ingest / pin / forget
- merge policy
- provenance explanation
- contradiction handling

成功条件:
- private/public の競合で誤誘導しない
- memory 操作が CLI / MCP から一貫して使える

### Phase 3: Live Context Expansion
対象:
- travel
- 一般の live facts

作るもの:
- official-source-first resolver
- date / geo aware retrieval
- freshness policy
- stale warning 強化

成功条件:
- 営業時間や日付依存情報の誤答率を抑えられる
- citation と `as_of` を安定して返せる

## 11. 評価指標
- source resolution accuracy
- citation correctness
- stale fact rate
- irrelevant chunk rate
- median latency
- token efficiency
- contradiction detection rate

## 12. 直近の実装タスク
1. `source registry` の Candid と Rust interface を定義する
2. `resolver` の入出力 schema を定義する
3. `source canister` 共通 retrieval interface を定義する
4. `evidence pack` の JSON schema を固定する
5. `kinic resolve/query/pack` の CLI 仕様を決める
6. MVP source として `Next.js / Supabase / React` の 3 source を載せる

## 13. 未確定事項
- resolver を rule-based で始めるか、軽量 classifier を併用するか
- embedding index を canister 内に持つか、pointer のみ持つか
- source update pipeline をどこで回すか
- CLI と MCP のレスポンスをどこまで共通化するか
- live data 取得時の利用規約とキャッシュポリシー

## 14. 現時点の結論
進む方向は妥当です。
ただし勝ち筋は `source ごとの canister 分割` そのものではなく、`resolver-based selective fan-out` と `evidence pack` にあります。
最初の実装は Code Context MVP に絞るべきです。ここで解像度と再現性を証明してから、private memory と live context に広げるのが安全です。
