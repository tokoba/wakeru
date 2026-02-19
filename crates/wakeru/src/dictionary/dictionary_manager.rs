//! 辞書管理モジュール
//!
//! vibrato-rkyv の辞書データのロード，プリセット辞書のダウンロードを管理する
//! 初回のみ自動的にダウンロードし, 2回目以降はキャッシュディレクトリーから読み込む
//! プリセット辞書には IPADIC, UniDic などがある
//! ローカル辞書を直接ロードすることも可能

use crate::errors::error_definition::DictionaryError;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use vibrato_rkyv::Dictionary;
use vibrato_rkyv::dictionary::LoadMode;
use vibrato_rkyv::dictionary::PresetDictionaryKind;

/// vibrato-rkyv の辞書管理構造体
pub struct DictionaryManager {
  /// 辞書キャッシュディレクトリー
  cache_dir: PathBuf,

  /// プリセット辞書の種別 `Ipadic`, `UnidicCwj`, `UnidicCsj` など
  /// ローカル辞書の場合は `None` を設定すること
  preset_kind: Option<PresetDictionaryKind>,

  /// 辞書ファイルパス(ローカル辞書設定時に必要, プリセット辞書の場合は不要 `None`)
  dictionary_path: Option<PathBuf>,

  /// ロード済みの辞書のキャッシュ(初回ロード時に1回だけ初期化される)
  /// 共有のため Arc で保持する
  /// DictionaryError は Clone を実装しているので Result を保持できる
  dictionary: OnceLock<Result<Arc<Dictionary>, DictionaryError>>,
}

/// 辞書管理構造体の実装ブロック
impl DictionaryManager {
  /// キャッシュディレクトリーのパスを返す
  pub fn cache_dir(&self) -> &Path {
    &self.cache_dir
  }

  /// プリセット辞書を使用する DictionaryManager のコンストラクタ
  pub fn with_preset(preset_kind: PresetDictionaryKind) -> Result<Self, DictionaryError> {
    let cache_dir = default_cache_dir()?;

    Ok(Self {
      cache_dir,
      preset_kind: Some(preset_kind),
      dictionary_path: None,       // プリセット辞書を使用する場合は辞書パス不要
      dictionary: OnceLock::new(), // 新規ロード
    })
  }

  /// ローカルの辞書ファイルを使用する DictionaryManager のコンストラクタ
  pub fn from_local_path<P: AsRef<Path>>(path: P) -> Result<Self, DictionaryError> {
    let path = path.as_ref().to_path_buf();

    if !path.is_file() {
      // ファイルが存在しなければエラー
      let s = path.display().to_string();
      return Err(DictionaryError::DictionaryNotFound(s));
    }

    // カレントディレクトリーの親ディレクトリーをキャッシュディレクトリーとする
    let cache_dir = path.parent().map(Path::to_path_buf).unwrap_or_else(|| PathBuf::from("."));

    Ok(Self {
      cache_dir,
      preset_kind: None,
      dictionary_path: Some(path),
      dictionary: OnceLock::new(),
    })
  }

  /// 辞書ロード
  /// 共有辞書としたいので `Arc<Dictionary>` を返す
  /// - 初回呼び出し時は指定されたパスから辞書ファイルをロードする
  /// - 2回目以降は `Arc<Dictionary>` のクローンを返す
  /// - 初回でエラーが発生した場合、そのエラーをキャッシュし以降返し続ける
  pub fn load(&self) -> Result<Arc<Dictionary>, DictionaryError> {
    self.dictionary.get_or_init(|| self.load_inner().map(Arc::new)).clone()
  }

  /// 辞書ロードの内部実装
  fn load_inner(&self) -> Result<Dictionary, DictionaryError> {
    match (&self.dictionary_path, self.preset_kind) {
      /* 辞書パスとプリセット辞書の種別のタプルでマッチさせる */
      // ローカル辞書指定の場合は，辞書パス指定ありで，プリセット辞書の種別なし
      (Some(path), _) => Self::load_from_local_path(path),

      // プリセット辞書指定の場合は，辞書パス指定なしで，プリセット辞書種別あり
      (None, Some(preset_kind)) => self.load_from_preset(preset_kind),

      // どちらでもない場合は 辞書パス or 辞書種別のエラー
      _ => Err(DictionaryError::InvalidPathOrInvalidPresetKind(
        self.cache_dir.clone(),
        self.preset_kind,
      )),
    }
  }

  /// ローカル辞書ファイルから辞書をロードする
  fn load_from_local_path(path: &Path) -> Result<Dictionary, DictionaryError> {
    Dictionary::from_path(path, LoadMode::TrustCache)
      .map_err(|e| DictionaryError::VibratoLoad(Arc::new(e)))
  }

  /// プリセット辞書設定時のロード処理
  /// 初回は辞書ファイルをダウンロードしてロードする
  /// 2回目以降はキャッシュディレクトリーからロードする
  fn load_from_preset(
    &self,
    preset_kind: PresetDictionaryKind,
  ) -> Result<Dictionary, DictionaryError> {
    // キャッシュディレクトリー作成(初回用)
    // ディレクトリーが存在しなければ新規作成する
    std::fs::create_dir_all(&self.cache_dir)
      .map_err(|e| DictionaryError::CacheDirCreationFailed(Arc::new(e)))?;

    // 辞書名に基づいてサブディレクトリーを作成
    let dict_dir = self.cache_dir.join(preset_kind.name());

    // 初回はダウンロード，2回目以降はキャッシュからロード
    Dictionary::from_preset_with_download(preset_kind, &dict_dir)
      .map_err(|e| DictionaryError::PresetDictDownloadFailed(Arc::new(e)))
  }
}

/// OSに応じたデフォルトのキャッシュディレクトリーパスを返す
///
/// | OS      | パス例                                    |
/// |---------|-------------------------------------------|
/// | Linux   | `~/.cache/wakeru/dict`                    |
/// | macOS   | `~/Library/Caches/wakeru/dict`             |
/// | Windows | `C:\Users\{user}\AppData\Local\wakeru\dict` |
fn default_cache_dir() -> Result<PathBuf, DictionaryError> {
  let base = dirs::cache_dir().ok_or(DictionaryError::CacheDirNotFound)?;

  Ok(base.join("wakeru").join("dict"))
}

/// `DictionaryManager` の手動 `Debug` 実装
///
/// `vibrato_rkyv::Dictionary` が `Debug` trait を実装していないため、
/// `#[derive(Debug)]` が使用できない。代わりにメタ情報のみを表示する。
impl fmt::Debug for DictionaryManager {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("DictionaryManager")
      .field("cache_dir", &self.cache_dir)
      .field("preset_kind", &self.preset_kind)
      .field("dictionary_path", &self.dictionary_path)
      // 中身の Dictionary は vibrato_rkyv で定義されており，
      // Debug trait が実装されていないので、初期化済みフラグだけ出す
      .field("dictionary_initialized", &self.dictionary.get().is_some())
      .finish()
  }
}
