//! APIステート定義

use std::sync::Arc;

use crate::config::Config;
use crate::service::WakeruApiService;

/// アプリケーション状態
///
/// サーバー全体で共有される状態。
/// 設定とサービスを含む。
#[derive(Clone)]
pub struct AppState {
  /// 設定
  pub config: Config,
  /// 形態素解析サービス
  ///
  /// - 本番: `Arc::new(WakeruApiServiceFull::new(&config)?)`
  /// - テスト: `Arc::new(StubWakeruApiService)`
  pub service: Arc<dyn WakeruApiService>,
}

impl AppState {
  /// 新しい AppState を作成する
  #[must_use]
  pub fn new(config: Config, service: Arc<dyn WakeruApiService>) -> Self {
    Self { config, service }
  }
}
