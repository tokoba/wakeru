//! Query Tokenization Module
//!
//! Provides tokenization processing for search queries.

use std::collections::HashSet;

use tantivy::Term;
use tantivy::schema::Field;
use tantivy::tokenizer::{TextAnalyzer, TokenStream, Tokenizer};

/// Tokenization result of query string
///
/// # Fields
/// - `terms`: Term vector for Tantivy search (for OR query construction)
/// - `query_tokens`: Token string vector for log output
#[derive(Debug, Clone)]
pub(crate) struct TokenizationResult {
  /// Vector of unique Terms (for query construction)
  pub(crate) terms: Vec<Term>,
  /// Vector of unique token strings (for log output)
  pub(crate) query_tokens: Vec<String>,
}

/// Pure function to tokenize query string using the given Tokenizer (Generic version)
///
/// # Process
/// - Skip empty string tokens
/// - Exclude duplicate tokens (only first occurrence is adopted, using HashSet)
/// - Convert to Term
///
/// # Generics
/// Since `Tokenizer` trait in tantivy 0.25.0 requires `Self: Sized`,
/// `&dyn Tokenizer` cannot be used. Use generics `<T: Tokenizer>` instead.
///
/// # Arguments
/// - `tokenizer`: Tokenizer (`T: Tokenizer` generics)
/// - `field`: Field to create Term for
/// - `query_str`: Query string to tokenize
///
/// # Returns
/// `TokenizationResult` containing unique Terms and token strings
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

/// Tokenization function for TextAnalyzer
///
/// In tantivy 0.25.0, `TextAnalyzer` does not implement `Tokenizer` trait,
/// so a dedicated function is provided.
///
/// # Arguments
/// - `analyzer`: TextAnalyzer (obtained from tantivy)
/// - `field`: Field to create Term for
/// - `query_str`: Query string to tokenize
///
/// # Returns
/// `TokenizationResult` containing unique Terms and token strings
pub(crate) fn tokenize_with_text_analyzer(
  analyzer: &mut TextAnalyzer,
  field: Field,
  query_str: &str,
) -> TokenizationResult {
  let mut token_stream = analyzer.token_stream(query_str);
  tokenize_from_stream(&mut token_stream, field)
}

/// Common process to extract Terms from token stream
fn tokenize_from_stream<T: TokenStream + ?Sized>(
  token_stream: &mut T,
  field: Field,
) -> TokenizationResult {
  let mut terms = Vec::new();
  let mut seen = HashSet::new();
  let mut query_tokens = Vec::new();

  while token_stream.advance() {
    let token = token_stream.token();

    // Skip empty tokens
    if token.text.is_empty() {
      continue;
    }

    // Skip duplicate tokens (adopt only first occurrence)
    if !seen.insert(token.text.clone()) {
      continue;
    }

    query_tokens.push(token.text.clone());

    // Convert to Tantivy Term
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

  /// Test that duplicate tokens are excluded
  #[test]
  fn tokenize_with_tokenizer_deduplicates_tokens() {
    // Prepare Field
    let mut schema_builder = Schema::builder();
    let text_field = schema_builder.add_text_field("text", TEXT);
    let _schema = schema_builder.build();

    let mut tokenizer = SimpleTokenizer::default();

    let input = "rust rust search rust";
    let result = tokenize_with_tokenizer(&mut tokenizer, text_field, input);

    // Duplicates are removed while keeping the first occurrence order
    assert_eq!(
      result.query_tokens,
      vec!["rust".to_string(), "search".to_string()]
    );
    assert_eq!(result.terms.len(), result.query_tokens.len());
  }

  /// Test that empty tokens are skipped and duplicates are excluded
  ///
  /// Since SimpleTokenizer does not generate empty tokens,
  /// implement Tokenizer for testing manually.
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
      // Assumed to be called only after advance
      &self.tokens[self.index - 1]
    }

    fn token_mut(&mut self) -> &mut Token {
      // Assumed to be called only after advance
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

    // Empty strings are not included, and duplicates are removed
    assert_eq!(
      result.query_tokens,
      vec!["rust".to_string(), "search".to_string()]
    );
    assert_eq!(result.terms.len(), 2);
  }
}
