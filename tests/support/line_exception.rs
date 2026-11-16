//! 検査の行単位の例外。
//!
//! `correct-allow:` を付けた行だけを外す。理由、期限、人の承認が同じ行にないと失敗する。
//! 期限を過ぎた例外も失敗する。

use std::time::{SystemTime, UNIX_EPOCH};

const MARKER: &str = "correct-allow:";

/// 行に有効な例外があれば `Ok(true)`。印がなければ `Ok(false)`。印が壊れていれば理由を返す。
#[allow(dead_code)]
pub fn exemption(line: &str) -> Result<bool, String> {
    let Some((_, rest)) = line.split_once(MARKER) else {
        return Ok(false);
    };
    validate_exemption(rest.trim()).map(|()| true)
}

/// `#` コメントより前。引用符の中の `#` はコメントにしない。
#[allow(dead_code)]
pub fn code_before_comment(line: &str) -> &str {
    let mut quote = None;
    for (index, character) in line.char_indices() {
        match (quote, character) {
            (None, '"' | '\'') => quote = Some(character),
            (Some(open), character) if character == open => quote = None,
            (None, '#') => return &line[..index],
            _ => {}
        }
    }
    line
}

fn validate_exemption(text: &str) -> Result<(), String> {
    let mut reason = None;
    let mut expires = None;
    let mut approved = None;
    for (index, part) in text.split(';').enumerate() {
        let part = part.trim();
        if part.is_empty() {
            return Err(exemption_format());
        }
        if index == 0 && !part.contains(':') {
            reason = Some(part);
            continue;
        }
        if let Some(value) = part.strip_prefix("expires:") {
            expires = Some(value.trim());
        } else if let Some(value) = part.strip_prefix("approved:") {
            approved = Some(value.trim());
        } else if let Some(value) = part.strip_prefix("reason:") {
            reason = Some(value.trim());
        } else {
            return Err(exemption_format());
        }
    }
    let Some(reason) = reason
        .filter(|reason| reason.chars().count() >= 8 && reason.chars().any(char::is_alphabetic))
    else {
        return Err(exemption_format());
    };
    let _ = reason;
    let Some(expires) = expires else {
        return Err(exemption_format());
    };
    let Some(approved) = approved.filter(|name| approved_is_human(name)) else {
        return Err(exemption_format());
    };
    let _ = approved;
    let expiry = parse_date(expires).ok_or_else(exemption_format)?;
    let today = today_utc();
    if expiry < today {
        return Err(format!(
            "correct-allow expired on {expires}. Pin the supported form, or replace the exception with a later expires date and a person's approval"
        ));
    }
    Ok(())
}

fn exemption_format() -> String {
    "a correct-allow exception needs `correct-allow: <reason>; expires: YYYY-MM-DD; approved: <person>` on the same line. The reason is at least 8 characters, and approved is a person (not agent, claude, or bot)".to_owned()
}

fn approved_is_human(name: &str) -> bool {
    const BANNED: &[&str] = &[
        "agent", "claude", "bot", "todo", "gpt", "cursor", "human", "user",
    ];
    let lower = name.to_ascii_lowercase();
    if lower
        .split(|character: char| !character.is_alphanumeric())
        .any(|word| BANNED.contains(&word))
    {
        return false;
    }
    let letters = name
        .chars()
        .filter(|character| character.is_alphabetic())
        .count();
    (2..=80).contains(&name.len()) && letters >= 2
}

fn parse_date(text: &str) -> Option<(i32, u32, u32)> {
    let mut parts = text.split('-');
    let year: i32 = parts.next()?.parse().ok()?;
    let month: u32 = parts.next()?.parse().ok()?;
    let day: u32 = parts.next()?.parse().ok()?;
    if parts.next().is_some() || !(1970..=2999).contains(&year) {
        return None;
    }
    let max_day = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap(year) => 29,
        2 => 28,
        _ => return None,
    };
    (1..=max_day).contains(&day).then_some((year, month, day))
}

fn is_leap(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

fn today_utc() -> (i32, u32, u32) {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    civil_from_days((seconds / 86_400) as i64)
}

/// Howard Hinnant の civil_from_days。Unix 日数 0 は 1970-01-01。
fn civil_from_days(mut days: i64) -> (i32, u32, u32) {
    days += 719_468;
    let era = if days >= 0 { days } else { days - 146_096 } / 146_097;
    let day_of_era = (days - era * 146_097) as u64;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36524 - day_of_era / 146_096) / 365;
    let year = year_of_era as i64 + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_part = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_part + 2) / 5 + 1;
    let month = if month_part < 10 {
        month_part + 3
    } else {
        month_part - 9
    };
    let year = if month <= 2 { year + 1 } else { year };
    (year as i32, month as u32, day as u32)
}

#[cfg(test)]
mod tests {
    use super::{approved_is_human, civil_from_days, exemption, parse_date};

    #[test]
    fn unix_epoch_is_1970_01_01() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
    }

    #[test]
    fn rejects_impossible_dates() {
        assert!(parse_date("2026-02-31").is_none());
        assert!(parse_date("2026-13-01").is_none());
        assert!(parse_date("2024-02-29").is_some());
        assert!(parse_date("2026-02-29").is_none());
    }

    #[test]
    fn expired_and_unsigned_exceptions_fail() {
        let expired = "uses: actions/checkout@v1 # correct-allow: the tag is frozen upstream; expires: 2020-01-01; approved: Yuichiro Shinozaki";
        let error = exemption(expired).expect_err("expired");
        assert!(error.contains("expired"), "{error}");
        let unsigned = "uses: actions/checkout@v1 # correct-allow: the tag is frozen upstream; expires: 2999-01-01; approved: claude";
        assert!(exemption(unsigned).is_err());
        assert!(!approved_is_human("claude"));
        let valid = "uses: actions/checkout@v1 # correct-allow: the tag is frozen upstream; expires: 2999-01-01; approved: Yuichiro Shinozaki";
        assert_eq!(exemption(valid), Ok(true));
        assert_eq!(exemption("uses: actions/checkout@v1"), Ok(false));
    }
}
