pub mod library;
pub mod mcp;
pub mod model;
pub mod server;
pub mod update;
pub mod watch;
pub mod web;

use library::Library;
use std::sync::Arc;

/// SHA-256のダイジェスト（バイト列）を小文字16進文字列へ変換する共通関数。
///
/// - スキルID・リビジョン（SQLiteに保存済み）、MCPの継続用リビジョン、更新物の
///   チェックサム照合がすべてこの表現に依存するため、出力は「1バイト=小文字2桁、
///   上位4ビットが先」の形式から絶対に変えないこと（値が変わると既存IDが失効する）。
/// - ハッシュ計算そのもの（長さ接頭辞の付け方・ストリーミング読込み）は用途ごとに
///   異なるので各呼び出し元に残し、ここではバイト列の16進化だけを共通化する。
pub(crate) fn sha256_hex(digest: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut text = String::with_capacity(digest.len() * 2);
    for byte in digest {
        text.push(HEX[(byte >> 4) as usize] as char);
        text.push(HEX[(byte & 0x0f) as usize] as char);
    }
    text
}

#[derive(Clone)]
pub struct AppState {
    pub library: Library,
    pub csrf: Arc<str>,
    pub rescan: Arc<tokio::sync::Notify>,
    pub shutdown: Arc<tokio::sync::Notify>,
}

impl AppState {
    pub fn new(library: Library) -> Self {
        Self {
            library,
            csrf: uuid::Uuid::new_v4().to_string().into(),
            rescan: Arc::new(tokio::sync::Notify::new()),
            shutdown: Arc::new(tokio::sync::Notify::new()),
        }
    }

    pub async fn run<T, F>(&self, action: F) -> anyhow::Result<T>
    where
        T: Send + 'static,
        F: FnOnce(Library) -> anyhow::Result<T> + Send + 'static,
    {
        let library = self.library.clone();
        tokio::task::spawn_blocking(move || action(library)).await?
    }

    pub async fn run_mut<T, F>(&self, action: F) -> anyhow::Result<T>
    where
        T: Send + 'static,
        F: FnOnce(Library) -> anyhow::Result<T> + Send + 'static,
    {
        self.run(move |library| {
            let result = action(library.clone());
            // 保存結果と同期状態を同じレスポンスで表示できるよう、同期の終了を待つ。
            // 同期失敗はlast_errorへ保存し、原本への保存自体は取り消さない。
            if let Err(error) = library.sync_codex_entries()
                && result.is_ok()
                && library
                    .sync_summary()
                    .map_or(true, |settings| settings.last_error.is_empty())
            {
                return Err(error.context(
                    "変更を保存しましたが、呼び出し入口の同期とエラー状態の保存に失敗しました",
                ));
            }
            result
        })
        .await
    }
}
