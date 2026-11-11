//! When a schedule runs, picked without writing cron: every so often, every
//! day at a time, or on days of the week at a time. What is sent to the
//! service is still a cron expression (`0 9 * * 1-5`), with the time zone of
//! this device: the service reads an expression without one as UTC.

/// "Every `minutes`", as the service takes it: a cron step where the
/// clock divides evenly by it (every 15 minutes, every 6 hours), on the
/// quarter or the hour; otherwise a plain interval (`every:<ms>`), counted
/// from when the schedule is made.
pub fn every(minutes: u32) -> String {
    let minutes = minutes.max(1);
    match minutes {
        m if m < 60 && 60 % m == 0 => format!("*/{m} * * * *"),
        60 => "0 * * * *".into(),
        m if m % 60 == 0 && m < 1440 && 24 % (m / 60) == 0 => format!("0 */{} * * *", m / 60),
        m => format!("every:{}", m as u64 * 60_000),
    }
}

/// An interval of milliseconds in words.
pub fn interval_words(ms: u64, zh: bool) -> String {
    let (n, en, cn) = match ms {
        m if m >= 86_400_000 && m % 86_400_000 == 0 => (m / 86_400_000, "day", "天"),
        m if m >= 3_600_000 && m % 3_600_000 == 0 => (m / 3_600_000, "hour", "小时"),
        m => (m / 60_000, "minute", "分钟"),
    };
    match (zh, n) {
        (true, 1) if cn != "分钟" => format!("每{cn}"),
        (true, _) => format!("每 {n} {cn}"),
        (false, 1) => format!("Every {en}"),
        (false, _) => format!("Every {n} {en}s"),
    }
}

/// A time of day as typed: `9`, `9:30`, `09.30`, `930`, `9：30`.
pub fn parse_time(typed: &str) -> Option<(u8, u8)> {
    let typed = typed.trim();
    let digits = |s: &str| (!s.is_empty() && s.len() <= 2 && s.bytes().all(|b| b.is_ascii_digit())).then(|| s.parse::<u8>().ok()).flatten();
    let (hour, minute) = match typed.split_once([':', '：', '.', ' ']) {
        Some((h, m)) => (digits(h.trim())?, digits(m.trim())?),
        // `930`: the last two digits are the minutes.
        None if typed.len() >= 3 && typed.is_ascii() => (digits(&typed[..typed.len() - 2])?, digits(&typed[typed.len() - 2..])?),
        None => (digits(typed)?, 0),
    };
    (hour < 24 && minute < 60).then_some((hour, minute))
}

/// Every day at a time.
pub fn daily((hour, minute): (u8, u8)) -> String {
    format!("{minute} {hour} * * *")
}

/// On `days` (0 is Sunday) at a time; none of them is no schedule, all of
/// them is every day.
pub fn weekly(days: &[u8], time: (u8, u8)) -> Option<String> {
    let mut days: Vec<u8> = days.iter().map(|d| d % 7).collect();
    days.sort_unstable();
    days.dedup();
    match days.len() {
        0 => None,
        7 => Some(daily(time)),
        _ if days == [1, 2, 3, 4, 5] => Some(format!("{} {} * * 1-5", time.1, time.0)),
        _ => Some(format!("{} {} * * {}", time.1, time.0, days.iter().map(u8::to_string).collect::<Vec<_>>().join(","))),
    }
}

/// The name of this device's time zone (`Asia/Shanghai`).
pub fn local_zone() -> Option<String> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        iana_time_zone::get_timezone().ok().filter(|z| !z.is_empty())
    }
    #[cfg(target_arch = "wasm32")]
    {
        None
    }
}

/// What the form shows for an expression: how it repeats and with what.
#[derive(Debug, Clone, PartialEq)]
pub enum Picked {
    /// Every `count` of a unit of minutes (1, 60 or 1440).
    Every { count: u32, unit: u32 },
    Daily(u8, u8),
    /// Days (0 is Sunday) and a time.
    Weekly(Vec<u8>, (u8, u8)),
    Custom(String),
}

/// An expression (`0 9 * * 1-5`, `every:<ms>`) as the form's choices; one
/// the form does not write is `Custom`.
pub fn picked(expression: &str) -> Picked {
    let every = |minutes: u32| {
        let unit = [1440, 60].into_iter().find(|u| minutes >= *u && minutes % u == 0).unwrap_or(1);
        Picked::Every { count: minutes / unit, unit }
    };
    let expression = expression.trim();
    if let Some(ms) = expression.strip_prefix("every:").and_then(|ms| ms.parse::<u64>().ok()) {
        return every((ms / 60_000).clamp(1, u32::MAX as u64) as u32);
    }
    let custom = || Picked::Custom(expression.to_owned());
    let fields: Vec<&str> = expression.split_whitespace().collect();
    let [minute, hour, "*", "*", weekday] = fields[..] else { return custom() };
    let number = |s: &str, below: u8| s.parse::<u8>().ok().filter(|n| *n < below);
    let step = |s: &str| s.strip_prefix("*/").and_then(|n| n.parse::<u32>().ok()).filter(|n| *n > 0);
    if weekday == "*" {
        match (step(minute), number(minute, 60), hour, step(hour)) {
            (Some(n), _, "*", _) => return every(n),
            (_, Some(0), "*", _) => return every(60),
            (_, Some(0), _, Some(n)) => return every(n * 60),
            _ => {}
        }
    }
    let (Some(h), Some(m)) = (number(hour, 24), number(minute, 60)) else { return custom() };
    match weekday {
        "*" => Picked::Daily(h, m),
        days => days_of(days).map_or_else(custom, |d| Picked::Weekly(d, (h, m))),
    }
}

/// The days a cron day-of-week field names (0 is Sunday), in order.
fn days_of(field: &str) -> Option<Vec<u8>> {
    let mut days = Vec::new();
    for part in field.split(',') {
        let one = |s: &str| s.parse::<u8>().ok().filter(|d| *d <= 7).map(|d| d % 7);
        match part.split_once('-') {
            Some((a, b)) => {
                let (a, b) = (a.parse::<u8>().ok()?, b.parse::<u8>().ok()?);
                if a > b || b > 7 {
                    return None;
                }
                days.extend((a..=b).map(|d| d % 7));
            }
            None => days.push(one(part)?),
        }
    }
    days.sort_unstable();
    days.dedup();
    (!days.is_empty()).then_some(days)
}

/// A cron expression in words, in the active language; the expression
/// itself when it is not one of the shapes this file writes.
pub fn describe(expression: &str) -> String {
    describe_in(expression, crate::i18n::is_zh())
}

fn describe_in(expression: &str, zh: bool) -> String {
    words(expression, zh).unwrap_or_else(|| format!("cron {}", expression.trim()))
}

fn words(expression: &str, zh: bool) -> Option<String> {
    if let Some(ms) = expression.trim().strip_prefix("every:") {
        return ms.parse().ok().map(|ms| interval_words(ms, zh));
    }
    let fields: Vec<&str> = expression.split_whitespace().collect();
    let [minute, hour, "*", "*", weekday] = fields[..] else { return None };
    let number = |s: &str, below: u8| s.parse::<u8>().ok().filter(|n| *n < below);
    let step = |s: &str| s.strip_prefix("*/").and_then(|n| n.parse::<u32>().ok()).filter(|n| *n > 0);
    if weekday == "*" {
        if let (Some(n), "*") = (step(minute), hour) {
            return Some(if zh { format!("每 {n} 分钟") } else { format!("Every {n} minutes") });
        }
        if let (Some(0), "*") = (number(minute, 60), hour) {
            return Some(if zh { "每小时".into() } else { "Every hour".into() });
        }
        if let (Some(0), Some(n)) = (number(minute, 60), step(hour)) {
            return Some(if zh { format!("每 {n} 小时") } else { format!("Every {n} hours") });
        }
    }
    let time = format!("{}:{:02}", number(hour, 24)?, number(minute, 60)?);
    if weekday == "*" {
        return Some(if zh { format!("每天 {time}") } else { format!("Every day at {time}") });
    }
    let days = days_of(weekday)?;
    Some(match days[..] {
        [0, 1, 2, 3, 4, 5, 6] if zh => format!("每天 {time}"),
        [0, 1, 2, 3, 4, 5, 6] => format!("Every day at {time}"),
        [1, 2, 3, 4, 5] if zh => format!("工作日 {time}"),
        [1, 2, 3, 4, 5] => format!("Weekdays at {time}"),
        [0, 6] if zh => format!("周末 {time}"),
        [0, 6] => format!("Weekends at {time}"),
        _ => {
            // Monday first, Sunday last, as the form shows them.
            let mut ordered = days.clone();
            ordered.sort_by_key(|d| (*d + 6) % 7);
            let names: Vec<&str> = ordered.iter().map(|d| day_name(*d, zh)).collect();
            if zh { format!("每周{} {time}", names.join("、")) } else { format!("{} at {time}", names.join(", ")) }
        }
    })
}

/// A day's short name (0 is Sunday).
pub fn day_name(day: u8, zh: bool) -> &'static str {
    let names = if zh { ["日", "一", "二", "三", "四", "五", "六"] } else { ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"] };
    names[(day % 7) as usize]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_time_is_read_as_people_type_it() {
        assert_eq!(parse_time("9"), Some((9, 0)));
        assert_eq!(parse_time(" 9:30 "), Some((9, 30)));
        assert_eq!(parse_time("09.05"), Some((9, 5)));
        assert_eq!(parse_time("9：30"), Some((9, 30)));
        assert_eq!(parse_time("930"), Some((9, 30)));
        assert_eq!(parse_time("2130"), Some((21, 30)));
        assert_eq!(parse_time("24:00"), None);
        assert_eq!(parse_time("9:60"), None);
        assert_eq!(parse_time("九点"), None);
        assert_eq!(parse_time(""), None);
    }

    #[test]
    fn the_form_writes_cron() {
        assert_eq!(every(15), "*/15 * * * *");
        assert_eq!(every(60), "0 * * * *");
        assert_eq!(every(360), "0 */6 * * *");
        // What the clock does not divide by runs on a plain interval.
        assert_eq!(every(45), "every:2700000");
        assert_eq!(every(300), "every:18000000");
        assert_eq!(every(2880), "every:172800000");
        assert_eq!(describe_in(&every(45), true), "每 45 分钟");
        assert_eq!(describe_in(&every(300), false), "Every 5 hours");
        assert_eq!(describe_in(&every(1440), true), "每天");
        assert_eq!(daily((9, 30)), "30 9 * * *");
        assert_eq!(weekly(&[5, 1, 3], (18, 0)).as_deref(), Some("0 18 * * 1,3,5"));
        assert_eq!(weekly(&[1, 2, 3, 4, 5], (9, 0)).as_deref(), Some("0 9 * * 1-5"));
        assert_eq!(weekly(&[0, 1, 2, 3, 4, 5, 6], (9, 0)).as_deref(), Some("0 9 * * *"));
        assert_eq!(weekly(&[], (9, 0)), None);
    }

    #[test]
    fn an_expression_fills_the_form_again() {
        assert_eq!(picked("*/15 * * * *"), Picked::Every { count: 15, unit: 1 });
        assert_eq!(picked("0 * * * *"), Picked::Every { count: 1, unit: 60 });
        assert_eq!(picked("0 */6 * * *"), Picked::Every { count: 6, unit: 60 });
        assert_eq!(picked(&every(45)), Picked::Every { count: 45, unit: 1 });
        assert_eq!(picked(&every(2880)), Picked::Every { count: 2, unit: 1440 });
        assert_eq!(picked("30 9 * * *"), Picked::Daily(9, 30));
        assert_eq!(picked("0 9 * * 1-5"), Picked::Weekly(vec![1, 2, 3, 4, 5], (9, 0)));
        assert_eq!(picked("0 9 1 * *"), Picked::Custom("0 9 1 * *".into()));
    }

    #[test]
    fn cron_reads_back_in_words() {
        assert_eq!(describe_in("*/30 * * * *", false), "Every 30 minutes");
        assert_eq!(describe_in("0 * * * *", true), "每小时");
        assert_eq!(describe_in("0 */6 * * *", true), "每 6 小时");
        assert_eq!(describe_in("30 9 * * *", true), "每天 9:30");
        assert_eq!(describe_in("0 9 * * 1-5", true), "工作日 9:00");
        assert_eq!(describe_in("0 9 * * 1-5", false), "Weekdays at 9:00");
        assert_eq!(describe_in("0 10 * * 0,6", true), "周末 10:00");
        assert_eq!(describe_in("0 18 * * 0,1,3", true), "每周一、三、日 18:00");
        assert_eq!(describe_in("0 18 * * 1,3", false), "Mon, Wed at 18:00");
        // What the form does not write stays as it is.
        assert_eq!(describe_in("0 9 1 * *", false), "cron 0 9 1 * *");
        assert_eq!(describe_in("5 * * * *", false), "cron 5 * * * *");
    }
}
