//! Sophon's own log, kept: `~/.sophon/sophon.log` (the one before it,
//! `sophon.log.1`), what the `log` macros say at `info` and above, Sophon's
//! own lines, and warnings from the libraries. A Sophon opened from the
//! Finder has no terminal, so without it what went wrong (a service that
//! would not start, one that stopped) was said to nobody.
//!
//! With `RUST_LOG` set it also goes to the terminal, as `env_logger` did.
//! Keys and tokens are blanked as in `connection.log`.

use std::{
    io::Write,
    path::PathBuf,
    sync::{Mutex, OnceLock},
};

/// Past this, the log moves to `sophon.log.1` and starts again.
const MAX_BYTES: u64 = 1024 * 1024;

/// The log's file.
pub(crate) fn path() -> PathBuf {
    crate::daemon::data_dir().join("sophon.log")
}

struct Logger {
    file: Mutex<()>,
    terminal: bool,
}

static LOGGER: OnceLock<Logger> = OnceLock::new();

/// Starts keeping the log (once; later calls do nothing).
pub fn init() {
    let logger = LOGGER.get_or_init(|| Logger { file: Mutex::new(()), terminal: std::env::var_os("RUST_LOG").is_some() });
    if log::set_logger(logger).is_ok() {
        log::set_max_level(log::LevelFilter::Info);
    }
}

fn ours(target: &str) -> bool {
    target == "sophon" || target.starts_with("sophon::")
}

impl log::Log for Logger {
    fn enabled(&self, m: &log::Metadata) -> bool {
        m.level() <= if ours(m.target()) { log::Level::Info } else { log::Level::Warn }
    }

    fn log(&self, record: &log::Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let line = crate::daemon::connection::redact(&format!(
            "{} {:5} {}: {}",
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f"),
            record.level(),
            record.target().trim_start_matches("sophon::"),
            record.args()
        ));
        if self.terminal {
            eprintln!("{line}");
        }
        let _held = self.file.lock().unwrap_or_else(|e| e.into_inner());
        let path = path();
        if std::fs::metadata(&path).is_ok_and(|m| m.len() > MAX_BYTES) {
            let _ = std::fs::rename(&path, path.with_extension("log.1"));
        }
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
            let _ = writeln!(f, "{line}");
        }
    }

    fn flush(&self) {}
}

/// The last `n` lines of `path` (all of a shorter file).
pub(crate) fn tail(path: &std::path::Path, n: usize) -> Vec<String> {
    let Ok(text) = std::fs::read_to_string(path) else { return vec![] };
    let lines: Vec<&str> = text.lines().collect();
    lines[lines.len().saturating_sub(n)..].iter().map(|l| (*l).to_owned()).collect()
}

/// What the service's own log (`<home>/daemon.log`, JSON lines) says of
/// its starts and stops: the last `n` such lines, as `time message
/// (reason)`. Why a service went away is there (a signal, an update,
/// the supervisor), and nowhere in Sophon's.
pub(crate) fn service_lifecycle(home: &std::path::Path, n: usize) -> Vec<String> {
    const MARKS: [&str; 9] = [
        "shutdown",
        "Shutdown",
        "SIGTERM",
        "SIGINT",
        "Worker exited",
        "Starting daemon worker",
        "uncaught",
        "Uncaught",
        "heap",
    ];
    // The current file is enough for the last few; the one before it when
    // the current is new.
    let mut files = vec![home.join("daemon.log")];
    if let Ok(dir) = std::fs::read_dir(home) {
        let mut older: Vec<PathBuf> = dir
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.file_name().and_then(|f| f.to_str()).is_some_and(|f| f.ends_with("-daemon.log")))
            .collect();
        older.sort();
        if let Some(last) = older.pop() {
            files.insert(0, last);
        }
    }
    let mut out = Vec::new();
    for file in files {
        let Ok(text) = std::fs::read_to_string(&file) else { continue };
        for line in text.lines().filter(|l| MARKS.iter().any(|m| l.contains(m))) {
            let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else { continue };
            let Some(msg) = v.get("msg").and_then(serde_json::Value::as_str) else { continue };
            if !MARKS.iter().any(|m| msg.contains(m)) {
                continue;
            }
            let time = match v.get("time") {
                Some(serde_json::Value::Number(ms)) => ms
                    .as_i64()
                    .and_then(chrono::DateTime::from_timestamp_millis)
                    .map(|t| t.with_timezone(&chrono::Local).format("%Y-%m-%d %H:%M:%S").to_string())
                    .unwrap_or_default(),
                Some(serde_json::Value::String(s)) => chrono::DateTime::parse_from_rfc3339(s)
                    .map(|t| t.with_timezone(&chrono::Local).format("%Y-%m-%d %H:%M:%S").to_string())
                    .unwrap_or_else(|_| s.clone()),
                _ => String::new(),
            };
            let why = v.get("reason").or_else(|| v.get("signal")).and_then(serde_json::Value::as_str).map(|r| format!(" ({r})")).unwrap_or_default();
            out.push(format!("{time} {msg}{why}"));
        }
    }
    out.split_off(out.len().saturating_sub(n))
}

/// Everything to look at when the service misbehaves, as one text to
/// paste: versions, the service's starts and stops, and the ends of
/// Sophon's log and of the connection's. Nothing is sent anywhere: the
/// user copies it, reads it, and shares it if they choose.
pub(crate) fn diagnostics(service_home: Option<&std::path::Path>, versions: &[(String, String)]) -> String {
    let mut out = String::from("## Sophon diagnostics\n\n");
    for (k, v) in versions {
        out.push_str(&format!("- {k}: {v}\n"));
    }
    if let Some(home) = service_home {
        out.push_str(&format!("\n### Service starts and stops ({})\n```\n", home.display()));
        for l in service_lifecycle(home, 30) {
            out.push_str(&l);
            out.push('\n');
        }
        out.push_str("```\n");
    }
    out.push_str("\n### sophon.log\n```\n");
    for l in tail(&path(), 150) {
        out.push_str(&l);
        out.push('\n');
    }
    out.push_str("```\n\n### connection.log\n```\n");
    for l in tail(&crate::daemon::data_dir().join("connection.log"), 80) {
        out.push_str(&l);
        out.push('\n');
    }
    out.push_str("```\n");
    crate::daemon::connection::redact(&out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore]
    fn diagnostics_live() {
        // This Mac's own service log and Sophon's: what "Copy diagnostics" gives.
        let home = std::path::PathBuf::from(std::env::var("HOME").unwrap()).join(".paseo");
        println!("{}", diagnostics(Some(&home), &[("Sophon".into(), env!("CARGO_PKG_VERSION").into())]));
    }

    #[test]
    fn the_service_log_says_why_it_stopped() {
        let dir = std::env::temp_dir().join(format!("sophon-applog-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("daemon.log"),
            [
                r#"{"level":30,"time":1794794914220,"msg":"ws_runtime_metrics"}"#,
                r#"{"level":30,"time":"2026-11-16T02:08:59.274Z","msg":"Supervisor shutdown requested","reason":"supervisor_received_SIGTERM"}"#,
                r#"{"level":30,"time":1794794939274,"signal":"SIGTERM","msg":"SIGTERM received, shutting down gracefully..."}"#,
                "not json SIGTERM",
            ]
            .join("\n"),
        )
        .unwrap();
        let lines = service_lifecycle(&dir, 10);
        assert_eq!(lines.len(), 2, "{lines:?}");
        assert!(lines[0].ends_with("Supervisor shutdown requested (supervisor_received_SIGTERM)"));
        assert!(lines[1].contains("SIGTERM received") && lines[1].ends_with("(SIGTERM)"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
