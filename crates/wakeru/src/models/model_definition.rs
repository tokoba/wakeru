//! データモデル定義
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::HashMap;

/// metadata 内でタグ情報を保存するための予約キー。
///
/// 検索時のタグフィルタ（`metadata.tags:value`）は、このキーに保存された配列を前提とします。
pub const TAGS_KEY: &str = "tags";

/// 任意のメタデータのキー・バリュー形式マップ
/// qdrant `payload`やpgvector `jsonb` 列と適合するように
/// キー・バリュー形式とする
///
/// key: 文字列
/// Value: Json値
///
pub type Metadata = HashMap<String, JsonValue>;

/// インデックス対象のドキュメント
///
/// RAG パイプラインから投入される「チャンクテキスト + メタデータ」を想定します。
///
/// # タグについて
///
/// タグは `metadata["tags"]` に JSON 配列として保存され、検索時のタグフィルタで利用されます。
/// [`with_tag`](Self::with_tag) / [`with_tags`](Self::with_tags) / [`tags`](Self::tags) メソッドは、
/// この予約キーを扱うための糖衣構文です。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
  /// チャンクID
  pub id: String,

  /// ソースドキュメントID
  pub source_id: String,

  /// チャンクテキスト本文
  pub text: String,

  /// 任意のメタデータ
  #[serde(default)]
  pub metadata: Metadata,
}

/// BM25検索結果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
  /// チャンクID
  pub doc_id: String,

  /// ソースドキュメントID
  pub source_id: String,

  /// BM25 score
  pub score: f32,

  /// チャンクテキスト本文
  pub text: String,

  /// 任意メタデータ
  #[serde(default)]
  pub metadata: Metadata,
}

/// ドキュメントの実装ブロック
impl Document {
  /// ドキュメントのコンストラクタ
  pub fn new(id: impl Into<String>, source_id: impl Into<String>, text: impl Into<String>) -> Self {
    Self {
      id: id.into(),
      source_id: source_id.into(),
      text: text.into(),
      metadata: Metadata::default(),
    }
  }

  /// メタデータを1件追加し Self を返すビルダー
  #[must_use]
  pub fn with_metadata(mut self, key: impl Into<String>, value: JsonValue) -> Self {
    self.metadata.insert(key.into(), value);
    self
  }

  /// 複数のメタデータを一括で追加し Self を返すビルダー
  #[must_use]
  pub fn with_metadata_map(mut self, metadata: Metadata) -> Self {
    self.metadata.extend(metadata);
    self
  }

  // ─── タグ用ヘルパーメソッド ───

  /// タグを 1 件追加するビルダーメソッド。
  ///
  /// # 挙動
  ///
  /// - 内部的には `metadata[TAGS_KEY]`（デフォルトでは `"tags"`）に JSON 配列として格納されます。
  /// - すでに `metadata["tags"]` が存在し、JSON 配列でない場合は配列で上書きします。
  ///
  /// # 目的
  ///
  /// このメソッドは、検索時にタグフィルタ（`metadata.tags:value`）で利用される
  /// `metadata["tags"]` を安全に操作するための糖衣構文です。
  ///
  /// # 例
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
      // すでに "tags" が別型で使われていた場合は上書き
      *entry = JsonValue::Array(vec![JsonValue::String(tag)]);
    }

    self
  }

  /// 複数タグをまとめて追加するビルダーメソッド。
  ///
  /// [`with_tag`](Self::with_tag) を複数回呼び出すのと同等です。
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

  /// メタデータに格納されているタグ一覧を取り出します。
  ///
  /// `metadata[TAGS_KEY]` が JSON 配列の場合のみ、その要素のうち
  /// 文字列のものを `Vec<String>` として返します。
  /// それ以外の場合や未設定の場合は空ベクタを返します。
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
// テストモジュール
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
  use super::*;
  use serde_json::json;

  // ─── Document::new のテスト ───────────────────────────────────────────────

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
    // String で渡す
    let doc1 = Document::new(
      String::from("id1"),
      String::from("src1"),
      String::from("text1"),
    );
    assert_eq!(doc1.id, "id1");

    // &str で渡す
    let doc2 = Document::new("id2", "src2", "text2");
    assert_eq!(doc2.id, "id2");
  }

  // ─── with_metadata / with_metadata_map のテスト ───────────────────────────

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

  // ─── with_tag のテスト ────────────────────────────────────────────────────

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
    // metadata["tags"] を文字列（非配列）で初期化
    let mut doc = Document::new("id", "src", "text");
    doc.metadata.insert(TAGS_KEY.to_string(), json!("not-an-array"));

    // with_tag を呼ぶと配列で上書きされる
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
    // 重複タグは許容される（仕様通り）
    let doc = Document::new("id", "src", "text").with_tag("dup").with_tag("dup");

    let tags = doc.tags();
    assert_eq!(tags, vec!["dup".to_string(), "dup".to_string()]);
  }

  // ─── with_tags のテスト ───────────────────────────────────────────────────

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

  // ─── tags() のエッジケース ─────────────────────────────────────────────────

  #[test]
  fn tags_returns_empty_when_not_set() {
    let doc = Document::new("id", "src", "text");

    // tags キーが存在しない
    assert!(doc.tags().is_empty());
  }

  #[test]
  fn tags_returns_empty_when_value_is_not_array() {
    let mut doc = Document::new("id", "src", "text");
    doc.metadata.insert(TAGS_KEY.to_string(), json!("string-value"));

    // tags が配列でない
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

    // 文字列要素だけが抽出される
    let tags = doc.tags();
    assert_eq!(tags, vec!["valid".to_string(), "also-valid".to_string()]);
  }

  #[test]
  fn tags_returns_empty_for_empty_array() {
    let mut doc = Document::new("id", "src", "text");
    doc.metadata.insert(TAGS_KEY.to_string(), json!([]));

    assert!(doc.tags().is_empty());
  }

  // ─── with_metadata とタグの相互作用 ────────────────────────────────────────

  #[test]
  fn with_metadata_does_not_conflict_with_tags() {
    let doc =
      Document::new("id", "src", "text").with_metadata("author", json!("alice")).with_tag("rust");

    assert_eq!(doc.metadata["author"], json!("alice"));
    assert_eq!(doc.tags(), vec!["rust".to_string()]);
  }

  #[test]
  fn with_metadata_can_overwrite_tags_key() {
    // with_metadata で "tags" を上書きするとタグは壊れる
    let doc = Document::new("id", "src", "text")
      .with_tag("valid-tag")
      .with_metadata(TAGS_KEY, json!("broken"));

    // tags() は空になる（非配列なので）
    assert!(doc.tags().is_empty());
  }

  #[test]
  fn with_tag_restores_broken_tags_after_with_metadata() {
    let doc = Document::new("id", "src", "text")
      .with_tag("first")
      .with_metadata(TAGS_KEY, json!("broken"))
      .with_tag("restored");

    // with_tag が壊れた tags を修復する
    let tags = doc.tags();
    assert_eq!(tags, vec!["restored".to_string()]);
  }

  // ─── TAGS_KEY 定数の確認 ───────────────────────────────────────────────────

  #[test]
  fn tags_key_is_tags() {
    assert_eq!(TAGS_KEY, "tags");
  }

  // ─── Document のシリアライズ/デシリアライズ ─────────────────────────────────

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
    // metadata は #[serde(default)] なので省略可能
    let json_str = r#"{
      "id": "doc-1",
      "source_id": "src-1",
      "text": "sample text"
    }"#;

    let doc: Document = serde_json::from_str(json_str).expect("should deserialize");

    assert!(doc.metadata.is_empty());
  }

  // ─── SearchResult のテスト ────────────────────────────────────────────────

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
    // metadata は #[serde(default)] なので省略可能
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
