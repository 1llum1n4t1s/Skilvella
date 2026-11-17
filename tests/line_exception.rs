//! `correct-allow` の期限と承認者の契約。
//!
//! 実装は規約検査（`command_output`・`release_scripts`・`workflow_uses`）と `#[path]` で共有する。
//! 日付換算が壊れたときの失敗を、その規約検査の失敗と混ぜないため、契約の確認はこのファイルだけが持つ。

#[path = "support/line_exception.rs"]
mod line_exception;

#[test]
fn unix_epoch_is_1970_01_01() {
    assert_eq!(line_exception::civil_from_days(0), (1970, 1, 1));
}

#[test]
fn rejects_impossible_dates() {
    assert!(line_exception::parse_date("2026-02-31").is_none());
    assert!(line_exception::parse_date("2026-13-01").is_none());
    assert!(line_exception::parse_date("2024-02-29").is_some());
    assert!(line_exception::parse_date("2026-02-29").is_none());
}

#[test]
fn expired_and_unsigned_exceptions_fail() {
    let expired = "uses: actions/checkout@v1 # correct-allow: the tag is frozen upstream; expires: 2020-01-01; approved: Yuichiro Shinozaki";
    let error = line_exception::exemption(expired).expect_err("expired");
    assert!(error.contains("expired"), "{error}");
    let unsigned = "uses: actions/checkout@v1 # correct-allow: the tag is frozen upstream; expires: 2999-01-01; approved: claude";
    assert!(line_exception::exemption(unsigned).is_err());
    assert!(!line_exception::approved_is_human("claude"));
    let valid = "uses: actions/checkout@v1 # correct-allow: the tag is frozen upstream; expires: 2999-01-01; approved: Yuichiro Shinozaki";
    assert_eq!(line_exception::exemption(valid), Ok(true));
    assert_eq!(
        line_exception::exemption("uses: actions/checkout@v1"),
        Ok(false)
    );
}
