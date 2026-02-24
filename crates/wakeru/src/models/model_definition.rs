//! Data Model Definition
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::HashMap;

/// Reserved key for saving tag information within metadata.
///
/// Tag filters during search (`metadata.tags:value`) assume an array saved under this key.
pub const TAGS_KEY: &str = "tags";

/// Arbitrary key-value map for metadata
/// Uses key-value format to be compatible with qdrant `payload` and pgvector `jsonb` columns
///
/// key: String
/// Value: JsonValue
///
pub type Metadata = HashMap<String, JsonValue>;

/// Document to be indexed
///
/// Assumes "chunk text + metadata" input from RAG pipeline.
///
/// # About Tags
///
/// Tags are saved as a JSON array in `metadata["tags"]` and used for tag filtering during search.
/// The [`with_tag`](Self::with_tag) / [`with_tags`](Self::with_tags) / [`tags`](Self::tags) methods are
/// syntactic sugar for handling this reserved key.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
  /// Chunk ID
  pub id: String,

  /// Source Document ID
  pub source_id: String,

  /// Chunk text body
  pub text: String,

  /// Arbitrary metadata
  #[serde(default)]
  pub metadata: Metadata,
}

/// BM25 Search Result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
  /// Chunk ID
  pub doc_id: String,

  /// Source Document ID
  pub source_id: String,

  /// BM25 score
  pub score: f32,

  /// Chunk text body
  pub text: String,

  /// Arbitrary metadata
  #[serde(default)]
  pub metadata: Metadata,
}

/// Implementation block for Document
impl Document {
  /// Constructor for Document
  pub fn new(id: impl Into<String>, source_id: impl Into<String>, text: impl Into<String>) -> Self {
    Self {
      id: id.into(),
      source_id: source_id.into(),
      text: text.into(),
      metadata: Metadata::default(),
    }
  }

  /// Builder that adds one metadata item and returns Self
  #[must_use]
  pub fn with_metadata(mut self, key: impl Into<String>, value: JsonValue) -> Self {
    self.metadata.insert(key.into(), value);
    self
  }

  /// Builder that adds multiple metadata items at once and returns Self
  #[must_use]
  pub fn with_metadata_map(mut self, metadata: Metadata) -> Self {
    self.metadata.extend(metadata);
    self
  }

  // ─── Helper methods for tags ───

  /// Builder method to add one tag.
  ///
  /// # Behavior
  ///
  /// - Internally stored as a JSON array in `metadata[TAGS_KEY]` (default `"tags"`).
  /// - If `metadata["tags"]` already exists and is not a JSON array, it overwrites it with an array.
  ///
  /// # Purpose
  ///
  /// This method is syntactic sugar to safely manipulate `metadata["tags"]`
  /// which is used in tag filters (`metadata.tags:value`) during search.
  ///
  /// # Examples
  ///
  /// ```ignore
  /// let doc = Document::new("id1", "src1", "text")
  ///     .with_tag("project:foo")
  ///     .with_tag("env:prod");
  /// ```
  #[must_use]
  pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
    let tag = tag.into();
    let entry = self.metadata.entry(TAGS_KEY.to_string()).or_insert(JsonValue::Array(vec![]));

    if let JsonValue::Array(arr) = entry {
      arr.push(JsonValue::String(tag));
    } else {
      // Overwrite if "tags" is already used by another type
      *entry = JsonValue::Array(vec![JsonValue::String(tag)]);
    }

    self
  }

  /// Builder method to add multiple tags at once.
  ///
  /// Equivalent to calling [`with_tag`](Self::with_tag) multiple times.
  #[must_use]
  pub fn with_tags<I, S>(mut self, tags: I) -> Self
  where
    I: IntoIterator<Item = S>,
    S: Into<String>,
  {
    for tag in tags {
      self = self.with_tag(tag);
    }
    self
  }

  /// Extracts the list of tags stored in metadata.
  ///
  /// Returns string elements as `Vec<String>` only if `metadata[TAGS_KEY]` is a JSON array.
  /// Returns an empty vector in other cases or if unset.
  pub fn tags(&self) -> Vec<String> {
    self
      .metadata
      .get(TAGS_KEY)
      .and_then(|v| v.as_array())
      .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
      .unwrap_or_default()
  }
}

// ─────────────────────────────────────────────────────────────────────────────
// Test Module
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
  use super::*;
  use serde_json::json;

  // ─── Test Document::new ───────────────────────────────────────────────

  #[test]
  fn document_new_creates_empty_metadata() {
    let doc = Document::new("doc-1", "src-1", "sample text");

    assert_eq!(doc.id, "doc-1");
    assert_eq!(doc.source_id, "src-1");
    assert_eq!(doc.text, "sample text");
    assert!(doc.metadata.is_empty());
  }

  #[test]
  fn document_new_accepts_string_and_str() {
    // Pass as String
    let doc1 = Document::new(
      String::from("id1"),
      String::from("src1"),
      String::from("text1"),
    );
    assert_eq!(doc1.id, "id1");

    // Pass as &str
    let doc2 = Document::new("id2", "src2", "text2");
    assert_eq!(doc2.id, "id2");
  }

  // ─── Test with_metadata / with_metadata_map ───────────────────────────

  #[test]
  fn with_metadata_adds_single_entry() {
    let doc = Document::new("id", "src", "text").with_metadata("author", json!("alice"));

    assert_eq!(doc.metadata["author"], json!("alice"));
  }

  #[test]
  fn with_metadata_chain_adds_multiple_entries() {
    let doc = Document::new("id", "src", "text")
      .with_metadata("author", json!("alice"))
      .with_metadata("version", json!(1));

    assert_eq!(doc.metadata["author"], json!("alice"));
    assert_eq!(doc.metadata["version"], json!(1));
  }

  #[test]
  fn with_metadata_map_merges_entries() {
    let mut map = Metadata::new();
    map.insert("key1".to_string(), json!("value1"));
    map.insert("key2".to_string(), json!(42));

    let doc = Document::new("id", "src", "text").with_metadata_map(map);

    assert_eq!(doc.metadata["key1"], json!("value1"));
    assert_eq!(doc.metadata["key2"], json!(42));
  }

  #[test]
  fn with_metadata_map_overwrites_existing() {
    let doc = Document::new("id", "src", "text")
      .with_metadata("key", json!("original"))
      .with_metadata_map(HashMap::from([("key".to_string(), json!("overwritten"))]));

    assert_eq!(doc.metadata["key"], json!("overwritten"));
  }

  // ─── Test with_tag ────────────────────────────────────────────────────

  #[test]
  fn with_tag_creates_tags_array_when_missing() {
    let doc = Document::new("id", "src", "text").with_tag("foo");

    let tags = doc.tags();
    assert_eq!(tags, vec!["foo".to_string()]);
  }

  #[test]
  fn with_tag_appends_to_existing_tags() {
    let doc = Document::new("id", "src", "text").with_tag("foo").with_tag("bar");

    let tags = doc.tags();
    assert_eq!(tags, vec!["foo".to_string(), "bar".to_string()]);
  }

  #[test]
  fn with_tag_overwrites_non_array_tags() {
    // Initialize metadata["tags"] with string (non-array)
    let mut doc = Document::new("id", "src", "text");
    doc.metadata.insert(TAGS_KEY.to_string(), json!("not-an-array"));

    // Calling with_tag overwrites it with an array
    let doc = doc.with_tag("fixed");

    let tags = doc.tags();
    assert_eq!(tags, vec!["fixed".to_string()]);
  }

  #[test]
  fn with_tag_overwrites_null_tags() {
    let mut doc = Document::new("id", "src", "text");
    doc.metadata.insert(TAGS_KEY.to_string(), json!(null));

    let doc = doc.with_tag("tag1");

    let tags = doc.tags();
    assert_eq!(tags, vec!["tag1".to_string()]);
  }

  #[test]
  fn with_tag_overwrites_object_tags() {
    let mut doc = Document::new("id", "src", "text");
    doc.metadata.insert(TAGS_KEY.to_string(), json!({"nested": "object"}));

    let doc = doc.with_tag("tag1");

    let tags = doc.tags();
    assert_eq!(tags, vec!["tag1".to_string()]);
  }

  #[test]
  fn with_tag_allows_duplicate_tags() {
    // Duplicate tags are allowed (as per specification)
    let doc = Document::new("id", "src", "text").with_tag("dup").with_tag("dup");

    let tags = doc.tags();
    assert_eq!(tags, vec!["dup".to_string(), "dup".to_string()]);
  }

  // ─── Test with_tags ───────────────────────────────────────────────────

  #[test]
  fn with_tags_adds_multiple_tags() {
    let doc = Document::new("id", "src", "text").with_tags(vec!["a", "b", "c"]);

    let tags = doc.tags();
    assert_eq!(
      tags,
      vec!["a".to_string(), "b".to_string(), "c".to_string()]
    );
  }

  #[test]
  fn with_tags_accepts_empty_iterator() {
    let doc = Document::new("id", "src", "text").with_tags(Vec::<&str>::new());

    let tags = doc.tags();
    assert!(tags.is_empty());
  }

  #[test]
  fn with_tags_can_be_chained_with_with_tag() {
    let doc =
      Document::new("id", "src", "text").with_tag("first").with_tags(vec!["second", "third"]);

    let tags = doc.tags();
    assert_eq!(
      tags,
      vec![
        "first".to_string(),
        "second".to_string(),
        "third".to_string()
      ]
    );
  }

  // ─── Edge cases for tags() ─────────────────────────────────────────────────

  #[test]
  fn tags_returns_empty_when_not_set() {
    let doc = Document::new("id", "src", "text");

    // tags key does not exist
    assert!(doc.tags().is_empty());
  }

  #[test]
  fn tags_returns_empty_when_value_is_not_array() {
    let mut doc = Document::new("id", "src", "text");
    doc.metadata.insert(TAGS_KEY.to_string(), json!("string-value"));

    // tags is not an array
    assert!(doc.tags().is_empty());
  }

  #[test]
  fn tags_returns_empty_when_value_is_null() {
    let mut doc = Document::new("id", "src", "text");
    doc.metadata.insert(TAGS_KEY.to_string(), json!(null));

    assert!(doc.tags().is_empty());
  }

  #[test]
  fn tags_filters_out_non_string_elements() {
    let mut doc = Document::new("id", "src", "text");
    doc.metadata.insert(
      TAGS_KEY.to_string(),
      json!(["valid", 123, true, null, "also-valid"]),
    );

    // Only string elements are extracted
    let tags = doc.tags();
    assert_eq!(tags, vec!["valid".to_string(), "also-valid".to_string()]);
  }

  #[test]
  fn tags_returns_empty_for_empty_array() {
    let mut doc = Document::new("id", "src", "text");
    doc.metadata.insert(TAGS_KEY.to_string(), json!([]));

    assert!(doc.tags().is_empty());
  }

  // ─── Interaction between with_metadata and tags ────────────────────────────────────────

  #[test]
  fn with_metadata_does_not_conflict_with_tags() {
    let doc =
      Document::new("id", "src", "text").with_metadata("author", json!("alice")).with_tag("rust");

    assert_eq!(doc.metadata["author"], json!("alice"));
    assert_eq!(doc.tags(), vec!["rust".to_string()]);
  }

  #[test]
  fn with_metadata_can_overwrite_tags_key() {
    // If "tags" is overwritten by with_metadata, tags are broken
    let doc = Document::new("id", "src", "text")
      .with_tag("valid-tag")
      .with_metadata(TAGS_KEY, json!("broken"));

    // tags() becomes empty (since it is not an array)
    assert!(doc.tags().is_empty());
  }

  #[test]
  fn with_tag_restores_broken_tags_after_with_metadata() {
    let doc = Document::new("id", "src", "text")
      .with_tag("first")
      .with_metadata(TAGS_KEY, json!("broken"))
      .with_tag("restored");

    // with_tag restores broken tags
    let tags = doc.tags();
    assert_eq!(tags, vec!["restored".to_string()]);
  }

  // ─── Check TAGS_KEY constant ───────────────────────────────────────────────────

  #[test]
  fn tags_key_is_tags() {
    assert_eq!(TAGS_KEY, "tags");
  }

  // ─── Document serialization/deserialization ─────────────────────────────────

  #[test]
  fn document_serializes_correctly() {
    let doc = Document::new("doc-1", "src-1", "sample text")
      .with_metadata("author", json!("alice"))
      .with_tag("rust");

    let json_str = serde_json::to_string(&doc).expect("should serialize");

    assert!(json_str.contains("doc-1"));
    assert!(json_str.contains("alice"));
    assert!(json_str.contains("rust"));
  }

  #[test]
  fn document_deserializes_correctly() {
    let json_str = r#"{
      "id": "doc-1",
      "source_id": "src-1",
      "text": "sample text",
      "metadata": {
        "author": "alice",
        "tags": ["rust", "search"]
      }
    }"#;

    let doc: Document = serde_json::from_str(json_str).expect("should deserialize");

    assert_eq!(doc.id, "doc-1");
    assert_eq!(doc.source_id, "src-1");
    assert_eq!(doc.text, "sample text");
    assert_eq!(doc.metadata["author"], json!("alice"));
    assert_eq!(doc.tags(), vec!["rust".to_string(), "search".to_string()]);
  }

  #[test]
  fn document_deserializes_with_missing_metadata() {
    // metadata is #[serde(default)] so it can be omitted
    let json_str = r#"{
      "id": "doc-1",
      "source_id": "src-1",
      "text": "sample text"
    }"#;

    let doc: Document = serde_json::from_str(json_str).expect("should deserialize");

    assert!(doc.metadata.is_empty());
  }

  // ─── Test SearchResult ────────────────────────────────────────────────

  #[test]
  fn search_result_serializes_correctly() {
    let result = SearchResult {
      doc_id: "doc-1".to_string(),
      source_id: "src-1".to_string(),
      score: 0.95,
      text: "result text".to_string(),
      metadata: Metadata::from([("key".to_string(), json!("value"))]),
    };

    let json_str = serde_json::to_string(&result).expect("should serialize");

    assert!(json_str.contains("doc-1"));
    assert!(json_str.contains("0.95"));
    assert!(json_str.contains("result text"));
  }

  #[test]
  fn search_result_deserializes_correctly() {
    let json_str = r#"{
      "doc_id": "doc-1",
      "source_id": "src-1",
      "score": 0.95,
      "text": "result text",
      "metadata": {"key": "value"}
    }"#;

    let result: SearchResult = serde_json::from_str(json_str).expect("should deserialize");

    assert_eq!(result.doc_id, "doc-1");
    assert_eq!(result.source_id, "src-1");
    assert!((result.score - 0.95).abs() < f32::EPSILON);
    assert_eq!(result.text, "result text");
    assert_eq!(result.metadata["key"], json!("value"));
  }

  #[test]
  fn search_result_deserializes_with_missing_metadata() {
    // metadata is #[serde(default)] so it can be omitted
    let json_str = r#"{
      "doc_id": "doc-1",
      "source_id": "src-1",
      "score": 0.95,
      "text": "result text"
    }"#;

    let result: SearchResult = serde_json::from_str(json_str).expect("should deserialize");

    assert!(result.metadata.is_empty());
  }
}
