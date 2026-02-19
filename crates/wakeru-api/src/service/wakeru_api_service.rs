//! 形態素解析サービス

use std::time::Instant;

use vibrato_rkyv::Tokenizer as VibratoImpl;
use wakeru::dictionary::DictionaryManager;
use wakeru::tokenizer::should_index;

use crate::config::MAX_TEXT_LENGTH;
use crate::config::{Config, Preset};
use crate::errors::{ApiError, Result};
use crate::models::{TokenDto, WakeruRequest, WakeruResponse};

/// 形態素解析サービスの共通インターフェース
///
/// このトレイトにより、本番実装（`WakeruApiServiceFull`）と
/// テスト用スタブ／モックを差し替え可能にする。
pub trait WakeruApiService: Send + Sync {
  /// 形態素解析を実行する
  ///
  /// # Errors
  /// - 入力エラー（空文字列・長さ超過など）
  /// - 内部エラー
  fn analyze(&self, request: WakeruRequest) -> Result<WakeruResponse>;
}

/// Preset を vibrato-rkyv の PresetDictionaryKind に変換する
///
/// config 層が vibrato に依存しないよう、サービス層で変換を行う
#[must_use]
fn preset_to_vibrato_kind(preset: &Preset) -> vibrato_rkyv::dictionary::PresetDictionaryKind {
  use vibrato_rkyv::dictionary::PresetDictionaryKind;
  match preset {
    Preset::Ipadic => PresetDictionaryKind::Ipadic,
    Preset::UnidicCwj => PresetDictionaryKind::UnidicCwj,
    Preset::UnidicCsj => PresetDictionaryKind::UnidicCsj,
  }
}

/// 形態素解析サービス
///
/// Dictionary と VibratoImpl を直接保持することで、
/// フィルタリング前の全トークンを取得できる。
#[derive(Clone)]
pub struct WakeruApiServiceFull {
  /// vibrato トークナイザー（内部実装）
  inner: VibratoImpl,
}

impl WakeruApiServiceFull {
  /// サービスを初期化する
  ///
  /// # Arguments
  /// * `config` - 設定（辞書プリセットを含む）
  ///
  /// # Errors
  /// 辞書のロードに失敗した場合にエラーを返す
  pub fn new(config: &Config) -> Result<Self> {
    let kind = preset_to_vibrato_kind(&config.preset);

    // 辞書マネージャーを作成して辞書をロード
    let manager = DictionaryManager::with_preset(kind)
      .map_err(|e| ApiError::config(format!("辞書マネージャーの作成に失敗: {}", e)))?;

    let dict =
      manager.load().map_err(|e| ApiError::config(format!("辞書のロードに失敗: {}", e)))?;

    // VibratoImpl を直接作成
    let inner = VibratoImpl::from_shared_dictionary(dict);

    Ok(Self { inner })
  }

  /// 形態素解析を実行する（全トークン返却）
  ///
  /// # Arguments
  /// * `request` - 解析リクエスト
  ///
  /// # Returns
  /// 解析結果（全トークン列と処理時間）
  ///
  /// # Errors
  /// - テキストが空の場合
  /// - テキストが最大長を超える場合
  pub fn analyze(&self, request: WakeruRequest) -> Result<WakeruResponse> {
    // テキスト長の検証
    let text_bytes = request.text.len();
    if text_bytes == 0 {
      return Err(ApiError::invalid_input("テキストが空です"));
    }

    if text_bytes > MAX_TEXT_LENGTH {
      return Err(ApiError::text_too_long(text_bytes, MAX_TEXT_LENGTH));
    }

    // 処理時間計測開始
    let start = Instant::now();

    // ワーカーを作成して解析
    let mut worker = self.inner.new_worker();
    worker.reset_sentence(&request.text);
    worker.tokenize();

    let mut tokens = Vec::with_capacity(worker.num_tokens());

    for token in worker.token_iter() {
      let surface = token.surface();
      let feature = token.feature();
      let start_byte = token.range_byte().start;
      let end_byte = token.range_byte().end;

      // インデックス対象かどうかを判定
      let should_index_flag = should_index(feature);

      let dto = TokenDto::from_feature(surface, feature, start_byte, end_byte, should_index_flag);
      tokens.push(dto);
    }

    // 処理時間計測終了
    let elapsed_ms = start.elapsed().as_millis() as u64;

    Ok(WakeruResponse { tokens, elapsed_ms })
  }
}

/// トレイト `WakeruApiService` の本番実装
impl WakeruApiService for WakeruApiServiceFull {
  fn analyze(&self, request: WakeruRequest) -> Result<WakeruResponse> {
    // 注意: `self.analyze(...)` と書くとトレイトメソッドを再帰呼び出ししてしまうので、
    // 明示的に固有メソッドを呼ぶ。
    WakeruApiServiceFull::analyze(self, request)
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::config::Preset;

  fn create_test_config() -> Config {
    Config {
      bind_addr: "127.0.0.1:5531".to_string(),
      preset: Preset::UnidicCwj,
    }
  }

  #[test]
  fn test_service_creation() {
    let config = create_test_config();

    // 辞書がロードできるか確認
    let service = WakeruApiServiceFull::new(&config)
      .expect("辞書ロードに失敗しました: テスト環境を確認してください");
    let response = service.analyze(WakeruRequest {
      text: "東京".to_string(),
    });
    assert!(response.is_ok());
    let response = response.unwrap();
    assert!(!response.tokens.is_empty());
  }

  #[test]
  fn test_empty_text_error() {
    let config = create_test_config();
    let service = WakeruApiServiceFull::new(&config)
      .expect("辞書ロードに失敗しました: テスト環境を確認してください");
    let result = service.analyze(WakeruRequest {
      text: "".to_string(),
    });
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.code(), "invalid_input");
  }

  #[test]
  fn test_text_too_long_error() {
    let config = create_test_config();
    let service = WakeruApiServiceFull::new(&config)
      .expect("辞書ロードに失敗しました: テスト環境を確認してください");
    let long_text = "a".repeat(MAX_TEXT_LENGTH + 1);
    let result = service.analyze(WakeruRequest { text: long_text });
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.code(), "text_too_long");
  }

  #[test]
  fn test_preset_to_vibrato_kind() {
    use vibrato_rkyv::dictionary::PresetDictionaryKind;

    assert_eq!(
      preset_to_vibrato_kind(&Preset::Ipadic),
      PresetDictionaryKind::Ipadic
    );
    assert_eq!(
      preset_to_vibrato_kind(&Preset::UnidicCwj),
      PresetDictionaryKind::UnidicCwj
    );
    assert_eq!(
      preset_to_vibrato_kind(&Preset::UnidicCsj),
      PresetDictionaryKind::UnidicCsj
    );
  }
}
