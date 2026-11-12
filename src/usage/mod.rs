//! Token and cost statistics from the agents' own session logs on this
//! computer (Claude Code's and Codex's JSONL files), and the history of the
//! plan limits the daemon reports, with a forecast of when one runs out.
//!
//! Parsing and the sums are plain functions, so tests feed them strings;
//! reading the folders is in `scan`, which only exists where there is a
//! local disk to read (not iOS, not the browser).

// Without `scan`, only the limits half is used; the rest stays for the
// tests and the desktop.
#![cfg_attr(any(target_os = "ios", target_os = "android", target_arch = "wasm32"), allow(dead_code))]

pub(crate) mod limits;
pub(crate) mod parse;
pub(crate) mod price;
#[cfg(not(any(target_os = "ios", target_os = "android", target_arch = "wasm32")))]
pub(crate) mod scan;

use std::collections::HashMap;

/// Which agent wrote a log.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Source {
    Claude,
    Codex,
}

/// Token counts. `input` is the uncached part of the prompt only; cache
/// reads and writes are counted apart, as both providers bill them apart.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Tokens {
    pub(crate) input: u64,
    pub(crate) output: u64,
    pub(crate) cache_read: u64,
    pub(crate) cache_write: u64,
}

impl Tokens {
    pub(crate) fn total(&self) -> u64 {
        self.input + self.output + self.cache_read + self.cache_write
    }

    fn add(&mut self, o: &Tokens) {
        self.input += o.input;
        self.output += o.output;
        self.cache_read += o.cache_read;
        self.cache_write += o.cache_write;
    }

    /// The share of the prompt served from the cache: reads over every
    /// prompt token (uncached, read and written). `None` with no prompt.
    pub(crate) fn cache_hit_rate(&self) -> Option<f64> {
        let prompt = self.input + self.cache_read + self.cache_write;
        (prompt > 0).then(|| self.cache_read as f64 / prompt as f64)
    }
}

/// One model response's usage, as a log records it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Entry {
    /// Unix seconds.
    pub(crate) at: i64,
    pub(crate) source: Source,
    pub(crate) model: String,
    pub(crate) tokens: Tokens,
    /// What identifies the response across lines and files: a streamed
    /// message is written several times, and a resumed session copies
    /// earlier lines into a new file. `None`: counted as it is.
    pub(crate) key: Option<u64>,
}

/// Sums over a span: tokens, and the estimated cost of the priced part.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct Totals {
    pub(crate) tokens: Tokens,
    /// USD at API list prices, of the tokens of priced models.
    pub(crate) cost: f64,
    /// Tokens of models the price table does not know.
    pub(crate) unpriced: u64,
    pub(crate) responses: u64,
}

impl Totals {
    fn add(&mut self, e: &Entry) {
        self.tokens.add(&e.tokens);
        self.responses += 1;
        match price::cost(&e.model, &e.tokens) {
            Some(c) => self.cost += c,
            None => self.unpriced += e.tokens.total(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ModelRow {
    pub(crate) source: Source,
    pub(crate) model: String,
    pub(crate) priced: bool,
    pub(crate) totals: Totals,
}

/// One range of the page (today, 7 or 30 days): its sums, by model (the
/// costliest first), and by day (oldest first, every day of the range).
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Stats {
    pub(crate) totals: Totals,
    pub(crate) models: Vec<ModelRow>,
    pub(crate) days: Vec<Totals>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum Range {
    #[default]
    Today,
    Week,
    Month,
}

impl Range {
    pub(crate) fn days(self) -> i64 {
        match self {
            Range::Today => 1,
            Range::Week => 7,
            Range::Month => 30,
        }
    }
}

/// What the page shows of the session logs.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Report {
    pub(crate) today: Stats,
    pub(crate) week: Stats,
    pub(crate) month: Stats,
    /// The local day (days since the epoch) the last column is.
    pub(crate) last_day: i64,
    pub(crate) files: usize,
}

// A report as JSON, for a phone to show the computer's (it has no session
// logs of its own): the host's Sophon reads them and sends this.
impl Tokens {
    fn to_json(self) -> serde_json::Value {
        serde_json::json!([self.input, self.output, self.cache_read, self.cache_write])
    }
    fn from_json(v: &serde_json::Value) -> Option<Self> {
        let n = |i: usize| v.get(i).and_then(serde_json::Value::as_u64);
        Some(Tokens { input: n(0)?, output: n(1)?, cache_read: n(2)?, cache_write: n(3)? })
    }
}

impl Totals {
    fn to_json(self) -> serde_json::Value {
        serde_json::json!({ "t": self.tokens.to_json(), "c": self.cost, "u": self.unpriced, "r": self.responses })
    }
    fn from_json(v: &serde_json::Value) -> Option<Self> {
        Some(Totals {
            tokens: Tokens::from_json(v.get("t")?)?,
            cost: v.get("c")?.as_f64()?,
            unpriced: v.get("u")?.as_u64()?,
            responses: v.get("r")?.as_u64()?,
        })
    }
}

impl Stats {
    fn to_json(&self) -> serde_json::Value {
        let models: Vec<_> = self
            .models
            .iter()
            .map(|m| serde_json::json!({ "s": if m.source == Source::Claude { "claude" } else { "codex" }, "m": m.model, "p": m.priced, "t": m.totals.to_json() }))
            .collect();
        serde_json::json!({ "totals": self.totals.to_json(), "models": models, "days": self.days.iter().map(|d| d.to_json()).collect::<Vec<_>>() })
    }
    fn from_json(v: &serde_json::Value) -> Option<Self> {
        let models = v
            .get("models")?
            .as_array()?
            .iter()
            .filter_map(|m| {
                Some(ModelRow {
                    source: if m.get("s")?.as_str()? == "claude" { Source::Claude } else { Source::Codex },
                    model: m.get("m")?.as_str()?.to_owned(),
                    priced: m.get("p")?.as_bool()?,
                    totals: Totals::from_json(m.get("t")?)?,
                })
            })
            .collect();
        let days = v.get("days")?.as_array()?.iter().filter_map(Totals::from_json).collect();
        Some(Stats { totals: Totals::from_json(v.get("totals")?)?, models, days })
    }
}

impl Report {
    pub(crate) fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "today": self.today.to_json(), "week": self.week.to_json(), "month": self.month.to_json(),
            "lastDay": self.last_day, "files": self.files,
        })
    }

    pub(crate) fn from_json(v: &serde_json::Value) -> Option<Self> {
        Some(Report {
            today: Stats::from_json(v.get("today")?)?,
            week: Stats::from_json(v.get("week")?)?,
            month: Stats::from_json(v.get("month")?)?,
            last_day: v.get("lastDay")?.as_i64()?,
            files: v.get("files")?.as_u64()? as usize,
        })
    }

    pub(crate) fn range(&self, r: Range) -> &Stats {
        match r {
            Range::Today => &self.today,
            Range::Week => &self.week,
            Range::Month => &self.month,
        }
    }
}

/// The local day of `at`: days since the epoch in local time, `offset`
/// giving the seconds east of UTC at a moment (it changes with daylight
/// saving).
pub(crate) fn local_day(at: i64, offset: &dyn Fn(i64) -> i64) -> i64 {
    (at + offset(at)).div_euclid(86_400)
}

/// The report over `entries` at `now`. Duplicates (same key) count once:
/// the copy with the most output, as a streamed message's earlier copies
/// can hold partial counts.
pub(crate) fn aggregate<'a>(entries: impl IntoIterator<Item = &'a Entry>, now: i64, offset: &dyn Fn(i64) -> i64) -> Report {
    let today = local_day(now, offset);
    let first = today - Range::Month.days() + 1;
    let mut keyed: HashMap<u64, &Entry> = HashMap::new();
    let mut loose: Vec<&Entry> = vec![];
    for e in entries {
        if e.at > now + 86_400 || local_day(e.at, offset) < first {
            continue;
        }
        match e.key {
            Some(k) => {
                let slot = keyed.entry(k).or_insert(e);
                if e.tokens.output > slot.tokens.output {
                    *slot = e;
                }
            }
            None => loose.push(e),
        }
    }
    // In time order, so the sums come out the same each time.
    let mut kept: Vec<&Entry> = keyed.into_values().chain(loose).collect();
    kept.sort_by_key(|e| (e.at, e.key));
    let stats = |range: Range| {
        let from = today - range.days() + 1;
        let mut s = Stats { days: vec![Totals::default(); range.days() as usize], ..Default::default() };
        let mut models: HashMap<(Source, &str), Totals> = HashMap::new();
        for e in &kept {
            let day = local_day(e.at, offset);
            if day < from || day > today {
                continue;
            }
            s.totals.add(e);
            s.days[(day - from) as usize].add(e);
            models.entry((e.source, e.model.as_str())).or_default().add(e);
        }
        s.models = models
            .into_iter()
            .map(|((source, model), totals)| ModelRow { source, model: model.to_owned(), priced: price::of(model).is_some(), totals })
            .collect();
        s.models.sort_by(|a, b| {
            b.totals.cost.total_cmp(&a.totals.cost).then(b.totals.tokens.total().cmp(&a.totals.tokens.total())).then(a.model.cmp(&b.model))
        });
        s
    };
    Report { today: stats(Range::Today), week: stats(Range::Week), month: stats(Range::Month), last_day: today, files: 0 }
}

/// Seconds east of UTC at `at`, this computer's time zone (UTC in a
/// browser, which has no zone database here).
pub(crate) fn local_offset(at: i64) -> i64 {
    #[cfg(not(target_arch = "wasm32"))]
    {
        use chrono::{Offset, TimeZone};
        chrono::Local.timestamp_opt(at, 0).single().map_or(0, |t| t.offset().fix().local_minus_utc() as i64)
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = at;
        0
    }
}

pub(crate) fn now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
}

/// (year, month, day) of days since the epoch (Howard Hinnant's
/// algorithm).
pub(crate) fn civil(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

/// A local day as `11-12`.
pub(crate) fn day_label(day: i64) -> String {
    let (_, m, d) = civil(day);
    format!("{m:02}-{d:02}")
}

/// A moment in local time: `14:05` today, `11-12 14:05` otherwise.
pub(crate) fn time_label(at: i64, offset: &dyn Fn(i64) -> i64) -> String {
    let local = at + offset(at);
    let (h, mi) = (local.rem_euclid(86_400) / 3600, local.rem_euclid(3600) / 60);
    let day = local.div_euclid(86_400);
    if day == local_day(now(), offset) {
        format!("{h:02}:{mi:02}")
    } else {
        format!("{} {h:02}:{mi:02}", day_label(day))
    }
}

/// A short count: 950, 12.3K, 4.56M, 1.2B.
pub(crate) fn compact(n: u64) -> String {
    let f = n as f64;
    match n {
        0..1_000 => n.to_string(),
        1_000..1_000_000 => format!("{:.1}K", f / 1e3),
        1_000_000..1_000_000_000 => format!("{:.2}M", f / 1e6),
        _ => format!("{:.2}B", f / 1e9),
    }
}

/// Dollars: cents under $100, whole dollars above.
pub(crate) fn dollars(v: f64) -> String {
    if v >= 100.0 {
        let whole = v.round() as u64;
        let s = whole.to_string();
        let mut out = String::new();
        for (i, c) in s.chars().enumerate() {
            if i > 0 && (s.len() - i) % 3 == 0 {
                out.push(',');
            }
            out.push(c);
        }
        format!("${out}")
    } else {
        format!("${v:.2}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(at: i64, model: &str, output: u64, key: Option<u64>) -> Entry {
        Entry { at, source: Source::Claude, model: model.into(), tokens: Tokens { input: 10, output, cache_read: 90, cache_write: 0 }, key }
    }

    const DAY: i64 = 86_400;

    #[test]
    fn ranges_split_by_local_day_and_dedupe() {
        // Now: day 100 at noon UTC, in UTC+8 (20:00 local).
        let now = 100 * DAY + 12 * 3600;
        let utc8 = |_: i64| 8 * 3600;
        let entries = [
            entry(now - 60, "claude-opus-5-5", 5, Some(1)),
            // The same message streamed again with its final count.
            entry(now - 59, "claude-opus-5-5", 50, Some(1)),
            // 18:00 UTC the day before is 02:00 local today.
            entry(99 * DAY + 18 * 3600, "claude-opus-5-5", 100, None),
            // Three days ago: in the week, not today.
            entry(now - 3 * DAY, "mystery-model", 7, None),
            // Older than 30 days: nowhere.
            entry(now - 40 * DAY, "claude-opus-5-5", 1, None),
        ];
        let r = aggregate(entries.iter(), now, &utc8);
        assert_eq!(r.today.totals.responses, 2);
        assert_eq!(r.today.totals.tokens.output, 150);
        assert_eq!(r.week.totals.responses, 3);
        assert_eq!(r.month.totals.responses, 3);
        assert_eq!(r.week.days.len(), 7);
        assert_eq!(r.week.days[6].tokens.output, 150);
        assert_eq!(r.week.days[3].tokens.output, 7);
        // The unknown model is unpriced, the known one costs something.
        assert_eq!(r.week.totals.unpriced, 107);
        assert!(r.week.totals.cost > 0.0);
        assert_eq!(r.week.models[0].model, "claude-opus-5-5");
        assert!(!r.week.models[1].priced);
    }

    #[test]
    fn cache_hit_rate_counts_every_prompt_token() {
        let t = Tokens { input: 10, output: 5, cache_read: 80, cache_write: 10 };
        assert_eq!(t.cache_hit_rate(), Some(0.8));
        assert_eq!(Tokens::default().cache_hit_rate(), None);
    }

    #[test]
    fn short_numbers() {
        assert_eq!(compact(950), "950");
        assert_eq!(compact(12_345), "12.3K");
        assert_eq!(compact(4_560_000), "4.56M");
        assert_eq!(dollars(3.456), "$3.46");
        assert_eq!(dollars(12_345.6), "$12,346");
    }

    #[test]
    fn civil_dates() {
        assert_eq!(civil(0), (1970, 1, 1));
        assert_eq!(civil(1_794_035_289 / 86_400), (2026, 11, 7));
        assert_eq!(civil(-1), (1969, 12, 31));
        assert_eq!(day_label(1_794_035_289 / 86_400), "11-07");
    }

    #[test]
    fn a_report_survives_the_trip_to_a_phone() {
        let totals = Totals { tokens: Tokens { input: 1, output: 2, cache_read: 3, cache_write: 4 }, cost: 1.5, unpriced: 7, responses: 9 };
        let stats = Stats {
            totals,
            models: vec![ModelRow { source: Source::Codex, model: "gpt".into(), priced: true, totals }],
            days: vec![totals, Totals::default()],
        };
        let report = Report { today: stats.clone(), week: stats.clone(), month: stats, last_day: 20_000, files: 3 };
        assert_eq!(Report::from_json(&report.to_json()), Some(report));
    }
}
