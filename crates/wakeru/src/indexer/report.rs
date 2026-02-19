//! ドキュメント追加結果のレポート型定義
//!
//! バッチ追加時の成功・スキップを集計する型を定義します。

use serde::{Deserialize, Serialize};

/// `add_documents` の集計結果
///
/// バッチ追加時の成功・スキップを集計し、
/// 処理の最後まで正常に完了したことを保証する。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AddDocumentsReport {
  /// 入力バッチのドキュメント総数
  pub total: usize,
  /// 実際にインデックスに追加された件数
  pub added: usize,
  /// 重複によりスキップされた件数
  pub skipped_duplicates: usize,
}

impl AddDocumentsReport {
  /// 全て追加されたか（skipped == 0）
  pub fn is_all_added(&self) -> bool {
    self.skipped_duplicates == 0
  }

  /// 追加成功を記録
  pub fn record_added(&mut self) {
    self.added += 1;
  }

  /// スキップを記録
  pub fn record_skipped(&mut self) {
    self.skipped_duplicates += 1;
  }

  /// 合計件数を記録
  pub fn record_total(&mut self) {
    self.total += 1;
  }
}
