# wakeru

[English](README.md) | [日本語](README_ja.md)

[![CI](https://github.com/tokoba/wakeru/workflows/CI/badge.svg)](https://github.com/tokoba/wakeru/actions)
[![Security](https://github.com/tokoba/wakeru/workflows/Security/badge.svg)](https://github.com/tokoba/wakeru/actions)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/rust-2024-orange.svg)](https://www.rust-lang.org)

Japanese morphological analysis and BM25 full-text search library –
fast filtering and re-ranking for RAG pipelines.

## Overview

**wakeru** (meaning "to split / separate" in Japanese) is a Rust library
that combines Japanese morphological analysis with full-text search.
It is designed for Retrieval-Augmented Generation (RAG) workflows and offers:

- **Japanese morphological analysis** using [vibrato-rkyv](https://github.com/akiradeveloper/vibrato) for high-speed tokenization
- **BM25 full-text search** powered by [tantivy](https://github.com/quickwit-oss/tantivy)
- **RAG-oriented optimization** via part-of-speech filtering (focus on nouns, verbs, adjectives)
- **Multi-language support** for Japanese (IPADIC/UniDic) and English (stemming)

## Quickstart

### Installation

Add the following to your `Cargo.toml`:

```toml
[dependencies]
wakeru = "0.1.1"
```

### Basic usage

```rust
use wakeru::{WakeruConfig, WakeruService, Language};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize the service
    let config = WakeruConfig::builder()
        .language(Language::Japanese)
        .build()?;

    let service = WakeruService::new(config).await?;

    // Index a document
    service
        .add_document(
            "doc1",
            "Rustは高速で安全なシステムプログラミング言語です",
            &["programming", "systems"],
        )
        .await?;

    // Full-text search
    let results = service.search("Rust プログラミング", 10).await?;

    for result in results {
        println!("score: {:.2}, document: {}", result.score, result.id);
    }

    Ok(())
}
```

### More examples

You can run the bundled example with:

```bash
cargo run --example example_wakeru
```

## Running the sample

The `scripts` directory contains sample scripts for performing morphological analysis using wakeru.

### Start wakeru-api

Open a terminal and start wakeru-api.
By default, http://127.0.0.1:5530 is used.

```sh
$ ./scripts/run_api.sh
warning: C:\\Drive\\rust\\wakeru\\Cargo.toml: unused manifest key: workspace.dev-dependencies
    Finished `dev` profile [unoptimized & debuginfo] target(s) in 0.28s
     Running `target\\debug\\wakeru-api.exe`
2026-02-19T12:09:57.721670Z  INFO wakeru_api: Configuration loaded preset=UnidicCwj
2026-02-19T12:09:57.723958Z  INFO wakeru_api: Morphological analysis service initialized
2026-02-19T12:09:57.725096Z  INFO wakeru_api::api::routes: Starting server: http://127.0.0.1:5530
```

### Sending a request to wakeru-api

Open another terminal and send a morphological analysis request to wakeru-api, specifying a JSON file.

```sh
$ ./scripts/run_api_test.sh
{"tokens":[{"surface":"Hello","pos":"noun","lemma":"hello","start_byte":0,"end_byte":5,"should_index":true},{"surface":"world","pos":"noun","lemma":"world","start_byte":6,"end_byte":11,"should_index":true}]}
```

The result formatted in JSON looks like the following:

[Input](./scripts/sample_input_text.json) | [Output](./scripts/wakeru_api_result.json)

## Architecture

```text
┌─────────────────────────────────────────────────┐
│  wakeru crate                                   │
│                                                 │
│  ┌──────────────┐  ┌──────────┐  ┌──────────┐   │
│  │ DictionaryMgr │→│ Tokenizer│→│ IndexMgr │   │
│  │ (dictionary)  │  │ (morph.) │  │ (index) │   │
│  └──────────────┘  └──────────┘  └────┬─────┘   │
│                                      │          │
│                                      ▼          │
│                              ┌──────────────┐   │
│                              │ SearchEngine │   │
│                              │   (BM25)     │   │
│                              └──────┬───────┘   │
└─────────────────────────────────────┼───────────┘
                                      │
                                      ▼
                              SearchResult[]
                                      │
                                      ▼
                               RAG pipeline
```

## Crates

| Crate        | Description                                    |
|--------------|------------------------------------------------|
| `wakeru`     | Core morphological analysis and search library |
| `wakeru-api` | Axum-based Web API server                      |

## Development environment

### Requirements

- **Rust**: Edition 2024, MSRV 1.93.0
- **OS**: Windows, macOS, Linux

### Development commands

```bash
# Build
cargo build --workspace

# Tests (using nextest)
cargo nextest run

# Format check
cargo fmt --all -- --check

# Lint
cargo clippy --workspace --all-targets -- -D warnings

# Generate documentation
cargo doc --workspace --no-deps

# Security audit
cargo deny check
```

## Part-of-speech filtering

To improve retrieval quality for RAG (Retrieval-Augmented Generation), wakeru applies part-of-speech (POS) filtering to focus on informative terms.

**Included POS:**

- Nouns (common nouns, proper nouns, "sahen" verbal nouns)
- Verbs
- Adjectives

**Excluded POS:**

- Particles (e.g., は, が, を, に, で)
- Auxiliary verbs (e.g., です, ます, だ, ない)
- Symbols
- Pronouns and non-independent nouns

This filtering reduces noise from function words and helps ranking focus on content-bearing terms.

## License

MIT License – see [LICENSE](LICENSE) for details.

## Contributing

Contributions are welcome!
Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## Related links

- [vibrato-rkyv](https://github.com/akiradeveloper/vibrato) – Japanese morphological analysis engine
- [tantivy](https://github.com/quickwit-oss/tantivy) – Full-text search engine
- [Documentation on docs.rs](https://docs.rs/wakeru)

## Contact

- **GitHub**: <https://github.com/tokoba/wakeru>
- **Author**: tokoba

## Made with ❤️ in Rust
