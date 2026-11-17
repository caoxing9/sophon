//! Short relative times for lists ("now", "5m", "3h", "2d").

use std::time::{SystemTime, UNIX_EPOCH};

/// Seconds since the epoch of an RFC 3339 time (`2026-11-07T07:08:09.123Z`,
/// or with an offset).
fn parse(s: &str) -> Option<i64> {
    let b = s.as_bytes();
    let num = |from: usize, len: usize| -> Option<i64> { s.get(from..from + len)?.parse().ok() };
    let (y, mo, d) = (num(0, 4)?, num(5, 2)?, num(8, 2)?);
    let (h, mi, sec) = (num(11, 2)?, num(14, 2)?, num(17, 2)?);
    // Days from the civil date (Howard Hinnant's algorithm).
    let y2 = if mo <= 2 { y - 1 } else { y };
    let era = y2.div_euclid(400);
    let yoe = y2 - era * 400;
    let doy = (153 * (if mo > 2 { mo - 3 } else { mo + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    let mut t = days * 86_400 + h * 3600 + mi * 60 + sec;
    // A trailing offset, `+08:00` / `-05:00`.
    if let Some(i) = s[19..].find(['+', '-']).map(|i| i + 19)
        && b.len() >= i + 6
    {
        let sign = if b[i] == b'+' { 1 } else { -1 };
        let off = num(i + 1, 2)? * 3600 + num(i + 4, 2)? * 60;
        t -= sign * off;
    }
    Some(t)
}

/// Seconds since the epoch of RFC 3339 time `s`.
pub(crate) fn epoch_of(s: &str) -> Option<i64> {
    parse(s)
}

/// Now, in seconds since the epoch.
pub(crate) fn now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

/// How long ago `s` was, as monocode says it ("5 minutes ago"); empty
/// when it does not parse.
pub(crate) fn ago_long(s: &str) -> String {
    let Some(then) = parse(s) else { return String::new() };
    let n = (now() - then).max(0);
    let zh = crate::i18n::is_zh();
    let (count, unit_en, unit_zh) = match n {
        n if n < 60 => return if zh { "刚刚".into() } else { "just now".into() },
        n if n < 3600 => (n / 60, "minute", "分钟"),
        n if n < 86_400 => (n / 3600, "hour", "小时"),
        n if n < 86_400 * 30 => (n / 86_400, "day", "天"),
        n if n < 86_400 * 365 => (n / (86_400 * 30), "month", "个月"),
        n => (n / (86_400 * 365), "year", "年"),
    };
    if zh {
        format!("{count} {unit_zh}前")
    } else {
        format!("{count} {unit_en}{} ago", if count == 1 { "" } else { "s" })
    }
}

/// How long ago `s` was, briefly; empty when it does not parse.
pub(crate) fn ago(s: &str) -> String {
    let Some(then) = parse(s) else { return String::new() };
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(then);
    match (now - then).max(0) {
        n if n < 60 => crate::i18n::t("now").into(),
        n if n < 3600 => format!("{}m", n / 60),
        n if n < 86_400 => format!("{}h", n / 3600),
        n if n < 86_400 * 30 => format!("{}d", n / 86_400),
        n => format!("{}mo", n / (86_400 * 30)),
    }
}

#[cfg(test)]
mod tests {
    use super::parse;

    #[test]
    fn parses_utc_and_offsets() {
        assert_eq!(parse("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(parse("2026-11-07T07:08:09.123Z"), Some(1_794_035_289));
        assert_eq!(parse("2026-11-07T15:08:09+08:00"), Some(1_794_035_289));
    }
}
