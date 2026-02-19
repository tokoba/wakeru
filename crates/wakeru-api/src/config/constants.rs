//! API設定の定数定義

/// 入力テキストの最大長（バイト単位）
///
/// 10MB までのテキストを許可する。
/// 大きなテキストの処理によるリソース枯渇を防ぐための制限。
pub const MAX_TEXT_LENGTH: usize = 10_000_000;

/// デフォルトのバインドアドレス
///
/// 開発環境での利用を想定した localhost の標準ポート。
pub const DEFAULT_BIND_ADDR: &str = "127.0.0.1:5530";

/// デフォルトの辞書プリセット名
///
/// UniDic (CWJ) をデフォルトとして使用。
/// 現代日本語書き言葉コーパスに基づく辞書。
pub const DEFAULT_PRESET_DICT: &str = "unidic-cwj";
