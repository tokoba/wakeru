//! vibrato を用いた tantivy 用トークナイザー

use std::sync::Arc;
use tantivy::tokenizer::{Token, TokenStream, Tokenizer};
use tracing::debug;
use vibrato_rkyv::Dictionary;
use vibrato_rkyv::Tokenizer as VibratoImpl;

/// Vibrato-rkyv を用いた Tantivy 用 日本語 Tokenizer
///
/// - ステートレス（辞書参照のみ保持）
/// - `Clone + Send + Sync`
/// - Tantivy の `Tokenizer` トレイト実装
#[derive(Clone)]
pub struct VibratoTokenizer {
  inner: VibratoImpl,
}

/// Tantivy の TokenStream トレイトの実装
///
/// - ライフタイムパラメータなし（完全所有型）
/// - `IntoIter` でトークン列を順次消費
/// - `advance` で `token.position += 1` を行う
pub struct VibratoTokenStream {
  /// (表層形, 開始バイト, 終了バイト) のイテレータ
  tokens: std::vec::IntoIter<(String, usize, usize)>,

  /// Tantivy の Token （毎回上書きして再利用）
  token: Token,
}

impl VibratoTokenizer {
  /// 既にロード済みの Dictionary からトークナイザーを構築する
  ///
  /// `vibrato_rkyv::Tokenizer::new(dict)` に対応するコンストラクタ。
  pub fn from_dictionary(dict: Dictionary) -> Self {
    Self {
      inner: VibratoImpl::new(dict),
    }
  }

  /// 共有辞書 (`Arc<Dictionary>`) からトークナイザーを構築する。
  ///
  /// `DictionaryManager::load()`など、辞書を `Arc` で共有している場合はこちらを使う。
  ///
  /// 内部では `vibrato_rkyv::Tokenizer::from_shared_dictionary(dict)` を呼び出す。
  ///
  /// # 使用例
  /// ```rust,no_run
  /// # use wakeru::dictionary::DictionaryManager;
  /// # use wakeru::tokenizer::vibrato_tokenizer::VibratoTokenizer;
  /// # use vibrato_rkyv::dictionary::PresetDictionaryKind;
  /// let manager = DictionaryManager::with_preset(PresetDictionaryKind::Ipadic).unwrap();
  /// let dict = manager.load().unwrap();
  /// let tokenizer = VibratoTokenizer::from_shared_dictionary(dict);
  /// ```
  pub fn from_shared_dictionary(dict: Arc<Dictionary>) -> Self {
    Self {
      inner: VibratoImpl::from_shared_dictionary(dict),
    }
  }
}

impl Tokenizer for VibratoTokenizer {
  // ライフタイムパラメータを持たない所有型ストリームを使用
  type TokenStream<'a> = VibratoTokenStream;

  /// `&mut self`（可変参照）から TokenStream を生成
  fn token_stream<'a>(&'a mut self, input_text: &'a str) -> Self::TokenStream<'a> {
    // worker は解析用と計算領域として lattice を保持する。
    // 都度生成する
    let mut worker = self.inner.new_worker();

    // 文字列をセットし, 通常の tokenizer で解析実行
    worker.reset_sentence(input_text);
    worker.tokenize();

    // 入力テキストのログ出力
    debug!(input_text = %input_text, "形態素解析開始");

    // Vibrato の結果を一旦 Vec にためてから IntoIter に変換
    let mut tokens = Vec::with_capacity(worker.num_tokens());
    // 品詞フィルタリングなどを追加したい場合はこのコードブロックで追加可能
    // 例) 助詞や記号除外してインデックスサイズ削減など
    for token in worker.token_iter() {
      let surface = token.surface();
      let feature = token.feature();
      let indexed = should_index(feature);

      // 各トークンのデバッグログ出力
      debug!(
        surface = %surface,
        ?feature,
        start = token.range_byte().start,
        end = token.range_byte().end,
        indexed,
        "トークン"
      );

      if indexed {
        tokens.push((
          surface.to_string(),
          // 文字列単位ではなくバイト単位でオフセット管理するtantivy仕様に合わせる
          // range_char()は使用禁止
          token.range_byte().start,
          token.range_byte().end,
        ));
      }
    }

    // インデックス対象トークンのログ出力
    debug!(
      input_text = %input_text,
      total_tokens = worker.num_tokens(),
      indexed_tokens = tokens.len(),
      "形態素解析完了"
    );

    VibratoTokenStream {
      tokens: tokens.into_iter(),
      token: Token::default(),
    }
  }
}

/// 品詞フィルタリング
///
/// 助詞・助動詞・記号・フィラー・感動詞・接続詞・接頭詞・連体詞を除外し、
/// 名詞のうち代名詞・非自立を除外する詳細版。
///
/// ## UniDic系辞書での接尾辞対応
/// UniDic系辞書では「金閣寺」が「金閣/寺」に分割され、「寺」は`接尾辞,名詞的`として
/// 解析される。地名に付く「寺」「駅」「温泉」などは意味のある内容語として
/// 扱いたいため、`接尾辞,名詞的`はインデックス対象に含める。
pub fn should_index(feature: &str) -> bool {
  // ─── 最優先: 除外すべき品詞 ───
  // 助詞・助動詞・記号・フィラー・感動詞・接続詞・接頭詞・連体詞
  if feature.starts_with("助詞")
    || feature.starts_with("助動詞")
    || feature.starts_with("記号")
    || feature.starts_with("フィラー")
    || feature.starts_with("感動詞")
    || feature.starts_with("接続詞")
    || feature.starts_with("接頭詞")
    || feature.starts_with("連体詞")
  {
    return false;
  }

  // ─── UniDic系: 接尾辞,名詞的 を名詞相当として扱う ───
  // 例: "接尾辞,名詞的,一般,*,*,*,寺,テラ,寺,テラ,*,*,*,*,*,*"
  // 地名に付く「寺」「駅」「温泉」などは意味のある内容語として扱う
  if feature.starts_with("接尾辞,名詞的") {
    return true;
  }

  // ─── 名詞の細分類チェック ───
  if feature.starts_with("名詞") {
    // 除外: 代名詞・非自立
    if feature.starts_with("名詞,代名詞") || feature.starts_with("名詞,非自立") {
      return false;
    }
    // それ以外の名詞は含める
    return true;
  }

  // ─── 動詞・形容詞は全て含める ───
  if feature.starts_with("動詞") || feature.starts_with("形容詞") {
    return true;
  }

  // ─── 形状詞（UniDic）は内容語として含める ───
  // 「きれいだ」「静かだ」など形容動詞的な語
  if feature.starts_with("形状詞") {
    return true;
  }

  // ─── 副詞は一般のみ含める ───
  if feature.starts_with("副詞") {
    return feature.starts_with("副詞,一般");
  }

  // ─── 上記以外は除外 ───
  false
}

impl TokenStream for VibratoTokenStream {
  /// 次のトークンへ進める。
  ///
  /// - `tokens` の `IntoIter` から 1 件 `next()` して `self.token` に上書き
  /// - 位置は `self.token.position += 1` でインクリメント
  fn advance(&mut self) -> bool {
    if let Some((surface, start, end)) = self.tokens.next() {
      // Token の内容を更新（String はムーブで再利用）
      self.token.text = surface;
      self.token.offset_from = start;
      self.token.offset_to = end;

      // Tantivy の Token::default() は position = usize::MAX で初期化されるため、
      // 通常の += 1 ではオーバーフローパニックが発生する。
      // wrapping_add(1) を使うことで usize::MAX + 1 = 0 となり、正しく0からカウント開始できる。
      self.token.position = self.token.position.wrapping_add(1);
      // 単語単位なので 1 固定
      self.token.position_length = 1;

      true
    } else {
      false
    }
  }

  fn token(&self) -> &Token {
    &self.token
  }

  fn token_mut(&mut self) -> &mut Token {
    &mut self.token
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  /// 内容語（名詞,一般）がインデックス対象になることを確認
  #[test]
  fn index_common_noun() {
    assert!(should_index("名詞,一般,*,*,*,*,東京,トウキョウ,トーキョー"));
  }

  /// 固有名詞がインデックス対象になることを確認
  #[test]
  fn index_proper_noun() {
    assert!(should_index(
      "名詞,固有名詞,地域,一般,*,*,東京,トウキョウ,トーキョー"
    ));
  }

  /// サ変接続名詞がインデックス対象になることを確認
  #[test]
  fn index_sahen_noun() {
    assert!(should_index("名詞,サ変接続,*,*,*,*,検索,ケンサク,ケンサク"));
  }

  /// 動詞がインデックス対象になることを確認
  #[test]
  fn index_verb() {
    assert!(should_index("動詞,自立,*,*,一段,連用形,食べる,タベ,タベ"));
  }

  /// 形容詞がインデックス対象になることを確認
  #[test]
  fn index_adjective() {
    assert!(should_index(
      "形容詞,自立,*,*,形容詞・アウオ段,基本形,高い,タカイ,タカイ"
    ));
  }

  /// 助詞が除外されることを確認
  #[test]
  fn exclude_particle() {
    assert!(!should_index("助詞,格助詞,一般,*,*,*,が,ガ,ガ"));
  }

  /// 記号が除外されることを確認
  #[test]
  fn exclude_symbol() {
    assert!(!should_index("記号,句点,*,*,*,*,。,。,。"));
  }

  /// 名詞,代名詞が除外されることを確認
  #[test]
  fn exclude_pronoun() {
    assert!(!should_index("名詞,代名詞,一般,*,*,*,これ,コレ,コレ"));
  }

  /// 名詞,非自立が除外されることを確認
  #[test]
  fn exclude_dependent_noun() {
    assert!(!should_index("名詞,非自立,一般,*,*,*,こと,コト,コト"));
  }

  /// 接続詞が除外されることを確認
  #[test]
  fn exclude_conjunction() {
    assert!(!should_index("接続詞,*,*,*,*,*,しかし,シカシ,シカシ"));
  }

  /// 助動詞が除外されることを確認
  #[test]
  fn exclude_auxiliary_verb() {
    assert!(!should_index(
      "助動詞,*,*,*,特殊・デス,基本形,です,デス,デス"
    ));
  }

  /// フィラーが除外されることを確認
  #[test]
  fn exclude_filler() {
    assert!(!should_index("フィラー,*,*,*,*,*,えー,エー,エー"));
  }

  /// 感動詞が除外されることを確認
  #[test]
  fn exclude_interjection() {
    assert!(!should_index("感動詞,*,*,*,*,*,はい,ハイ,ハイ"));
  }

  /// UniDic系の接尾辞,名詞的がインデックス対象になることを確認
  /// 「金閣寺」が「金閣/寺」に分割された場合の「寺」を想定
  #[test]
  fn index_suffix_noun_for_unidic() {
    assert!(should_index(
      "接尾辞,名詞的,一般,*,*,*,寺,テラ,寺,テラ,*,*,*,*,*,*"
    ));
  }

  /// UniDic系の接尾辞,名詞的（地名系）がインデックス対象になることを確認
  #[test]
  fn index_suffix_noun_place_names() {
    // 駅
    assert!(should_index(
      "接尾辞,名詞的,一般,*,*,*,駅,エキ,駅,エキ,*,*,*,*,*,*"
    ));
    // 温泉
    assert!(should_index(
      "接尾辞,名詞的,一般,*,*,*,温泉,オンセン,温泉,オンセン,*,*,*,*,*,*"
    ));
  }

  /// 他の種類の接尾辞（動詞的など）は除外されることを確認
  #[test]
  fn exclude_other_suffix_types() {
    // 接尾辞,動詞的 は除外
    assert!(!should_index("接尾辞,動詞的,*,*,*,*,れる,レル,れる,レル"));
    // 接尾辞,形容詞的 は除外
    assert!(!should_index("接尾辞,形容詞的,*,*,*,*,しい,シイ,しい,シイ"));
  }

  /// 形状詞（UniDic）がインデックス対象になることを確認
  /// 「きれいだ」「静かだ」など形容動詞的な語
  #[test]
  fn index_adjectival_noun() {
    assert!(should_index(
      "形状詞,一般,*,*,*,*,きれい,キレイ,キレイ,きれい,キレイ,1,C2,*"
    ));
    assert!(should_index(
      "形状詞,一般,*,*,*,*,静か,シズカ,シズカ,静か,シズカ,0,C2,*"
    ));
  }

  /// UniDicの補助記号（句点・読点）が除外されることを確認
  /// `feature.starts_with("記号")`ではマッチしないが、allow-list方式で除外される
  #[test]
  fn exclude_unidic_punctuation() {
    // 句点
    assert!(!should_index(
      "補助記号,句点,*,*,*,*,*,。,。,*,。,*,記号,*,*,*,*,*,*,補助,*,*,*,*,*,*,*,6880571302400,25"
    ));
    // 読点
    assert!(!should_index(
      "補助記号,読点,*,*,*,*,*,、,、,*,、,*,記号,*,*,*,*,*,*,補助,*,*,*,*,*,*,*,6605693395456,24"
    ));
  }
}
