//! 管理画面のHTTPテストで共有する補助。
//!
//! 各テストクレート（`mod common;` で読み込む側）が全関数を使うとは限らないため、
//! 補助関数には個別に `#[allow(dead_code)]` を付けて `clippy --all-targets -D warnings` を通す。

use reqwest::{Client, IntoUrl, RequestBuilder, Response};
use skilvella::AppState;
use std::path::PathBuf;

/// リダイレクトを追わないクライアント。303の `Location` や状態コード自体を検証するために使う。
#[allow(dead_code)]
pub fn no_redirect_client() -> Client {
    Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap()
}

/// フォームPOSTの土台。`Content-Type: application/x-www-form-urlencoded` だけを付ける。
///
/// 本文・Origin・Accept などは呼び出し側で追加する。本文のエンコード方法はテストごとに
/// 送信バイトが異なる（生の連結・全バイトの%エンコードなど）ため、ここでは決めない。
#[allow(dead_code)]
pub fn form_post(client: &Client, url: impl IntoUrl) -> RequestBuilder {
    client
        .post(url)
        .header("content-type", "application/x-www-form-urlencoded")
}

/// RFC 3986 の非予約文字（英数字と `-._~`）以外を `%XX` にする。
#[allow(dead_code)]
pub fn encode_form_value(s: &str) -> String {
    s.bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}

/// キーと値を `encode_form_value` でエンコードし、`&` で連結したフォーム本文を作る。
#[allow(dead_code)]
pub fn encode_form(fields: &[(&str, &str)]) -> String {
    fields
        .iter()
        .map(|(k, v)| format!("{}={}", encode_form_value(k), encode_form_value(v)))
        .collect::<Vec<_>>()
        .join("&")
}

/// `encode_form` の本文でフォームをPOSTし、応答を返す。
#[allow(dead_code)]
pub async fn post_form(client: &Client, url: &str, fields: &[(&str, &str)]) -> Response {
    form_post(client, url)
        .body(encode_form(fields))
        .send()
        .await
        .unwrap()
}

/// `SKILVELLA_MIGRATION_TEST_ROOT`（未設定ならOSの一時フォルダー）配下の `{label}-{UUID}` を返す。
///
/// ディレクトリは作成しない（呼び出し側が必要な子フォルダーを作る）。unixで一時フォルダーが
/// シンボリックリンク経由でも、従来どおり正規化（canonicalize）せずに使う。
#[allow(dead_code)]
pub fn isolated_root(label: &str) -> PathBuf {
    std::env::var_os("SKILVELLA_MIGRATION_TEST_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join(format!("{label}-{}", uuid::Uuid::new_v4()))
}

pub struct HttpServer {
    pub base: String,
    task: tokio::task::JoinHandle<()>,
}

impl HttpServer {
    pub async fn start(state: AppState) -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let router = skilvella::server::router(state, port);
        let task = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        Self {
            base: format!("http://127.0.0.1:{port}"),
            task,
        }
    }
}

impl Drop for HttpServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}
