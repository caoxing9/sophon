//! A tool call's detail, laid out by what the call was (the tool
//! detail types): a command and its output, a file and its diff, a search
//! and what it found, a page and what it returned. The desktop shows it
//! under the call's row, the phone in a sheet; both use this.

use nsh_core::{element::FontWeight, prelude::*};
use serde_json::Value;

use crate::i18n::{t, tf};
use crate::ui::kit::text;
use crate::ui::kit::*;

/// Long outputs are cut here: a log of megabytes would only slow the
/// layout, and the agent has the whole of it.
const MAX_CHARS: usize = 20_000;

/// The detail of a call of `kind`, `error` (what failed) under it.
pub(crate) fn tool_body(kind: &str, detail: &Value, error: Option<&str>) -> View {
    let field = |k: &str| detail.get(k).and_then(Value::as_str).filter(|s| !s.is_empty()).map(str::to_owned);
    let mut body = view().gap(px(S2)).min_w(px(0.0));
    match kind {
        "shell" => {
            if let Some(cmd) = field("command") {
                body = body.child(code(format!("$ {cmd}"), 160.0));
            }
            if let Some(out) = field("output") {
                body = body.child(label(t("Output"))).child(code(out, 320.0));
            }
            match detail.get("exitCode").and_then(Value::as_i64) {
                Some(0) | None => {}
                Some(code) => body = body.child(badge(tf("Exit code {}", &[&code.to_string()]), DANGER)),
            }
        }
        "read" => {
            if let Some(path) = field("filePath") {
                body = body.child(path_row(Icon::FileText, path));
            }
            if let Some(content) = field("content") {
                body = body.child(code(content, 240.0));
            }
        }
        "edit" => {
            if let Some(path) = field("filePath") {
                body = body.child(path_row(Icon::Pencil, path));
            }
            if let Some(diff) = field("unifiedDiff") {
                body = body.child(diff_block(diff.lines().map(str::to_owned).collect()));
            } else {
                let mut lines = vec![];
                lines.extend(field("oldString").unwrap_or_default().lines().map(|l| format!("-{l}")));
                lines.extend(field("newString").unwrap_or_default().lines().map(|l| format!("+{l}")));
                if !lines.is_empty() {
                    body = body.child(diff_block(lines));
                }
            }
        }
        "write" => {
            if let Some(path) = field("filePath") {
                body = body.child(path_row(Icon::FileText, path));
            }
            if let Some(content) = field("content") {
                body = body.child(code(content, 240.0));
            }
        }
        "search" => {
            if let Some(q) = field("query") {
                body = body.child(code(q, 80.0));
            }
            let files: Vec<String> = detail
                .get("filePaths")
                .and_then(Value::as_array)
                .map(|a| a.iter().filter_map(Value::as_str).map(str::to_owned).collect())
                .unwrap_or_default();
            let webs: Vec<(String, String)> = detail
                .get("webResults")
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .filter_map(|r| Some((r.get("title")?.as_str()?.to_owned(), r.get("url")?.as_str()?.to_owned())))
                        .collect()
                })
                .unwrap_or_default();
            if !files.is_empty() {
                body = body.child(label(t("Results"))).child(code(files.join("\n"), 200.0));
            }
            if !webs.is_empty() {
                let mut list = view().gap(px(6.0));
                for (title, url) in webs {
                    list = list.child(
                        view()
                            .gap(px(1.0))
                            .child(text(title).text_size(px(TEXT_SM)).text_color(FG).number_of_lines(1))
                            .child(text(url).text_size(px(TEXT_XS)).font_family(mono_font).text_color(FG_MUTED).number_of_lines(1)),
                    );
                }
                body = body.child(label(t("Results"))).child(list);
            }
            if files.is_empty()
                && let Some(content) = field("content")
            {
                body = body.child(code(content, 240.0));
            }
        }
        "fetch" => {
            if let Some(url) = field("url") {
                body = body.child(path_row(Icon::Globe, url));
            }
            if let Some(result) = field("result") {
                body = body.child(code(result, 240.0));
            }
        }
        "sub_agent" | "plain_text" => {
            if let Some(s) = field("description").or_else(|| field("text")).or_else(|| field("title")) {
                body = body.child(text(s).text_size(px(TEXT_BASE)).line_height(px(20.0)).text_color(FG).selectable(true));
            }
        }
        _ => {
            if !detail.is_null() {
                body = body.child(code(serde_json::to_string_pretty(detail).unwrap_or_default(), 280.0));
            }
        }
    }
    if let Some(e) = error {
        body = body.child(
            view()
                .flex_row()
                .gap(px(S2))
                .px(px(S3))
                .py(px(S2))
                .rounded(px(R_LG))
                .bg(tint(DANGER, 0.08))
                .child(icon(Icon::CircleAlert, 15.0).text_color(DANGER))
                .child(text(e.to_owned()).text_size(px(TEXT_SM)).text_color(DANGER).selectable(true).flex_1()),
        );
    }
    body
}

/// Whether a call has anything to show beyond its row.
pub(crate) fn has_body(detail: &Value, error: Option<&str>) -> bool {
    error.is_some() || detail.as_object().is_some_and(|o| o.iter().any(|(k, v)| k != "type" && !v.is_null()))
}

fn clip(s: String) -> String {
    if s.len() <= MAX_CHARS {
        return s;
    }
    let mut end = MAX_CHARS;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}\n…", &s[..end])
}

fn label(s: &'static str) -> impl IntoElement {
    text(s).text_size(px(TEXT_XS)).font_weight(FontWeight(600)).text_color(FG_MUTED).mt(px(2.0))
}

fn badge(s: String, tone: AdaptiveColor) -> impl IntoElement {
    view()
        .flex_row()
        .self_start()
        .px(px(S2))
        .py(px(2.0))
        .rounded(px(R_MD))
        .bg(tint(tone, 0.1))
        .child(text(s).text_size(px(TEXT_XS)).font_weight(FontWeight(600)).text_color(tone))
}

fn path_row(glyph: Icon, path: String) -> impl IntoElement {
    view()
        .flex_row()
        .items_center()
        .gap(px(6.0))
        .min_w(px(0.0))
        .child(icon(glyph, 14.0).text_color(FG_MUTED))
        .child(text(path).text_size(px(TEXT_CODE)).font_family(mono_font).text_color(FG).selectable(true).flex_1())
}

/// Monospaced text on a soft panel, scrolling past `max_h`.
fn code(s: String, max_h: f32) -> impl IntoElement {
    scroll().max_h(px(max_h)).rounded(px(R_LG)).bg(SURFACE2).child(
        text(clip(s))
            .px(px(S3))
            .py(px(S2))
            .text_size(px(TEXT_CODE))
            .line_height(px(18.0))
            .font_family(mono_font)
            .text_color(FG)
            .selectable(true),
    )
}

/// A diff's lines, additions and deletions tinted.
fn diff_block(lines: Vec<String>) -> impl IntoElement {
    let mut col = view().py(px(S1));
    let mut chars = 0;
    for line in lines {
        chars += line.len();
        if chars > MAX_CHARS {
            col = col.child(text("…").px(px(S3)).text_size(px(TEXT_CODE)).text_color(FG_MUTED));
            break;
        }
        let (bg, fg) = if line.starts_with('+') && !line.starts_with("+++") {
            (tint(SUCCESS, 0.12), SUCCESS)
        } else if line.starts_with('-') && !line.starts_with("---") {
            (tint(DANGER, 0.12), DANGER)
        } else if line.starts_with("@@") {
            (tint(FG, 0.0), FG_MUTED)
        } else {
            (tint(FG, 0.0), FG)
        };
        col = col.child(
            text(if line.is_empty() { " ".to_owned() } else { line })
                .px(px(S3))
                .bg(bg)
                .text_size(px(TEXT_CODE))
                .line_height(px(18.0))
                .font_family(mono_font)
                .text_color(fg)
                .selectable(true),
        );
    }
    scroll().max_h(px(320.0)).rounded(px(R_LG)).bg(SURFACE2).overflow_hidden().child(col)
}
