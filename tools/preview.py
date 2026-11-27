#!/usr/bin/env python3
"""Writes site/index.html: the docs site. Every component runs live in the
page: the gallery's web build (`nsh package --target web`, in site/demo/)
opens at the component in an iframe, with how to use it and where NSH differs
from the browser beside it.

The components and their order come from the gallery's pages
(examples/gallery/src/pages/<family>.rs). `tools/site.sh` builds the whole
site; serve site/ (GitHub Pages, or `python3 -m http.server -d site`).

    python3 tools/preview.py
"""

import html
import json
import os
import re

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(os.path.join(HERE, ".."))
REPO = "https://github.com/caoxing9/nsh-shadcn"

FAMILIES = [
    ("button", "Actions"),
    ("basics", "Basics"),
    ("forms", "Forms"),
    ("overlays", "Overlays"),
    ("menus", "Menus"),
    ("layout", "Layout"),
    ("chat", "Chat"),
]


def pages(family):
    src = open(os.path.join(ROOT, "examples/gallery/src/pages", f"{family}.rs")).read()
    return re.findall(r'id:\s*"([a-z0-9-]+)",\s*title:\s*"([^"]+)"', src)


def page_function(src, page_id):
    """The function that draws the page `page_id` in a gallery file: the one its
    `body:` names (a closure's inner call when it wraps it, `|| body(badge())`)."""
    m = re.search(r'id:\s*"%s",\s*title:\s*"[^"]+",\s*body(?::\s*([^}\n]+))?' % re.escape(page_id), src)
    if not m:
        return None
    if m.group(1) is None:  # `Page { id, title, body }`: the function is `body`
        m = re.match(r"(.*)", "body")
    names = re.findall(r"([a-z_][a-z0-9_]*)\s*(?=\(|,|$|\s*\})", m.group(1).strip().rstrip(","))
    defined = [n for n in names if re.search(r"\bfn %s\s*\(" % n, src)]
    if not defined:
        return None
    name = next((n for n in reversed(defined) if n != "body"), defined[-1])
    start = re.search(r"(?m)^(?:pub )?fn %s\s*\(" % name, src)
    if not start:
        return None
    # The doc comments above it, then the function to its closing brace.
    begin = start.start()
    lines_before = src[:begin].split("\n")
    docs = []
    for line in reversed(lines_before[:-1]):
        if line.startswith("///"):
            docs.insert(0, line)
        else:
            break
    depth, i, opened = 0, src.index("{", start.end()), False
    while i < len(src):
        if src[i] == "{":
            depth += 1
            opened = True
        elif src[i] == "}":
            depth -= 1
            if opened and depth == 0:
                break
        i += 1
    return "\n".join(docs + [src[begin:i + 1]])


def source_file(family, page_id):
    """The component's Rust file, if it has one of its own."""
    name = page_id.replace("-", "_")
    for path in (f"src/ui/{family}/{name}.rs", f"src/ui/{name}.rs"):
        if os.path.exists(os.path.join(ROOT, path)):
            return path
    return f"src/ui/{family}/mod.rs" if family != "button" else "src/ui/button.rs"


def adaptations(path):
    """The module doc's list of where NSH differs from shadcn (its `//! - ` items)."""
    items, current = [], None
    for line in open(os.path.join(ROOT, path)):
        if not line.startswith("//!"):
            if items or current:
                break
            continue
        text = line[3:].rstrip("\n")
        if text.startswith(" - "):
            if current:
                items.append(current)
            current = text[3:].strip()
        elif current is not None and text.startswith("   "):
            current += " " + text.strip()
        elif current is not None:
            items.append(current)
            current = None
    if current:
        items.append(current)
    return items


def main():
    data = []
    for family, label in FAMILIES:
        gallery = open(os.path.join(ROOT, "examples/gallery/src/pages", f"{family}.rs")).read()
        for page_id, title in pages(family):
            path = source_file(family, page_id)
            data.append({
                "id": page_id,
                "title": title,
                "family": label,
                "source": f"{REPO}/blob/main/{path}",
                "shadcn": f"https://ui.shadcn.com/docs/components/{page_id}",
                "notes": adaptations(path),
                "usage": page_function(gallery, page_id),
                "gallery": f"{REPO}/blob/main/examples/gallery/src/pages/{family}.rs",
            })
    out = os.path.join(ROOT, "site")
    os.makedirs(out, exist_ok=True)
    app_id = json.load(open(os.path.join(ROOT, "examples/gallery/nsh.json")))["id"]
    page = (TEMPLATE.replace("__DATA__", json.dumps(data, ensure_ascii=False))
            .replace("__REPO__", html.escape(REPO))
            .replace("__PREFIX__", f"nsh:{app_id}:"))
    with open(os.path.join(out, "index.html"), "w") as f:
        f.write(page)
    print(f"{os.path.join(out, 'index.html')}: {len(data)} components")


TEMPLATE = r"""<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>nsh-shadcn — components</title>
<style>
:root {
  --background: #fff; --foreground: #0a0a0a; --muted: #f5f5f5; --muted-foreground: #737373;
  --border: #e5e5e5; --accent: #f5f5f5; --primary: #171717; --primary-foreground: #fafafa;
  --frame: #fafafa; color-scheme: light;
}
.dark { color-scheme: dark;
  --background: #0a0a0a; --foreground: #fafafa; --muted: #262626; --muted-foreground: #a1a1a1;
  --border: #ffffff1a; --accent: #262626; --primary: #e5e5e5; --primary-foreground: #171717;
  --frame: #171717;
}
* { box-sizing: border-box; }
html, body { margin: 0; height: 100%; }
body {
  font: 14px/1.5 -apple-system, BlinkMacSystemFont, "Segoe UI", system-ui, sans-serif;
  background: var(--background); color: var(--foreground); -webkit-font-smoothing: antialiased;
  display: grid; grid-template-columns: 248px 1fr; grid-template-rows: 56px 1fr;
}
a { color: inherit; }
header {
  grid-column: 1 / -1; display: flex; align-items: center; gap: 12px; padding: 0 20px;
  border-bottom: 1px solid var(--border);
}
header .brand { font-weight: 600; font-size: 15px; }
header .brand span { color: var(--muted-foreground); font-weight: 400; margin-left: 8px; }
header .spacer { flex: 1; }
.btn {
  display: inline-flex; align-items: center; gap: 6px; height: 32px; padding: 0 12px;
  border-radius: 8px; border: 1px solid var(--border); background: var(--background);
  color: var(--foreground); font: inherit; font-weight: 500; cursor: pointer; text-decoration: none;
}
.btn:hover { background: var(--accent); }
.btn[aria-pressed="true"] { background: var(--primary); color: var(--primary-foreground); border-color: var(--primary); }
nav { border-right: 1px solid var(--border); overflow-y: auto; padding: 16px 12px 32px; }
nav input {
  width: 100%; height: 34px; padding: 0 10px; border-radius: 8px; border: 1px solid var(--border);
  background: var(--background); color: var(--foreground); font: inherit; margin-bottom: 12px; outline: none;
}
nav input:focus { border-color: var(--muted-foreground); }
nav h4 { margin: 16px 8px 6px; font-size: 12px; font-weight: 500; color: var(--muted-foreground); }
nav a {
  display: block; padding: 5px 8px; border-radius: 6px; text-decoration: none; font-size: 14px;
}
nav a:hover { background: var(--accent); }
nav a.active { background: var(--accent); font-weight: 500; }
main { overflow-y: auto; padding: 28px 36px 56px; }
main h1 { margin: 0; font-size: 30px; font-weight: 700; letter-spacing: -0.02em; }
main .family { color: var(--muted-foreground); font-size: 14px; margin-bottom: 4px; }
main .links { display: flex; gap: 8px; margin: 14px 0 22px; flex-wrap: wrap; }
.live { position: relative; max-width: 1100px; }
.live iframe {
  display: block; width: 100%; height: 640px; border: 1px solid var(--border); border-radius: 12px;
  background: var(--frame);
}
.live .loading {
  position: absolute; inset: 1px; display: flex; align-items: center; justify-content: center;
  color: var(--muted-foreground); border-radius: 12px; background: var(--frame); pointer-events: none;
  transition: opacity .2s;
}
.live .loading.done { opacity: 0; }
.live p { font-size: 13px; color: var(--muted-foreground); margin: 8px 0 0; }
.notes { margin-top: 28px; max-width: 880px; }
.notes h2 { font-size: 16px; margin: 0 0 8px; }
.notes p { color: var(--muted-foreground); margin: 0 0 8px; }
.notes ul { margin: 0; padding-left: 20px; color: var(--muted-foreground); }
.notes li { margin: 4px 0; }
code { font: 12.5px/1.4 ui-monospace, SFMono-Regular, Menlo, monospace; background: var(--muted); padding: 1px 5px; border-radius: 4px; }
.hint { color: var(--muted-foreground); font-size: 13px; }
.usage { margin-top: 28px; max-width: 1100px; }
.usage h2, .doc h2 { font-size: 18px; margin: 28px 0 8px; letter-spacing: -0.01em; }
.usage .bar { display: flex; align-items: center; gap: 8px; margin-bottom: 8px; }
.usage .bar p { margin: 0; flex: 1; color: var(--muted-foreground); }
pre {
  margin: 0; padding: 14px 16px; border-radius: 10px; border: 1px solid var(--border); background: var(--muted);
  overflow: auto; font: 12.5px/1.55 ui-monospace, SFMono-Regular, Menlo, monospace; max-height: 560px;
}
pre code { background: none; padding: 0; }
.doc { max-width: 880px; }
.doc p, .doc li { color: var(--muted-foreground); }
.doc p code, .doc li code { color: var(--foreground); }
.doc pre { margin: 8px 0 16px; }
.kw { color: #a626a4; } .dark .kw { color: #c678dd; }
.st { color: #50a14f; } .dark .st { color: #98c379; }
.cm { color: #a0a1a7; font-style: italic; } .dark .cm { color: #7f848e; }
@media (max-width: 900px) {
  body { grid-template-columns: 1fr; grid-template-rows: 56px auto 1fr; }
  nav { border-right: 0; border-bottom: 1px solid var(--border); max-height: 40vh; }
}
</style>
</head>
<body>
<header>
  <div class="brand">nsh-shadcn<span>shadcn/ui for NSH, in Rust</span></div>
  <div class="spacer"></div>
  <span class="hint">← → to move</span>
  <button class="btn" id="theme">Dark</button>
  <a class="btn" href="__REPO__">GitHub</a>
</header>
<nav>
  <input id="search" placeholder="Search components…" autocomplete="off">
  <div id="list"></div>
</nav>
<main id="main"></main>
<script>
const DATA = __DATA__;
const state = { dark: false, id: null };
const PREFIX = "__PREFIX__";
const $ = (s) => document.querySelector(s);
const esc = (s) => s.replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[c]));
const code = (s) => esc(s).replace(/`([^`]+)`/g, "<code>$1</code>");

function renderList(filter = "") {
  const q = filter.trim().toLowerCase();
  let html = q ? "" : `<h4>Getting started</h4><a href="#getting-started" class="${state.id === "getting-started" ? "active" : ""}">Introduction</a>`;
  let family = null;
  for (const c of DATA) {
    if (q && !c.title.toLowerCase().includes(q) && !c.id.includes(q)) continue;
    if (c.family !== family) { family = c.family; html += `<h4>${family}</h4>`; }
    html += `<a href="#${c.id}" class="${c.id === state.id ? "active" : ""}">${esc(c.title)}</a>`;
  }
  $("#list").innerHTML = html || `<p class="hint" style="padding:0 8px">No match.</p>`;
}

function highlight(src) {
  return esc(src)
    .replace(/(\/\/[^\n]*)/g, '<span class="cm">$1</span>')
    .replace(/(&quot;(?:[^&]|&(?!quot;))*?&quot;)/g, '<span class="st">$1</span>')
    .replace(/\b(fn|let|move|use|pub|impl|for|in|if|else|match|mut|return|struct|enum|const|as)\b/g, '<span class="kw">$1</span>');
}

const START = `<div class="family">Getting started</div><h1>nsh-shadcn</h1>
<div class="doc">
<p>shadcn/ui for <a href="https://nsh.dev">NSH (Native Shell)</a>, in Rust: every component of shadcn's new-york-v4
registry with the same variants, sizes and look, drawn with native views. It is an
<a href="https://nsh.dev/docs/nshm">nshm</a>: one crate that Rust apps call and TSX apps import. Every component on
this site is live: NSH's web build of the gallery, running in the page — press, type and open things.</p>
<h2>Install</h2>
<p>A Rust app adds the crate:</p>
<pre><code>cargo add nsh-shadcn</code></pre>
<p>A TSX app (or any NSH app) installs it with the NSH CLI 0.7:</p>
<pre><code>nsh add nsh-shadcn</code></pre>
<h2>Use it from Rust</h2>
<pre><code>${highlight(`use nsh_core::prelude::*;
use nsh_shadcn::{self as ui, IconName};

#[component]
fn Toolbar() -> impl IntoElement {
    view()
        .flex_row()
        .gap_2()
        .child(ui::button("Save").on_press(|| save()))
        .child(ui::button("Cancel").outline())
        .child(ui::button("Delete").destructive().sm())
        .child(ui::icon_button(IconName::Plus, "Add").ghost())
}`)}</code></pre>
<p>Each component has a constructor taking what it needs (<code>button("Save")</code>, <code>badge("New")</code>,
<code>card()</code>) and setters named as shadcn's props (<code>.variant(..)</code>, <code>.size(..)</code>,
<code>.disabled(..)</code>), with shorthands for the variants (<code>.outline()</code>, <code>.sm()</code>).
Restyle one as you would with <code>className</code>: <code>.app_class("w-full")</code> — its classes come last, so
they win. Every page shows the Rust that draws it under <b>Usage</b>.</p>
<h2>Use it from TSX</h2>
<pre><code>${highlight(`import { Button } from "nsh-shadcn";

export default function Toolbar() {
  return <Button label="Save" variant="outline" size="sm" onPress={save} />;
}`)}</code></pre>
<p>Components keep their Rust names; props are camelCase.</p>
<h2>Theme</h2>
<p>The components draw on the app's theme, whose tokens are shadcn's (<code>background</code>, <code>primary</code>,
<code>muted-foreground</code>, <code>radius</code>…). NSH's default theme is shadcn's neutral; set
<code>"theme"</code> in <code>nsh.json</code> to a file of shadcn <code>cssVars</code> — from shadcn's themes or
tweakcn — for another look. Light and dark follow the system.</p>
<h2>Differences from the browser</h2>
<p>A component uses shadcn's own class strings. Where NSH has no equivalent (descendant selectors, <code>calc()</code>,
tw-animate-css…), the component's page lists what it does instead, under <b>Where NSH differs</b>.</p>
</div>`;

function renderMain() {
  if (state.id === "getting-started") {
    $("#main").innerHTML = START;
    $("#main").scrollTop = 0;
    document.title = "nsh-shadcn";
    return;
  }
  const c = DATA.find((d) => d.id === state.id) || DATA[0];
  const live = `<div class="live"><iframe id="demo" title="${esc(c.title)} (live)"></iframe>
    <div class="loading" id="loading">Loading the live component…</div>
    <p>Live: the component drawn by NSH's web runtime. The same Rust draws it natively on macOS, iOS, Android, Windows and Linux.</p></div>`;
  const notes = c.notes.length ? `<div class="notes"><h2>Where NSH differs</h2>
    <p>The component uses shadcn's own classes; these are the places NSH needs something else.</p>
    <ul>${c.notes.map((n) => `<li>${code(n)}</li>`).join("")}</ul></div>` : "";
  const usage = c.usage ? `<div class="usage"><h2>Usage</h2><div class="bar">
    <p>The Rust that draws this page (<code>use nsh_shadcn as ui;</code>).</p>
    <button class="btn" id="copy">Copy</button><a class="btn" href="${c.gallery}">Gallery source</a></div>
    <pre><code>${highlight(c.usage)}</code></pre></div>` : "";
  $("#main").innerHTML = `<div class="family">${c.family}</div><h1>${esc(c.title)}</h1>
    <div class="links"><a class="btn" href="${c.source}">Component source</a><a class="btn" href="${c.shadcn}">shadcn/ui docs</a></div>
    ${live}${usage}${notes}`;
  loadDemo(c.id);
  const copy = $("#copy");
  if (copy) copy.onclick = () => navigator.clipboard.writeText(c.usage).then(() => {
    copy.textContent = "Copied";
    setTimeout(() => (copy.textContent = "Copy"), 1200);
  });
  $("#main").scrollTop = 0;
  document.title = `${c.title} — nsh-shadcn`;
}

function loadDemo(id) {
  // The gallery reads these when it starts (examples/gallery/src/lib.rs).
  localStorage.setItem(PREFIX + "page", id);
  localStorage.setItem(PREFIX + "bare", "1");
  localStorage.setItem(PREFIX + "scheme", state.dark ? "dark" : "light");
  const frame = $("#demo"), loading = $("#loading");
  frame.onload = () => setTimeout(() => loading.classList.add("done"), 1200);
  frame.src = "demo/index.html?" + id + "-" + (state.dark ? "dark" : "light");
}

function route() {
  state.id = location.hash.slice(1) || "getting-started";
  renderList($("#search").value);
  renderMain();
  document.querySelector("nav a.active")?.scrollIntoView({ block: "nearest" });
}

$("#theme").onclick = () => {
  state.dark = !state.dark;
  document.documentElement.classList.toggle("dark", state.dark);
  $("#theme").textContent = state.dark ? "Light" : "Dark";
  renderMain();
};
$("#search").oninput = (e) => renderList(e.target.value);
document.addEventListener("keydown", (e) => {
  if (e.target.tagName === "INPUT") return;
  const ids = ["getting-started", ...DATA.map((d) => d.id)];
  const i = ids.indexOf(state.id);
  if (e.key === "ArrowDown" || e.key === "ArrowRight") location.hash = ids[Math.min(i + 1, ids.length - 1)];
  if (e.key === "ArrowUp" || e.key === "ArrowLeft") location.hash = ids[Math.max(i - 1, 0)];
});
if (matchMedia("(prefers-color-scheme: dark)").matches) $("#theme").click();
window.addEventListener("hashchange", route);
route();
</script>
</body>
</html>
"""

if __name__ == "__main__":
    main()
