# コントリビューションガイド

wakeruプロジェクトへの貢献を検討いただきありがとうございます。このドキュメントは、プロジェクトへの効果的な貢献方法を説明します。

## 開発環境のセットアップ

### 要件

- **Rust**: Edition 2024, MSRV 1.93.0 以上
- **Git**: 最新バージョン
- **cargo-llvm-cov**: カバレッジ計測（オプション）
- **cargo-nextest**: 高速テスト実行（推奨）

### リポジトリのクローン

```bash
git clone https://github.com/tokoba/wakeru.git
cd wakeru
```

### 開発用ツールのインストール

```bash
# nextest（高速テスト）
cargo install cargo-nextest

# cargo-llvm-cov（カバレッジ）
cargo install cargo-llvm-cov

# markdownlint-cli2（Markdown Lint）
npm install -g markdownlint-cli2
```

## 開発ワークフロー

### 1. ブランチの作成

```bash
git checkout -b feature/your-feature-name
# または
git checkout -b fix/your-bug-fix
```

### 2. 変更の実装

#### コーディング規約

- **日本語コメント**: コメントとドキュメントは日本語で記述
- **テストファースト**: TDD（Red-Green-Refactor）を原則
- **100行制限**: 関数は100行以内に抑える
- **ファイル分割**: 1ファイル500行以下を目標

#### 品質チェック

変更を加えた後は、必ず以下のコマンドで検証してください：

```bash
# フォーマットチェック
cargo fmt --all -- --check

# 厳格Lint
cargo clippy --workspace --all-targets -- -D warnings

# テスト（nextest）
cargo nextest run

# ドキュメント生成
cargo doc --workspace --no-deps

# セキュリティチェック
cargo deny check
```

### 3. コミット

```bash
git add .
git commit -m "feat: 機能の説明"
```

#### コミットメッセージ規約

- `feat:` 新機能
- `fix:` バグ修正
- `docs:` ドキュメントのみの変更
- `style:` コードフォーマット（セミコロンなど）
- `refactor:` リファクタリング
- `test:` テストの追加・修正
- `chore:` ビルドプロセスやツールの変更

### 4. プルリクエストの作成

```bash
git push origin feature/your-feature-name
```

GitHubでプルリクエストを作成し、以下を記入してください：

- 変更内容の説明
- 関連するIssue番号
- テスト方法
- スクリーンショット（該当する場合）

## プルリクエストのレビュー基準

すべてのプルリクエストは以下をパスする必要があります：

- [ ] すべてのCIチェックがパス
- [ ] コードカバレッジが低下していない
- [ ] 新しい機能にはテストが含まれている
- [ ] ドキュメントが更新されている
- [ ] `cargo fmt` がパス
- [ ] `cargo clippy -- -D warnings` がパス

## テスト戦略

### テストカテゴリ

1. **ユニットテスト**: モジュール内の関数テスト
2. **統合テスト**: 複数モジュールの連携テスト
3. **辞書ダウンロードテスト**: `#[ignore]` 付き（重い処理）

### テスト実行

```bash
# 通常テスト
cargo nextest run

# ignoredテストを含む全テスト
cargo nextest run -- --ignored

# 特定クレートのテスト
cargo nextest run -p wakeru

# doctestを含むテスト
cargo nextest run --with-doctests
```

## 辞書キャッシュについて

日本語形態素解析には辞書が必要です：

- **IPADIC**: 約30MB（推奨）
- **UniDic**: 約700MB

初回実行時に辞書が自動ダウンロードされます。2回目以降はキャッシュが使用されます。

## バグ報告

バグを見つけた場合は、以下を含むIssueを作成してください：

- 再現手順
- 期待される動作
- 実際の動作
- 環境情報（OS, Rustバージョン）
- エラーログ

## 機能リクエスト

新しい機能を提案する場合は、以下を含むIssueを作成してください：

- 機能の説明
- ユースケース
- 代替案
- 実装の提案（該当する場合）

## ライセンス

貢献されるコードは、プロジェクトと同じ[MIT License](LICENSE)の下でライセンスされることに同意したものとみなされます。

## コミュニケーション

- **GitHub Issues**: バグ報告、機能リクエスト
- **GitHub Discussions**: 質問、アイデア共有
- **Pull Requests**: コードレビューとマージ

---

再度、貢献を検討いただきありがとうございます！
