//! dictionary 用のテスト
//! 辞書管理の統合テスト

use vibrato_rkyv::dictionary::PresetDictionaryKind;
use wakeru::dictionary::DictionaryManager;
use wakeru::errors::DictionaryError;

/// DictionaryManager のコンストラクタが正常に動作することを確認する。
#[test]
fn create_dictionary_manager_with_preset() {
  let result = DictionaryManager::with_preset(PresetDictionaryKind::Ipadic);

  // コンストラクタ自体はネットワーク不要なので成功するはず
  assert!(
    result.is_ok(),
    "DictionaryManager の構築に失敗: {:?}",
    result.err()
  );
}

/// 存在しないパスを指定した場合にエラーが返ることを確認する。
#[test]
fn from_local_path_with_nonexistent_file() {
  let result = DictionaryManager::from_local_path("/nonexistent/path/to/system.dic");

  assert!(result.is_err());
  let err = result.unwrap_err();
  // DictionaryError::DictionaryNotFound であることを確認
  assert!(
    matches!(err, DictionaryError::DictionaryNotFound(_)),
    "期待されるエラー型ではありません: {:?}",
    err
  );
}

/// プリセット辞書のダウンロード＆ロード テスト。
///
/// ネットワークアクセスと大容量ファイルの処理が必要なため
/// `#[ignore]` を付けている。
///
/// 実行方法:
/// ```bash
/// cargo test -- --ignored download_and_load_ipadic
/// ```
#[test]
#[ignore = "辞書ダウンロードは時間がかかるため通常テストから除外"]
fn download_and_load_ipadic() {
  let manager = DictionaryManager::with_preset(PresetDictionaryKind::Ipadic)
    .expect("DictionaryManager の構築に失敗");

  // 辞書をロード（初回はダウンロドが発生する）
  let dict = manager.load();
  assert!(dict.is_ok(), "辞書のロードに失敗: {:?}", dict.err());

  // 2回目のロードはキャッシュから取得される
  let dict2 = manager.load();
  assert!(dict2.is_ok(), "2回目のロードに失敗");
}

/// キャッシュ済み辞書のロードテスト。
///
/// 事前に `download_and_load_ipadic` を実行して
/// 辞書がキャッシュされている場合にのみ有効。
/// キャッシュが存在しない場合は自動スキップする。
#[test]
fn load_cached_dictionary() {
  let manager = DictionaryManager::with_preset(PresetDictionaryKind::Ipadic)
    .expect("DictionaryManager の構築に失敗");

  // キャッシュが存在するかチェック
  let cache_dir = manager.cache_dir();
  let dict_subdir = cache_dir.join(PresetDictionaryKind::Ipadic.name());

  if !dict_subdir.exists() {
    eprintln!(
      "辞書キャッシュが存在しないためスキップ: {}",
      dict_subdir.display()
    );
    return;
  }

  // キャッシュからロード
  let dict = manager.load();
  assert!(
    dict.is_ok(),
    "キャッシュ辞書のロードに失敗: {:?}",
    dict.err()
  );
}

/// ロードした辞書で基本的な形態素解析ができることを確認する。
///
/// 事前に辞書キャッシュが必要。
#[test]
fn basic_tokenize_with_cached_dictionary() {
  let manager = DictionaryManager::with_preset(PresetDictionaryKind::Ipadic)
    .expect("DictionaryManager の構築に失敗");

  let cache_dir = manager.cache_dir();
  let dict_subdir = cache_dir.join(PresetDictionaryKind::Ipadic.name());

  if !dict_subdir.exists() {
    eprintln!("辞書キャッシュが存在しないためスキップ");
    return;
  }

  // 辞書ロード
  let dict = manager.load().expect("辞書のロードに失敗");

  // Tokenizer と Worker を生成して形態素解析
  // from_shared_dictionary を使用して Arc<Dictionary> を直接渡す
  let tokenizer = vibrato_rkyv::Tokenizer::from_shared_dictionary(dict);
  let mut worker = tokenizer.new_worker();

  worker.reset_sentence("東京は日本の首都です");
  worker.tokenize();

  // トークン数がゼロでないことを確認
  assert!(worker.num_tokens() > 0, "形態素解析結果が空です");

  // 各トークンの表層形と品詞情報を出力（デバッグ用）
  for token in worker.token_iter() {
    println!(
      "  surface: {:8} | range_byte: {:?} | feature: {}",
      token.surface(),
      token.range_byte(),
      token.feature()
    );
  }
}
