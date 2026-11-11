//! Reads the session logs of this computer: Claude Code's
//! (`$CLAUDE_CONFIG_DIR/projects`, else `~/.claude/projects` and
//! `~/.config/claude/projects`) and Codex's (`$CODEX_HOME/sessions`, else
//! `~/.codex/sessions`, archived sessions too).
//!
//! Histories run to gigabytes, so each file is remembered by its size and
//! modification time: an unchanged file is not read again, one that grew
//! (logs only append) is read from where the last read stopped, and one
//! untouched for longer than the page looks back is skipped. Files are read
//! on a few threads at once; callers run this off the UI thread.

use std::{
    collections::HashMap,
    fs,
    io::{BufRead, BufReader, Seek, SeekFrom},
    path::{Path, PathBuf},
    sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use super::{
    Entry, Report, Source,
    parse::{CodexState, claude_line, codex_line},
};

/// How far back the page looks, plus a day of slack for time zones.
const LOOK_BACK: Duration = Duration::from_secs(31 * 86_400);

#[derive(Default)]
struct FileState {
    size: u64,
    modified: Option<SystemTime>,
    /// Where the next read starts: after the last complete line.
    consumed: u64,
    codex: CodexState,
    entries: Vec<Entry>,
}

static CACHE: Mutex<Option<HashMap<PathBuf, FileState>>> = Mutex::new(None);

fn home() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default()
}

pub(crate) fn roots() -> Vec<(Source, PathBuf)> {
    let mut out = vec![];
    match std::env::var("CLAUDE_CONFIG_DIR").ok().filter(|v| !v.trim().is_empty()) {
        // Claude Code takes a comma-separated list here.
        Some(dirs) => out.extend(dirs.split(',').map(|d| (Source::Claude, PathBuf::from(d.trim()).join("projects")))),
        None => {
            out.push((Source::Claude, home().join(".config/claude/projects")));
            out.push((Source::Claude, home().join(".claude/projects")));
        }
    }
    let codex = std::env::var_os("CODEX_HOME").filter(|v| !v.is_empty()).map(PathBuf::from).unwrap_or_else(|| home().join(".codex"));
    out.push((Source::Codex, codex.join("sessions")));
    out.push((Source::Codex, codex.join("archived_sessions")));
    let mut seen = std::collections::HashSet::new();
    out.retain(|(_, p)| p.is_dir() && seen.insert(fs::canonicalize(p).unwrap_or_else(|_| p.clone())));
    out
}

/// The `.jsonl` files under `dir` changed since `since`, with their size
/// and modification time.
pub(crate) fn walk(dir: &Path, source: Source, since: SystemTime, depth: u32, out: &mut Vec<(PathBuf, Source, u64, Option<SystemTime>)>) {
    let Ok(read) = fs::read_dir(dir) else { return };
    for entry in read.flatten() {
        let path = entry.path();
        // Follows links (a moved history), to a bounded depth against loops.
        let Ok(meta) = fs::metadata(&path) else { continue };
        if meta.is_dir() {
            if depth < 8 {
                walk(&path, source, since, depth + 1, out);
            }
        } else if path.extension().is_some_and(|e| e == "jsonl") {
            let modified = meta.modified().ok();
            if modified.is_none_or(|m| m >= since) {
                out.push((path, source, meta.len(), modified));
            }
        }
    }
}

/// Reads `path` from `state.consumed` on, adding what it finds.
fn read(path: &Path, source: Source, state: &mut FileState, oldest: i64) {
    let Ok(mut file) = fs::File::open(path) else { return };
    if file.seek(SeekFrom::Start(state.consumed)).is_err() {
        return;
    }
    let mut reader = BufReader::with_capacity(256 * 1024, file);
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match reader.read_until(b'\n', &mut buf) {
            Ok(0) | Err(_) => break,
            // A line still being written: read it whole next time.
            Ok(_) if buf.last() != Some(&b'\n') => break,
            Ok(n) => {
                state.consumed += n as u64;
                let Ok(line) = std::str::from_utf8(&buf) else { continue };
                let entry = match source {
                    Source::Claude => claude_line(line),
                    Source::Codex => codex_line(&mut state.codex, line),
                };
                if let Some(e) = entry.filter(|e| e.at >= oldest) {
                    state.entries.push(e);
                }
            }
        }
    }
}

/// Brings the remembered files up to date and sums them.
pub(crate) fn scan() -> Report {
    // Today's prices, before anything is summed with them.
    super::price::refresh();
    let now = super::now();
    let since = UNIX_EPOCH + Duration::from_secs(now.max(0) as u64) - LOOK_BACK;
    let oldest = now - LOOK_BACK.as_secs() as i64;
    let mut files = vec![];
    for (source, root) in roots() {
        walk(&root, source, since, 0, &mut files);
    }

    let mut guard = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    let cache = guard.get_or_insert_with(HashMap::new);
    let mut previous = std::mem::take(cache);
    // What each file needs: nothing, the rest of it, or all of it.
    let mut work: Vec<(PathBuf, Source, FileState)> = vec![];
    for (path, source, size, modified) in files {
        let mut state = previous.remove(&path).unwrap_or_default();
        if state.size == size && state.modified == modified && state.consumed > 0 {
            cache.insert(path, state);
            continue;
        }
        if size < state.consumed {
            // Shorter than before: rewritten, not appended to.
            state = FileState::default();
        }
        state.size = size;
        state.modified = modified;
        work.push((path, source, state));
    }

    let next = AtomicUsize::new(0);
    let slots: Vec<Mutex<Option<(PathBuf, FileState)>>> = work.iter().map(|_| Mutex::new(None)).collect();
    let work: Vec<Mutex<Option<(PathBuf, Source, FileState)>>> = work.into_iter().map(|w| Mutex::new(Some(w))).collect();
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get()).clamp(1, 8).min(work.len().max(1));
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| {
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some(job) = work.get(i) else { break };
                    let Some((path, source, mut state)) = job.lock().ok().and_then(|mut j| j.take()) else { continue };
                    read(&path, source, &mut state, oldest);
                    if let Ok(mut slot) = slots[i].lock() {
                        *slot = Some((path, state));
                    }
                }
            });
        }
    });
    for slot in slots {
        if let Some((path, state)) = slot.into_inner().ok().flatten() {
            cache.insert(path, state);
        }
    }

    let mut report = super::aggregate(cache.values().flat_map(|f| f.entries.iter()), now, &super::local_offset);
    report.files = cache.len();
    report
}

#[cfg(test)]
mod tests {
    use super::*;

    /// This computer's real logs: totals and timings only, no content.
    /// `cargo test --release --lib real_history -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn real_history() {
        let start = std::time::Instant::now();
        let r = scan();
        let first = start.elapsed();
        let again = std::time::Instant::now();
        let r2 = scan();
        println!("files {} · first {first:?} · again {:?}", r.files, again.elapsed());
        assert_eq!(r, r2);
        for (name, s) in [("today", &r.today), ("7 days", &r.week), ("30 days", &r.month)] {
            let k = s.totals.tokens;
            println!(
                "{name}: {} responses · in {} out {} read {} write {} · hit {:.1}% · ${:.2} · unpriced {}",
                s.totals.responses,
                k.input,
                k.output,
                k.cache_read,
                k.cache_write,
                k.cache_hit_rate().unwrap_or(0.0) * 100.0,
                s.totals.cost,
                s.totals.unpriced
            );
            for m in &s.models {
                println!("    {:?} {} {} tokens ${:.2}", m.source, m.model, m.totals.tokens.total(), m.totals.cost);
            }
        }
    }

    #[test]
    fn reads_appended_lines_once_and_waits_for_a_whole_line() {
        let dir = std::env::temp_dir().join(format!("sophon-usage-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("s.jsonl");
        let line = |id: &str, out: u64| {
            format!(
                r#"{{"type":"assistant","requestId":"r{id}","timestamp":"2026-11-10T02:00:00Z","message":{{"model":"claude-opus-5-5","id":"{id}","usage":{{"input_tokens":1,"output_tokens":{out}}}}}}}"#
            )
        };
        fs::write(&path, format!("{}\n{}", line("a", 5), &line("b", 7)[..20])).unwrap();
        let mut st = FileState::default();
        read(&path, Source::Claude, &mut st, 0);
        assert_eq!(st.entries.len(), 1);
        // The half-written line is finished, and another appended.
        fs::write(&path, format!("{}\n{}\n{}\n", line("a", 5), line("b", 7), line("c", 9))).unwrap();
        read(&path, Source::Claude, &mut st, 0);
        assert_eq!(st.entries.iter().map(|e| e.tokens.output).collect::<Vec<_>>(), [5, 7, 9]);
        let _ = fs::remove_dir_all(&dir);
    }
}
