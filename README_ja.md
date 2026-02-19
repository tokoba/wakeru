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
