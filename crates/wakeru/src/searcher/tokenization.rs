//! クエリトークナイゼーションモジュール
//!
//! 検索クエリのトークナイズ処理を提供します。

use std::collections::HashSet;

use tantivy::Term;
use tantivy::schema::Field;
use tantivy::tokenizer::{TextAnalyzer, TokenStream, Tokenizer};

/// クエリ文字列のトークナイズ結果
///
/// # フィールド
/// - `terms`: Tantivy の検索用 Term ベクタ（OR クエリ構築用）
/// - `query_tokens`: ログ出力用のトークン文字列ベクタ
#[derive(Debug, Clone)]
pub(crate) struct TokenizationResult {
  /// ユニークな Term のベクタ（クエリ構築用）
  pub(crate) terms: Vec<Term>,
  /// ユニークなトークン文字列のベクタ（ログ出力用）
  pub(crate) query_tokens: Vec<String>,
}

/// 与えられた Tokenizer を用いてクエリ文字列をトークナイズする純粋関数（ジェネリクス版）
///
/// # 処理内容
/// - 空文字列トークンをスキップ
/// - 重複トークンを除外（最初の出現のみ採用、HashSet 使用）
/// - Term への変換
///
/// # ジェネリクス
/// tantivy 0.25.0 の `Tokenizer` トレイトは `Self: Sized` を要求しているため
/// `&dyn Tokenizer` は使用できません。代わりにジェネリクス `<T: Tokenizer>` を使用します。
///
/// # 引数
/// - `tokenizer`: トークナイザー（`T: Tokenizer` ジェネリクス）
/// - `field`: Term 作成対象のフィールド
/// - `query_str`: トークナイズ対象のクエリ文字列
///
/// # 戻り値
/// ユニークな Term とトークン文字列を含む `TokenizationResult`
#[allow(dead_code)]
pub(crate) fn tokenize_with_tokenizer<T>(
  tokenizer: &mut T,
  field: Field,
  query_str: &str,
) -> TokenizationResult
where
  T: Tokenizer,
{
  let mut token_stream = tokenizer.token_stream(query_str);
  tokenize_from_stream(&mut token_stream, field)
}

/// TextAnalyzer 用のトークナイズ関数
///
/// tantivy 0.25.0 では `TextAnalyzer` が `Tokenizer` トレイトを実装していないため、
/// 専用の関数を用意しています。
///
/// # 引数
/// - `analyzer`: TextAnalyzer（tantivy から取得したもの）
/// - `field`: Term 作成対象のフィールド
/// - `query_str`: トークナイズ対象のクエリ文字列
///
/// # 戻り値
/// ユニークな Term とトークン文字列を含む `TokenizationResult`
pub(crate) fn tokenize_with_text_analyzer(
  analyzer: &mut TextAnalyzer,
  field: Field,
  query_str: &str,
) -> TokenizationResult {
  let mut token_stream = analyzer.token_stream(query_str);
  tokenize_from_stream(&mut token_stream, field)
}

/// トークンストリームから Term を抽出する共通処理
fn tokenize_from_stream<T: TokenStream + ?Sized>(
  token_stream: &mut T,
  field: Field,
) -> TokenizationResult {
  let mut terms = Vec::new();
  let mut seen = HashSet::new();
  let mut query_tokens = Vec::new();

  while token_stream.advance() {
    let token = token_stream.token();

    // 空トークンはスキップ
    if token.text.is_empty() {
      continue;
    }

    // 重複トークンはスキップ（最初の出現のみ採用）
    if !seen.insert(token.text.clone()) {
      continue;
    }

    query_tokens.push(token.text.clone());

    // Tantivy の Term に変換
    let term = Term::from_field_text(field, &token.text);
    terms.push(term);
  }

  TokenizationResult {
    terms,
    query_tokens,
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use tantivy::schema::{Schema, TEXT};
  use tantivy::tokenizer::{SimpleTokenizer, Token, Tokenizer};

  /// 重複トークンが除外されることのテスト
  #[test]
  fn tokenize_with_tokenizer_deduplicates_tokens() {
    // Field を用意
    let mut schema_builder = Schema::builder();
    let text_field = schema_builder.add_text_field("text", TEXT);
    let _schema = schema_builder.build();

    let mut tokenizer = SimpleTokenizer::default();

    let input = "rust rust search rust";
    let result = tokenize_with_tokenizer(&mut tokenizer, text_field, input);

    // 最初の出現順を保ったまま重複除去されていること
    assert_eq!(
      result.query_tokens,
      vec!["rust".to_string(), "search".to_string()]
    );
    assert_eq!(result.terms.len(), result.query_tokens.len());
  }

  /// 空トークンがスキップされ、かつ重複が除外されることのテスト
  ///
  /// SimpleTokenizer では空トークンを生成しないため、
  /// テスト用の Tokenizer を自前で実装します。
  #[derive(Clone)]
  struct TestTokenizer;

  impl Tokenizer for TestTokenizer {
    type TokenStream<'a> = TestTokenStream;

    fn token_stream<'a>(&mut self, _text: &'a str) -> Self::TokenStream<'a> {
      TestTokenStream {
        tokens: vec![
          Token {
            text: "".to_string(),
            ..Default::default()
          },
          Token {
            text: "rust".to_string(),
            ..Default::default()
          },
          Token {
            text: "".to_string(),
            ..Default::default()
          },
          Token {
            text: "rust".to_string(),
            ..Default::default()
          },
          Token {
            text: "search".to_string(),
            ..Default::default()
          },
        ],
        index: 0,
      }
    }
  }

  struct TestTokenStream {
    tokens: Vec<Token>,
    index: usize,
  }

  impl tantivy::tokenizer::TokenStream for TestTokenStream {
    fn advance(&mut self) -> bool {
      if self.index < self.tokens.len() {
        self.index += 1;
        true
      } else {
        false
      }
    }

    fn token(&self) -> &Token {
      // advance 後にのみ呼ばれる前提
      &self.tokens[self.index - 1]
    }

    fn token_mut(&mut self) -> &mut Token {
      // advance 後にのみ呼ばれる前提
      &mut self.tokens[self.index - 1]
    }
  }

  #[test]
  fn tokenize_with_tokenizer_skips_empty_and_deduplicates() {
    let mut schema_builder = Schema::builder();
    let text_field = schema_builder.add_text_field("text", TEXT);
    let _schema = schema_builder.build();

    let mut tokenizer = TestTokenizer;

    let result = tokenize_with_tokenizer(&mut tokenizer, text_field, "ignored");

    // 空文字は含まれず、重複も取り除かれている
    assert_eq!(
      result.query_tokens,
      vec!["rust".to_string(), "search".to_string()]
    );
    assert_eq!(result.terms.len(), 2);
  }
}
