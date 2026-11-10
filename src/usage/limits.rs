//! The plan limits over time: each fetch of the daemon's usage leaves a
//! point (time, % used, reset time) per window in `usage-history.json`,
//! and the page draws the current window's points and says when the window
//! runs out at the pace so far.

use std::collections::BTreeMap;

use serde_json::{Value, json};

const FILE: &str = "usage-history.json";
/// Points older than this go.
const KEEP_SECS: i64 = 30 * 86_400;
const MAX_POINTS: usize = 400;
const MAX_WINDOWS: usize = 64;
/// An unchanged reading closer than this to the last one adds nothing.
const SAME_READING_GAP: i64 = 15 * 60;
/// Reset times reported a little apart are the same window's.
const RESET_SLACK: i64 = 10 * 60;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Point {
    pub(crate) at: i64,
    pub(crate) pct: f64,
    pub(crate) resets: Option<i64>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct History {
    /// By window: `provider/window id`.
    pub(crate) windows: BTreeMap<String, Vec<Point>>,
}

fn same_window(a: Option<i64>, b: Option<i64>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => (a - b).abs() <= RESET_SLACK,
        (None, None) => true,
        _ => false,
    }
}

impl History {
    pub(crate) fn from_json(text: &str) -> History {
        let Ok(v) = serde_json::from_str::<Value>(text) else { return History::default() };
        let mut h = History::default();
        if let Some(map) = v.get("windows").and_then(Value::as_object) {
            for (key, list) in map {
                let points = list
                    .as_array()
                    .map(|l| {
                        l.iter()
                            .filter_map(|p| {
                                Some(Point { at: p.get(0)?.as_i64()?, pct: p.get(1)?.as_f64()?, resets: p.get(2).and_then(Value::as_i64) })
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                h.windows.insert(key.clone(), points);
            }
        }
        h
    }

    pub(crate) fn to_json(&self) -> String {
        let windows: serde_json::Map<String, Value> = self
            .windows
            .iter()
            .map(|(k, points)| (k.clone(), Value::Array(points.iter().map(|p| json!([p.at, p.pct, p.resets])).collect())))
            .collect();
        json!({ "version": 1, "windows": windows }).to_string()
    }

    pub(crate) fn load() -> History {
        crate::daemon::read_file(FILE).map(|t| History::from_json(&t)).unwrap_or_default()
    }

    pub(crate) fn save(&self) {
        if let Err(e) = crate::daemon::write_file(FILE, &self.to_json()) {
            log::warn!("cannot save the usage history: {e}");
        }
    }

    /// Adds a reading of window `key`; false when it added nothing (the
    /// same reading again soon after).
    pub(crate) fn record(&mut self, key: &str, at: i64, pct: f64, resets: Option<i64>) -> bool {
        let points = self.windows.entry(key.to_owned()).or_default();
        if let Some(last) = points.last()
            && last.pct == pct
            && same_window(last.resets, resets)
            && at - last.at < SAME_READING_GAP
        {
            return false;
        }
        points.push(Point { at, pct, resets });
        points.retain(|p| at - p.at <= KEEP_SECS);
        if points.len() > MAX_POINTS {
            let extra = points.len() - MAX_POINTS;
            points.drain(..extra);
        }
        // Windows no longer reported fade out by age; past the cap, the
        // ones heard from least recently go.
        self.windows.retain(|_, p| !p.is_empty() && p.last().is_some_and(|l| at - l.at <= KEEP_SECS));
        while self.windows.len() > MAX_WINDOWS {
            let stale = self.windows.iter().min_by_key(|(_, p)| p.last().map_or(0, |l| l.at)).map(|(k, _)| k.clone());
            match stale {
                Some(k) => self.windows.remove(&k),
                None => break,
            };
        }
        true
    }
}

/// The trailing points of the window that resets at `resets`: since the
/// reset time last changed or the use last fell (a reset).
pub(crate) fn current_window(points: &[Point], resets: Option<i64>) -> &[Point] {
    let mut start = points.len();
    while start > 0 {
        let p = &points[start - 1];
        if !same_window(p.resets, resets) {
            break;
        }
        if start < points.len() && points[start].pct < p.pct {
            break;
        }
        start -= 1;
    }
    &points[start..]
}

/// How long a window lasts, when its id or label says (Claude's 5-hour
/// session and weekly windows, daily ones).
pub(crate) fn window_length(id: &str, label: &str) -> Option<i64> {
    let s = format!("{id} {label}").to_ascii_lowercase();
    if s.contains("five_hour") || s.contains("5h") || s.contains("5-hour") || s.contains("session") {
        Some(5 * 3600)
    } else if s.contains("week") || s.contains("seven_day") || s.contains("7d") {
        Some(7 * 86_400)
    } else if s.contains("daily") || s.contains("day") {
        Some(86_400)
    } else {
        None
    }
}

/// When the window runs out at the pace so far, if that is before it
/// resets. The pace is the use over the time since the window began, when
/// its length is known; else the change between the first and last of
/// `points` (the current window's, at least ten minutes apart).
pub(crate) fn forecast(points: &[Point], now: i64, pct: f64, resets: Option<i64>, length: Option<i64>) -> Option<i64> {
    let resets = resets?;
    if pct >= 100.0 || pct <= 0.0 || resets <= now {
        return None;
    }
    let from_start = length.map(|len| now - (resets - len)).filter(|elapsed| *elapsed >= 60).map(|elapsed| pct / elapsed as f64);
    let from_points = || {
        let (first, last) = (points.first()?, points.last()?);
        let span = last.at - first.at;
        let rise = last.pct - first.pct;
        (span >= 600 && rise > 0.0).then(|| rise / span as f64)
    };
    let rate = from_start.or_else(from_points)?;
    let out = now + ((100.0 - pct) / rate).ceil() as i64;
    (out < resets).then_some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const H: i64 = 3600;

    #[test]
    fn records_skip_repeats_and_round_trip() {
        let mut h = History::default();
        assert!(h.record("claude/five_hour", 1_000_000, 10.0, Some(1_010_000)));
        assert!(!h.record("claude/five_hour", 1_000_060, 10.0, Some(1_010_003)));
        assert!(h.record("claude/five_hour", 1_000_120, 12.0, Some(1_010_000)));
        assert!(h.record("claude/five_hour", 1_000_000 + 20 * 60, 12.0, Some(1_010_000)));
        assert_eq!(h.windows["claude/five_hour"].len(), 3);
        let back = History::from_json(&h.to_json());
        assert_eq!(back, h);
        // Old points go.
        h.record("claude/five_hour", 1_000_000 + KEEP_SECS + 2_000, 1.0, None);
        assert_eq!(h.windows["claude/five_hour"].len(), 1);
    }

    #[test]
    fn the_current_window_starts_at_the_last_reset() {
        let p = |at, pct, resets| Point { at, pct, resets: Some(resets) };
        let points = [p(0, 80.0, 100), p(10, 90.0, 100), p(20, 5.0, 5000), p(30, 9.0, 5000), p(40, 15.0, 5004)];
        assert_eq!(current_window(&points, Some(5000)).len(), 3);
        // A drop without a new reset time is a reset too.
        let dropped = [p(0, 50.0, 100), p(10, 2.0, 100), p(20, 4.0, 100)];
        assert_eq!(current_window(&dropped, Some(100)).len(), 2);
    }

    #[test]
    fn forecast_from_the_window_start() {
        // A 5-hour window, 2 hours in, 60% used: 30% an hour, so the rest
        // (40%) takes 80 minutes, before the reset 3 hours away.
        let now = 100 * H;
        let resets = now + 3 * H;
        assert_eq!(forecast(&[], now, 60.0, Some(resets), Some(5 * H)), Some(now + 80 * 60));
        // At 20% after 2 hours (10%/h) the reset comes first.
        assert_eq!(forecast(&[], now, 20.0, Some(resets), Some(5 * H)), None);
        // Used up, or nothing used: no forecast.
        assert_eq!(forecast(&[], now, 100.0, Some(resets), Some(5 * H)), None);
        assert_eq!(forecast(&[], now, 0.0, Some(resets), Some(5 * H)), None);
    }

    #[test]
    fn forecast_from_points_when_the_length_is_unknown() {
        let now = 100 * H;
        let resets = now + 10 * H;
        let points = [Point { at: now - 2 * H, pct: 40.0, resets: Some(resets) }, Point { at: now, pct: 60.0, resets: Some(resets) }];
        // 10% an hour: 40% more in 4 hours.
        assert_eq!(forecast(&points, now, 60.0, Some(resets), None), Some(now + 4 * H));
        // Too short a span says nothing.
        assert_eq!(forecast(&points[1..], now, 60.0, Some(resets), None), None);
        assert_eq!(window_length("five_hour", "Session"), Some(5 * H));
        assert_eq!(window_length("weekly_model_fable", "Weekly · Fable"), Some(7 * 24 * H));
        assert_eq!(window_length("unknown_primary", "Primary limit"), None);
    }
}
