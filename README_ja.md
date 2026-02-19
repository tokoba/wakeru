# wakeru

[English](README.md) | [日本語](README_ja.md)

[![CI](https://github.com/tokoba/wakeru/workflows/CI/badge.svg)](https://github.com/tokoba/wakeru/actions)
[![Security](https://github.com/tokoba/wakeru/workflows/Security/badge.svg)](https://github.com/tokoba/wakeru/actions)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/rust-2024-orange.svg)](https://www.rust-lang.org)

日本語形態素解析と BM25 全文検索ライブラリ - RAG パイプライン向けの高速フィルタリング・リランキングを実現

## 概要

**wakeru**（分ける）は、日本語形態素解析と全文検索を組み合わせた Rust ライブラリです。以下の機能を提供します：

- **日本語形態素解析**: [vibrato-rkyv](https://github.com/akiradeveloper/vibrato) を使用した高速解析
- **BM25 全文検索**: [tantivy](https://github.com/quickwit-oss/tantivy) によるスコアリング検索
- **RAG 最適化**: 検索精度を高める品詞フィルタリング（名詞・動詞・形容詞にフォーカス）
- **マルチ言語対応**: 日本語（IPADIC/UniDic）と英語（stemming）

## サンプルの実行

scripts ディレクトリーに wakeru を用いて形態素解析を行うためのサンプルスクリプトを置いています。

### wakeru-api を起動

ターミナルを起動して wakeru-api を起動します。
デフォルトでは <http://127.0.0.1:5530> が使用されます。

```sh
$ ./scripts/run_api.sh
warning: C:\Drive\rust\wakeru\Cargo.toml: unused manifest key: workspace.dev-dependencies
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.28s
     Running `target\debug\wakeru-api.exe`
2026-02-19T12:09:57.721670Z  INFO wakeru_api: 設定を読み込みました preset=UnidicCwj
2026-02-19T12:09:57.723958Z  INFO wakeru_api: 形態素解析サービスを初期化しました
2026-02-19T12:09:57.725096Z  INFO wakeru_api::api::routes: サーバーを起動します: http://127.0.0.1:5530
```

### wakeru-api へのリクエスト

別のターミナルを起動して wakeru-api に対して json ファイルを指定して形態素解析依頼を行います。

```sh
$ ./scripts/run_api_test.sh
{"tokens":[{"surface":"親譲り","feature":"名詞,普通名詞,一般,*,*,*,オヤユズリ,親譲り,親譲り,オヤユズリ,親譲り,オヤユズリ,和,*,*,*,*,*,*,体,オヤユズリ,オヤユズリ,オヤユズリ,オヤユズリ,3,C1,*,15020986726490624,54646","pos":"名詞","pos_detail1":"普通名詞","pos_detail2":"一般","pos_detail3":"*","lemma":"オヤユズリ","reading":"親譲り","pronunciation":"親譲り","start_byte":0,"end_byte":9,"should_index":true}}
```

json フォーマットで成型した結果は以下のようになっています。

[入力](./scripts/sample_input_text.json) | [出力](./scripts/wakeru_api_result.json)

```json
{
  "tokens": [
    {
      "surface": "親譲り",
      "feature": "名詞,普通名詞,一般,*,*,*,オヤユズリ,親譲り,親譲り,オヤユズリ,親譲り,オヤユズリ,和,*,*,*,*,*,*,体,オヤユズリ,オヤユズリ,オヤユズリ,オヤユズリ,3,C1,*,15020986726490624,54646",
      "pos": "名詞",
      "pos_detail1": "普通名詞",
      "pos_detail2": "一般",
      "pos_detail3": "*",
      "lemma": "オヤユズリ",
      "reading": "親譲り",
      "pronunciation": "親譲り",
      "start_byte": 0,
      "end_byte": 9,
      "should_index": true
    },
    {
      "surface": "の",
      "feature": "助詞,格助詞,*,*,*,*,ノ,の,の,ノ,の,ノ,和,*,*,*,*,*,*,格助,ノ,ノ,ノ,ノ,*,名詞%F1,*,7968444268028416,28989",
      "pos": "助詞",
      "pos_detail1": "格助詞",
      "pos_detail2": "*",
      "pos_detail3": "*",
      "lemma": "ノ",
      "reading": "の",
      "pronunciation": "の",
      "start_byte": 9,
      "end_byte": 12,
      "should_index": false
    },
    {
      "surface": "無鉄砲",
      "feature": "名詞,普通名詞,形状詞可能,*,*,*,ムテッポウ,無鉄砲,無鉄砲,ムテッポー,無鉄砲,ムテッポー,漢,*,*,*,*,*,*,体,ムテッポウ,ムテッポウ,ムテッポウ,ムテッポウ,2,C1,*,10213372134040064,37156",
      "pos": "名詞",
      "pos_detail1": "普通名詞",
      "pos_detail2": "形状詞可能",
      "pos_detail3": "*",
      "lemma": "ムテッポウ",
      "reading": "無鉄砲",
      "pronunciation": "無鉄砲",
      "start_byte": 12,
      "end_byte": 21,
      "should_index": true
    },
    {
      "surface": "で",
      "feature": "助動詞,*,*,*,助動詞-ダ,連用形-一般,ダ,だ,で,デ,だ,ダ,和,*,*,*,*,*,*,助動,デ,ダ,デ,ダ,*,名詞%F1,*,6299110739157633,22916",
      "pos": "助動詞",
      "pos_detail1": "*",
      "pos_detail2": "*",
      "pos_detail3": "*",
      "lemma": "ダ",
      "reading": "だ",
      "pronunciation": "で",
      "start_byte": 21,
      "end_byte": 24,
      "should_index": false
    },
    {
      "surface": "小供",
      "feature": "名詞,普通名詞,一般,*,*,*,コドモ,子供,小供,コドモ,小供,コドモ,和,*,*,*,*,*,*,体,コドモ,コドモ,コドモ,コドモ,0,C2,*,3541535710913024,12884",
      "pos": "名詞",
      "pos_detail1": "普通名詞",
      "pos_detail2": "一般",
      "pos_detail3": "*",
      "lemma": "コドモ",
      "reading": "子供",
      "pronunciation": "小供",
      "start_byte": 24,
      "end_byte": 30,
      "should_index": true
    },
  ],
  "elapsed_ms": 1
}
```

## クイックスタート

### インストール

`Cargo.toml` に以下を追加します：

```toml
[dependencies]
wakeru = "0.1.1"
```

### 基本的な使用例

```rust
use wakeru::{WakeruConfig, WakeruService, Language};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // サービスの初期化
    let config = WakeruConfig::builder()
        .language(Language::Japanese)
        .build()?;

    let service = WakeruService::new(config).await?;

    // ドキュメントの登録
    service
        .add_document(
            "doc1",
            "Rustは高速で安全なシステムプログラミング言語です",
            &["programming", "systems"],
        )
        .await?;

    // 全文検索
    let results = service.search("Rust プログラミング", 10).await?;

    for result in results {
        println!("スコア: {:.2}, ドキュメント: {}", result.score, result.id);
    }

    Ok(())
}
```

### 詳しい使用例

以下のコマンドでサンプルを実行できます：

```bash
cargo run --example example_wakeru
```

## アーキテクチャ

```text
┌─────────────────────────────────────────────────┐
│  wakeru クレート                                │
│                                                 │
│  ┌──────────────┐  ┌──────────┐  ┌──────────┐   │
│  │ DictionaryMgr │→│Tokenizer │→│IndexMgr  │   │
│  │ (辞書管理)    │  │(形態素)  │  │(インデックス)│ │
│  └──────────────┘  └──────────┘  └────┬─────┘   │
│                                      │          │
│                                      ▼          │
│                              ┌──────────────┐   │
│                              │ SearchEngine │   │
│                              │ (BM25検索)   │   │
│                              └──────┬───────┘   │
└─────────────────────────────────────┼───────────┘
                                      │
                                      ▼
                              SearchResult[]
                                      │
                                      ▼
                              RAG パイプラインへ
```

## クレート構成

| クレート     | 説明                                   |
|--------------|----------------------------------------|
| `wakeru`     | 形態素解析・全文検索ライブラリ         |
| `wakeru-api` | Web API サーバー（Axum ベース）        |

## 開発環境

### 要件

- **Rust**: Edition 2024, MSRV 1.93.0
- **OS**: Windows, macOS, Linux

### 開発用コマンド

```bash
# ビルド
cargo build --workspace

# テスト（nextest 使用）
cargo nextest run

# フォーマットチェック
cargo fmt --all -- --check

# Lint
cargo clippy --workspace --all-targets -- -D warnings

# ドキュメント生成
cargo doc --workspace --no-deps

# セキュリティチェック
cargo deny check
```

## 品詞フィルタリング

RAG（Retrieval-Augmented Generation）向けに、検索品質を高めるための品詞フィルタリングを実装しています。

**含める品詞:**

- 名詞（一般、固有名詞、サ変接続）
- 動詞
- 形容詞

**除外する品詞:**

- 助詞（は、が、を、に、で）
- 助動詞（です、ます、だ、ない）
- 記号
- 代名詞、非自立名詞

機能語を除外し、内容語にフォーカスすることで、RAG における検索・リランキング品質の向上を狙います。

## ライセンス

MIT License - 詳細は [LICENSE](LICENSE) を参照してください。

## コントリビューション

コントリビューションを歓迎します！
詳細は [CONTRIBUTING.md](CONTRIBUTING.md) を参照してください。

## 関連リンク

- [vibrato-rkyv](https://github.com/akiradeveloper/vibrato) - 日本語形態素解析エンジン
- [tantivy](https://github.com/quickwit-oss/tantivy) - 全文検索エンジン
- [ドキュメント（docs.rs）](https://docs.rs/wakeru)

## コンタクト

- **GitHub**: <https://github.com/tokoba/wakeru>
- **Author**: tokoba

## Made with ❤️ in Rust
