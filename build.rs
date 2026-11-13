//! Windows版の実行ファイルへアプリアイコンをリソースとして埋め込む。
//!
//! スタートメニューのショートカット（アプリ自身が作る `Skilvella.lnk` とVelopackが作る項目）、
//! サインイン時の起動登録、Explorerの表示はいずれも実行ファイル内のアイコンを参照する。
//! リソースが無いとWindows既定の空白アイコンになるため、同梱の `skilvella.ico` を
//! `RT_ICON` / `RT_GROUP_ICON` として `.res` に書き出し、MSVCリンカーへ直接渡す。
//!
//! リソースコンパイラー（rc.exe）やビルド用クレートに依存しないよう、`.res` の形式は
//! ここで組み立てる。MSVCの `link.exe` と `lld-link` は `.res` を入力としてそのまま受け付ける。

use std::{env, fs, path::PathBuf};

const ICON: &str = "static/icons/skilvella.ico";
const RT_ICON: u16 = 3;
const RT_GROUP_ICON: u16 = 14;
/// Explorerは最小の識別子のアイコングループを実行ファイルの代表アイコンにする。
const GROUP_ID: u16 = 1;
/// MOVEABLE | DISCARDABLE（rc.exeが `ICON` に付ける既定値）
const ICON_FLAGS: u16 = 0x1010;
/// MOVEABLE | PURE | DISCARDABLE（rc.exeがアイコングループに付ける既定値）
const GROUP_FLAGS: u16 = 0x1030;
/// 言語中立。表示言語に関係なく同じアイコンを返す。
const LANGUAGE: u16 = 0;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed={ICON}");
    // ホストではなくビルド対象で判定する。LinuxとmacOSのビルドでは何もしない。
    let os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_env = env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    if os != "windows" || target_env != "msvc" {
        return;
    }
    let icon = fs::read(ICON).expect("read the application icon");
    let resource = icon_resource(&icon);
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR")).join("skilvella.res");
    fs::write(&output, resource).expect("write the icon resource");
    // 本体だけに付ける。配布ツールや起動補助は利用者がスタートメニューから開くものではない。
    println!("cargo:rustc-link-arg-bin=skilvella={}", output.display());
}

/// ICOファイルを、各画像の `RT_ICON` と、それらを束ねる `RT_GROUP_ICON` の `.res` に変換する。
fn icon_resource(icon: &[u8]) -> Vec<u8> {
    let u16_at = |at: usize| u16::from_le_bytes([icon[at], icon[at + 1]]);
    let u32_at = |at: usize| u32::from_le_bytes(icon[at..at + 4].try_into().unwrap());
    // ICONDIR: reserved(0)・type(1=ICO)・画像数
    assert!(
        icon.len() >= 6 && u16_at(0) == 0 && u16_at(2) == 1,
        "{ICON} is not an ICO file"
    );
    let count = u16_at(4);
    assert!(count > 0, "{ICON} has no images");

    // .resの先頭は、32ビット形式であることを示す空のエントリー。
    let mut out = Vec::new();
    push_entry(&mut out, 0, 0, 0, &[]);
    // GRPICONDIR: ICONDIRと同じ6バイトの見出しに、14バイトの項目が続く。
    let mut group = Vec::with_capacity(6 + 14 * usize::from(count));
    group.extend_from_slice(&icon[..6]);
    for index in 0..count {
        let entry = 6 + 16 * usize::from(index);
        assert!(icon.len() >= entry + 16, "{ICON} has a truncated directory");
        let size = u32_at(entry + 8) as usize;
        let offset = u32_at(entry + 12) as usize;
        let image = icon
            .get(offset..offset + size)
            .unwrap_or_else(|| panic!("{ICON} has an image outside the file"));
        // 256pxのPNG圧縮画像も含め、画像のバイト列はそのまま格納する。
        let id = index + 1;
        push_entry(&mut out, RT_ICON, id, ICON_FLAGS, image);
        // 幅・高さ・色数・予約・プレーン数・ビット数・サイズはICOの項目と同じで、
        // ファイル内の位置（4バイト）だけがRT_ICONの識別子（2バイト）に置き換わる。
        group.extend_from_slice(&icon[entry..entry + 12]);
        group.extend_from_slice(&id.to_le_bytes());
    }
    push_entry(&mut out, RT_GROUP_ICON, GROUP_ID, GROUP_FLAGS, &group);
    out
}

/// 種類と名前を数値の識別子で持つリソースを1件追加する（見出しは常に32バイト）。
fn push_entry(out: &mut Vec<u8>, kind: u16, name: u16, flags: u16, data: &[u8]) {
    const HEADER_SIZE: u32 = 32;
    out.extend_from_slice(&u32::try_from(data.len()).unwrap().to_le_bytes());
    out.extend_from_slice(&HEADER_SIZE.to_le_bytes());
    // 0xFFFFに続く値は、文字列ではなく数値の識別子であることを示す。
    for ordinal in [kind, name] {
        out.extend_from_slice(&0xFFFF_u16.to_le_bytes());
        out.extend_from_slice(&ordinal.to_le_bytes());
    }
    out.extend_from_slice(&0_u32.to_le_bytes()); // DataVersion
    out.extend_from_slice(&flags.to_le_bytes()); // MemoryFlags
    out.extend_from_slice(&LANGUAGE.to_le_bytes()); // LanguageId
    out.extend_from_slice(&0_u32.to_le_bytes()); // Version
    out.extend_from_slice(&0_u32.to_le_bytes()); // Characteristics
    out.extend_from_slice(data);
    // 次の見出しはDWORD境界から始める。
    out.resize(out.len().next_multiple_of(4), 0);
}
