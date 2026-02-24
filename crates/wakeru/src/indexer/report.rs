//! Document Addition Report Type Definition
//!
//! Defines types to aggregate success/skip counts during batch addition.

use serde::{Deserialize, Serialize};

/// Aggregation result of `add_documents`
///
/// Aggregates success/skip counts during batch addition
/// and guarantees that the process completed normally until the end.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AddDocumentsReport {
  /// Total number of documents in input batch
  pub total: usize,
  /// Number of documents actually added to the index
  pub added: usize,
  /// Number of documents skipped due to duplication
  pub skipped_duplicates: usize,
}

impl AddDocumentsReport {
  /// Whether all documents were added (skipped == 0)
  pub fn is_all_added(&self) -> bool {
    self.skipped_duplicates == 0
  }

  /// Record successful addition
  pub fn record_added(&mut self) {
    self.added += 1;
  }

  /// Record skip
  pub fn record_skipped(&mut self) {
    self.skipped_duplicates += 1;
  }

  /// Record total count
  pub fn record_total(&mut self) {
    self.total += 1;
  }
}
