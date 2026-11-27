//! コマンドライン・トレイ／メニューバー・Setup版メッセージボックスなど、管理画面（`web::i18n`）
//! の外で人が読む文言を、実行時に9言語へ切り替える。
//!
//! 言語の決め方（優先順）:
//! 1. 環境変数 `SKILVELLA_LANGUAGE`（明示の上書き）
//! 2. `LC_ALL` → `LC_MESSAGES` → `LANG`（空・`C`・`POSIX` は未設定として次へ進む）
//! 3. OSの表示言語（Windowsは `GetUserDefaultUILanguage`、macOSは `AppleLanguages` の先頭）
//! 4. 判定できない・未対応の言語なら英語
//!
//! 管理画面の判定（Cookie → Accept-Language）はブラウザーごとの別経路で、ここでは扱わない。
//!
//! 辞書は `src/locales/<タグ>.json`（キーは管理画面と同じく日本語の原文、日本語は原文そのもの）。
//! 辞書にないキーは原文（日本語）をそのまま返し、落ちない。プレースホルダーは `{name}` 形式。
//! キーと辞書の整合は `tests/localization.rs` が検証する。AIが読む文言（MCPなど）は対象外で英語に統一する。

use std::{collections::BTreeMap, sync::OnceLock};

/// 対応する言語タグ。先頭（日本語）は原文キーそのものを表示し、辞書を持たない。
pub const LANGUAGES: [&str; 9] = [
    "ja", "en", "zh-CN", "zh-TW", "ko", "es", "fr", "de", "pt-BR",
];
const ENGLISH: usize = 1;
// LANGUAGES[1..] と同じ並び。
const CATALOGS: [&str; 8] = [
    include_str!("locales/en.json"),
    include_str!("locales/zh-CN.json"),
    include_str!("locales/zh-TW.json"),
    include_str!("locales/ko.json"),
    include_str!("locales/es.json"),
    include_str!("locales/fr.json"),
    include_str!("locales/de.json"),
    include_str!("locales/pt-BR.json"),
];
type Catalog = BTreeMap<String, String>;
static TRANSLATIONS: [OnceLock<Catalog>; CATALOGS.len()] =
    [const { OnceLock::new() }; CATALOGS.len()];
// 判定にはmacOSで子プロセスを使うため、1プロセスで1回だけ行い結果を使い回す。
static CURRENT: OnceLock<usize> = OnceLock::new();

/// 現在の環境の言語タグ（`LANGUAGES` のいずれか）。
pub fn current() -> &'static str {
    LANGUAGES[index()]
}

fn index() -> usize {
    *CURRENT.get_or_init(|| detect(|name| std::env::var(name).ok(), os_language))
}

/// 環境変数の参照とOS言語の取得を差し替えられる判定本体。
fn detect(env: impl Fn(&str) -> Option<String>, os: impl FnOnce() -> Option<usize>) -> usize {
    for name in ["SKILVELLA_LANGUAGE", "LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Some(index) = env(name).as_deref().and_then(parse) {
            return index;
        }
    }
    os().unwrap_or(ENGLISH)
}

/// `ja`、`ja_JP.UTF-8`、`zh-Hant-TW`、`pt_PT`、`Japanese_Japan.932` などを言語の添字へ変換する。
///
/// 空文字・`C`・`POSIX`（`C.UTF-8` を含む）は判定材料にならないため `None`。
/// 対応外の言語は英語の添字を返す（判定を打ち切って英語にする）。
pub fn parse(value: &str) -> Option<usize> {
    let value = value.trim();
    let base = value
        .split(['.', '@'])
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    if base.is_empty() || base == "c" || base == "posix" {
        return None;
    }
    let mut parts = base.split(['_', '-']);
    let tag = match parts.next().unwrap_or_default() {
        "japanese" => "ja",
        // 繁体字はスクリプト（Hant）または地域（TW・HK・MO）で判定する。それ以外の zh は簡体字。
        "zh" if parts.any(|part| matches!(part, "tw" | "hk" | "mo" | "hant")) => "zh-TW",
        "zh" => "zh-CN",
        // pt-PT など pt-BR 以外の pt-* も pt-BR にまとめる。
        "pt" => "pt-BR",
        primary => primary,
    };
    // 大文字小文字を無視して照合し、未対応なら英語にする。
    Some(
        LANGUAGES
            .iter()
            .position(|language| language.eq_ignore_ascii_case(tag))
            .unwrap_or(ENGLISH),
    )
}

/// Windowsの `LANGID`（下位10bitが主言語、上位6bitが副言語）を言語の添字へ変換する。
#[cfg_attr(not(windows), allow(dead_code))]
fn from_langid(id: u16) -> usize {
    let tag = match id & 0x3ff {
        0x11 => "ja",
        0x09 => "en",
        // 中国語は副言語で分ける: 台湾(0x0404)・香港(0x0c04)・マカオ(0x1404)は繁体字、それ以外は簡体字。
        0x04 if matches!(id, 0x0404 | 0x0c04 | 0x1404) => "zh-TW",
        0x04 => "zh-CN",
        0x12 => "ko",
        0x0a => "es",
        0x0c => "fr",
        0x07 => "de",
        0x16 => "pt-BR",
        _ => "en",
    };
    LANGUAGES
        .iter()
        .position(|language| *language == tag)
        .unwrap_or(ENGLISH)
}

#[cfg(windows)]
fn os_language() -> Option<usize> {
    // windows-sys の機能を増やさないよう、kernel32 の1関数だけを直接宣言する。
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetUserDefaultUILanguage() -> u16;
    }
    // 安全性: 引数なしでLANGIDを返すだけの関数。
    Some(from_langid(unsafe { GetUserDefaultUILanguage() }))
}

#[cfg(target_os = "macos")]
fn os_language() -> Option<usize> {
    // GUIから起動された場合はLANGが無いため、OSの言語順の先頭を読む。
    // 出力は `(\n    "ja-JP",\n    "en-JP"\n)` の形式。
    let output = std::process::Command::new("/usr/bin/defaults")
        .args(["read", "-g", "AppleLanguages"])
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()
        .filter(|output| output.status.success())?;
    let text = String::from_utf8_lossy(&output.stdout);
    let first = text
        .split(['(', ')', ',', '\n'])
        .map(|part| part.trim().trim_matches('"'))
        .find(|part| !part.is_empty())?;
    parse(first)
}

#[cfg(not(any(windows, target_os = "macos")))]
fn os_language() -> Option<usize> {
    None
}

/// 現在の言語での文言を返す。日本語は原文（キー）そのもの、辞書に無ければ原文を返す。
pub fn text(key: &str) -> &str {
    lookup(index(), key)
}

fn lookup(index: usize, key: &str) -> &str {
    if index == 0 {
        return key;
    }
    let slot = index - 1;
    TRANSLATIONS[slot]
        .get_or_init(|| serde_json::from_str(CATALOGS[slot]).expect("同梱の翻訳辞書は有効なJSON"))
        .get(key)
        .map_or(key, String::as_str)
}

/// `{name}` を `values` の値へ置き換える。値の中の `{…}` は再展開しない。
pub fn format(key: &str, values: &[(&str, &str)]) -> String {
    expand(text(key), values)
}

fn expand(mut rest: &str, values: &[(&str, &str)]) -> String {
    let mut result = String::with_capacity(rest.len());
    while let Some((prefix, token)) = rest.split_once('{') {
        let Some((name, suffix)) = token.split_once('}') else {
            break;
        };
        result.push_str(prefix);
        if let Some((_, value)) = values.iter().find(|(field, _)| *field == name) {
            result.push_str(value);
        } else {
            result.push('{');
            result.push_str(name);
            result.push('}');
        }
        rest = suffix;
    }
    result.push_str(rest);
    result
}
