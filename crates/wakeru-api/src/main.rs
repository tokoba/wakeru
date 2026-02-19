//! wakeru-api サーバーエントリーポイント

use std::sync::Arc;

use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use wakeru_api::ApiError;
use wakeru_api::api::AppState;
use wakeru_api::api::run_server;
use wakeru_api::config::Config;
use wakeru_api::service::WakeruApiServiceFull;

#[tokio::main]
async fn main() -> Result<(), ApiError> {
  // ロギングの初期化
  tracing_subscriber::registry().with(tracing_subscriber::fmt::layer()).init();

  // 設定の読み込み
  let config = Config::from_env()?;
  tracing::info!(preset = ?config.preset, "設定を読み込みました");

  // サービスの初期化
  let service = Arc::new(WakeruApiServiceFull::new(&config)?);
  tracing::info!("形態素解析サービスを初期化しました");

  // アプリケーション状態の作成
  let state = AppState::new(config, service);

  // サーバー起動
  run_server(state).await
}
