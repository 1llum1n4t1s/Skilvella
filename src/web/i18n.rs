use axum::{
    extract::Request,
    http::{HeaderMap, Method, header},
    middleware::Next,
    response::Response,
};
use std::{collections::BTreeMap, sync::OnceLock};

const LANGUAGES: [(&str, &str); 9] = [
    ("ja", "日本語"),
    ("en", "English"),
    ("zh-CN", "简体中文"),
    ("zh-TW", "繁體中文"),
    ("ko", "한국어"),
    ("es", "Español"),
    ("fr", "Français"),
    ("de", "Deutsch"),
    ("pt-BR", "Português (Brasil)"),
];
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
static CLIENT_MESSAGES: [OnceLock<String>; LANGUAGES.len()] =
    [const { OnceLock::new() }; LANGUAGES.len()];

// ブラウザーで発生する操作の文言だけを渡す。動的な値は textContent へ出力する。
const CLIENT_KEYS: &[(&str, &str)] = &[
    (
        "policy-unconfirmed",
        "保存状態を確認できません。再読み込みしてください。",
    ),
    (
        "policy-recovered",
        "保存結果を確認できませんでした。最新の設定を表示しています。",
    ),
    (
        "policy-sync-failed",
        "保存しましたが、同期先への同期に失敗しました。設定画面を確認してください。",
    ),
    ("policy-saving", "保存中…"),
    (
        "policy-disabled",
        "保存しました。同期先が選ばれていないため入口には反映されません。",
    ),
    ("policy-saved", "保存しました。"),
    ("publish-failed", "公開状態を変更できませんでした。"),
    ("dirty", "未保存の変更"),
    ("confirm-title", "操作の確認"),
    ("cancel", "キャンセル"),
    ("continue", "続行する"),
    (
        "discard-navigation",
        "未保存の本文があります。変更を破棄してページを移動しますか？",
    ),
    (
        "discard-operation",
        "未保存の本文があります。変更を破棄して操作を続けますか？",
    ),
    ("eligible", "対象:"),
    ("selected", "{count}件選択"),
    (
        "repositories-selected",
        "{groups}取得元選択 · 対象{count}件",
    ),
    (
        "preview-failed",
        "プレビューを表示できませんでした。本文は編集欄に保持されています。",
    ),
    (
        "folder-dialog-opening",
        "フォルダー選択ダイアログを開いています…",
    ),
    (
        "folder-dialog-unavailable",
        "フォルダー選択ダイアログを開けませんでした。フォルダーの場所を入力してください。",
    ),
    (
        "folder-dialog-busy",
        "別のフォルダー選択ダイアログが開いています。先に閉じてください。",
    ),
    ("update-checking", "更新を確認中…"),
    ("update-available", "バージョン {version} を利用できます。"),
    ("update-current", "最新バージョンです。"),
    (
        "update-unsupported",
        "この環境では自動更新を利用できません。リリースページを確認してください。",
    ),
    (
        "update-check-failed",
        "更新の確認に失敗しました。接続を確認して再試行してください。",
    ),
    (
        "update-confirm",
        "更新のインストールを開始しますか？ アプリを再起動します。",
    ),
    ("update-downloading", "更新をダウンロードして検証中…"),
    (
        "update-restarting",
        "更新を開始しました。アプリの再起動をお待ちください。",
    ),
    (
        "update-reload-manual",
        "新しいバージョンの起動を確認できませんでした。しばらく待ってからページを再読み込みしてください。",
    ),
    (
        "update-install-failed",
        "更新をインストールできませんでした。再度更新を確認してください。",
    ),
];

#[derive(Clone, Copy)]
pub(super) struct Locale {
    index: usize,
    preference: Option<&'static str>,
    port: u16,
    is_get: bool,
}

impl Locale {
    fn from_request(headers: &HeaderMap, method: &Method, port: u16) -> Self {
        // Cookie はポートで分離されないため、接続を受けたポートを名前に含める。
        let cookie_name = cookie_name(port);
        let preference = headers
            .get_all(header::COOKIE)
            .iter()
            .filter_map(|value| value.to_str().ok())
            .flat_map(|value| value.split(';'))
            .filter_map(|cookie| cookie.trim().split_once('='))
            .filter(|(name, _)| *name == cookie_name)
            .find_map(|(_, value)| Self::valid_preference(value));
        let index = preference
            .and_then(language_index)
            .unwrap_or_else(|| browser_language(headers));
        Self {
            index,
            preference,
            port,
            is_get: method == Method::GET,
        }
    }

    pub(super) fn valid_preference(value: &str) -> Option<&'static str> {
        if value == "auto" {
            Some("auto")
        } else {
            language_index(value).map(|index| LANGUAGES[index].0)
        }
    }

    pub(super) fn preference(&self) -> &str {
        self.preference.unwrap_or("auto")
    }

    pub(super) fn language(&self) -> &'static str {
        LANGUAGES[self.index].0
    }

    pub(super) fn languages(&self) -> &'static [(&'static str, &'static str)] {
        &LANGUAGES
    }

    pub(super) fn migration_cookie(&self) -> Option<String> {
        // POSTの再送や未保存の下書きの再読込みを避けるため、移行はGET画面に限る。
        (self.is_get && self.preference.is_none()).then(|| cookie_name(self.port))
    }

    pub(super) fn set_cookie(&self, value: &'static str) -> String {
        format!(
            "{}={value}; Max-Age=31536000; Path=/; SameSite=Lax",
            cookie_name(self.port)
        )
    }

    pub(super) fn text<'a>(&self, key: &'a str) -> &'a str {
        if self.index == 0 {
            return key;
        }
        let index = self.index - 1;
        TRANSLATIONS[index]
            .get_or_init(|| {
                serde_json::from_str(CATALOGS[index]).expect("同梱の翻訳辞書は有効なJSON")
            })
            .get(key)
            .map_or(key, String::as_str)
    }

    pub(super) fn format(&self, key: &str, values: &[(&str, &str)]) -> String {
        // 利用者の入力値にプレースホルダーがあっても、再展開しない。
        let mut rest = self.text(key);
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

    pub(super) fn client_messages(&self) -> &str {
        CLIENT_MESSAGES[self.index].get_or_init(|| {
            let messages: BTreeMap<_, _> = CLIENT_KEYS
                .iter()
                .map(|(id, key)| (*id, self.text(key)))
                .collect();
            serde_json::to_string(&messages).expect("文字列だけの文言表はJSONに変換できる")
        })
    }
}

pub(super) async fn localize(mut request: Request, next: Next) -> Response {
    // 通常の入口では local_boundary が接続先と Host の一致を先に確認する。
    let port = request
        .headers()
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())
        .and_then(|host| host.rsplit_once(':'))
        .and_then(|(_, port)| port.parse().ok())
        .unwrap_or(80);
    let locale = Locale::from_request(request.headers(), request.method(), port);
    request.extensions_mut().insert(locale);
    next.run(request).await
}

fn language_index(code: &str) -> Option<usize> {
    LANGUAGES.iter().position(|(lang, _)| *lang == code)
}

fn cookie_name(port: u16) -> String {
    format!("skilvella-language-{port}")
}

fn resolve_tag(tag: &str) -> Option<usize> {
    let tag = tag.to_ascii_lowercase();
    let mut parts = tag.split('-');
    let code = match parts.next()? {
        "zh" if parts.any(|part| matches!(part, "tw" | "hk" | "mo" | "hant")) => "zh-TW",
        "zh" => "zh-CN",
        "pt" => "pt-BR",
        primary => primary,
    };
    language_index(code)
}

fn browser_language(headers: &HeaderMap) -> usize {
    let mut best = None;
    for value in headers.get_all(header::ACCEPT_LANGUAGE) {
        let Ok(value) = value.to_str() else {
            continue;
        };
        for range in value.split(',') {
            let mut parts = range.split(';').map(str::trim);
            let Some(index) = parts.next().and_then(resolve_tag) else {
                continue;
            };
            let quality = parts
                .find_map(|part| part.strip_prefix("q="))
                .map_or(1.0, |weight| weight.parse::<f32>().unwrap_or(0.0));
            if quality > 0.0 && quality <= 1.0 && best.is_none_or(|(_, current)| quality > current)
            {
                best = Some((index, quality));
            }
        }
    }
    // 言語を通知しないクライアントには従来のSSRと同じ日本語を返す。
    best.map_or(
        usize::from(headers.contains_key(header::ACCEPT_LANGUAGE)),
        |(index, _)| index,
    )
}
