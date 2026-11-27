#!/usr/bin/env python3
"""Writes reference/<page>.html: a gallery page drawn by shadcn/ui itself.

A page's body is `tools/reference/<page>.html`: markup with shadcn's exact
classes (copied from its `new-york-v4/ui/*.tsx`), as the gallery's page lays
the component out. Its first line names the title: `<!-- title: Button -->`.
`<i data-lucide="send" class="size-4"></i>` is an icon, drawn from the same
SVG as `src/icon.rs` (size-4 when no size is given, as shadcn's buttons do).

The page wraps the body as the gallery does: an 880 x 640 content area with
`p-8 gap-8`, the title and an outline "Toggle theme" button, on shadcn's
neutral theme and its base layer, with Tailwind 4 in the browser. Open it
with `#dark` for the dark theme. Screenshot it at 880 x 640, device scale 2,
as reference/<page>-{light,dark}.png.

    python3 tools/reference.py button     # one page
    python3 tools/reference.py            # every page
"""

import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..")
BODIES = os.path.join(HERE, "reference")
OUT = os.path.join(ROOT, "reference")

LIGHT = {
    "background": "oklch(1 0 0)", "foreground": "oklch(0.145 0 0)",
    "card": "oklch(1 0 0)", "card-foreground": "oklch(0.145 0 0)",
    "popover": "oklch(1 0 0)", "popover-foreground": "oklch(0.145 0 0)",
    "primary": "oklch(0.205 0 0)", "primary-foreground": "oklch(0.985 0 0)",
    "secondary": "oklch(0.97 0 0)", "secondary-foreground": "oklch(0.205 0 0)",
    "muted": "oklch(0.97 0 0)", "muted-foreground": "oklch(0.556 0 0)",
    "accent": "oklch(0.97 0 0)", "accent-foreground": "oklch(0.205 0 0)",
    "destructive": "oklch(0.577 0.245 27.325)", "border": "oklch(0.922 0 0)",
    "input": "oklch(0.922 0 0)", "ring": "oklch(0.708 0 0)",
    "chart-1": "oklch(0.87 0 0)", "chart-2": "oklch(0.556 0 0)", "chart-3": "oklch(0.439 0 0)",
    "chart-4": "oklch(0.371 0 0)", "chart-5": "oklch(0.269 0 0)",
    "sidebar": "oklch(0.985 0 0)", "sidebar-foreground": "oklch(0.145 0 0)",
    "sidebar-primary": "oklch(0.205 0 0)", "sidebar-primary-foreground": "oklch(0.985 0 0)",
    "sidebar-accent": "oklch(0.97 0 0)", "sidebar-accent-foreground": "oklch(0.205 0 0)",
    "sidebar-border": "oklch(0.922 0 0)", "sidebar-ring": "oklch(0.708 0 0)",
}
DARK = {
    "background": "oklch(0.145 0 0)", "foreground": "oklch(0.985 0 0)",
    "card": "oklch(0.205 0 0)", "card-foreground": "oklch(0.985 0 0)",
    "popover": "oklch(0.205 0 0)", "popover-foreground": "oklch(0.985 0 0)",
    "primary": "oklch(0.922 0 0)", "primary-foreground": "oklch(0.205 0 0)",
    "secondary": "oklch(0.269 0 0)", "secondary-foreground": "oklch(0.985 0 0)",
    "muted": "oklch(0.269 0 0)", "muted-foreground": "oklch(0.708 0 0)",
    "accent": "oklch(0.269 0 0)", "accent-foreground": "oklch(0.985 0 0)",
    "destructive": "oklch(0.704 0.191 22.216)", "border": "oklch(1 0 0 / 10%)",
    "input": "oklch(1 0 0 / 15%)", "ring": "oklch(0.556 0 0)",
    "chart-1": "oklch(0.87 0 0)", "chart-2": "oklch(0.556 0 0)", "chart-3": "oklch(0.439 0 0)",
    "chart-4": "oklch(0.371 0 0)", "chart-5": "oklch(0.269 0 0)",
    "sidebar": "oklch(0.205 0 0)", "sidebar-foreground": "oklch(0.985 0 0)",
    "sidebar-primary": "oklch(0.488 0.243 264.376)", "sidebar-primary-foreground": "oklch(0.985 0 0)",
    "sidebar-accent": "oklch(0.269 0 0)", "sidebar-accent-foreground": "oklch(0.985 0 0)",
    "sidebar-border": "oklch(1 0 0 / 10%)", "sidebar-ring": "oklch(0.556 0 0)",
}

# The "Toggle theme" button of the header: shadcn's outline, sm.
TOGGLE = (
    '<button class="inline-flex shrink-0 items-center justify-center gap-2 rounded-md text-sm '
    "font-medium whitespace-nowrap transition-all outline-none border bg-background shadow-xs "
    "dark:border-input dark:bg-input/30 h-8 gap-1.5 px-3\">Toggle theme</button>"
)


def icons():
    """Lucide bodies by name, from src/icon.rs (the glyphs the crate draws)."""
    src = open(os.path.join(ROOT, "src", "icon.rs")).read()
    return dict(re.findall(r'=> "([a-z0-9-]+)", r#"(.*?)"#;', src))


def inline_icons(markup, bodies):
    def svg(m):
        attrs = m.group(1)
        name = re.search(r'data-lucide="([a-z0-9-]+)"', attrs).group(1)
        cls = re.search(r'class="([^"]*)"', attrs)
        cls = cls.group(1) if cls else "size-4"
        if "size-" not in cls:
            cls += " size-4"
        return (
            f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" '
            f'stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" '
            f'class="shrink-0 {cls}">{bodies[name]}</svg>'
        )
    return re.sub(r"<i ([^>]*data-lucide[^>]*)></i>", svg, markup)


def page(name, bodies):
    raw = open(os.path.join(BODIES, f"{name}.html")).read()
    title = re.match(r"<!--\s*title:\s*(.*?)\s*-->", raw)
    title = title.group(1) if title else name.replace("-", " ").title()
    body = inline_icons(raw, bodies)
    css_vars = lambda d: "".join(f"--{k}:{v};" for k, v in d.items())
    inline = "".join(f"--color-{k}: var(--{k});" for k in LIGHT)
    return f"""<!doctype html><html><head><meta charset="utf-8">
<script src="https://cdn.jsdelivr.net/npm/@tailwindcss/browser@4"></script>
<style>:root{{--radius:0.625rem;{css_vars(LIGHT)}}} .dark{{{css_vars(DARK)}}}</style>
<style type="text/tailwindcss">
@custom-variant dark (&:is(.dark *));
@theme inline {{ {inline} --radius-sm: calc(var(--radius) - 4px); --radius-md: calc(var(--radius) - 2px); --radius-lg: var(--radius); --radius-xl: calc(var(--radius) + 4px); }}
@layer base {{ * {{ @apply border-border outline-ring/50; }} body {{ @apply bg-background text-foreground; }} }}
</style>
<style>html,body{{margin:0;font-family:-apple-system,BlinkMacSystemFont,system-ui,sans-serif;-webkit-font-smoothing:antialiased}}</style>
</head><body>
<div style="width:880px;height:640px;overflow:hidden" class="relative box-border flex flex-col gap-8 bg-background p-8 text-foreground">
<div class="flex flex-row items-center justify-between"><div class="text-2xl font-semibold">{title}</div>{TOGGLE}</div>
{body}
</div>
<script>document.documentElement.classList.toggle('dark', location.hash === '#dark')</script>
</body></html>"""


if __name__ == "__main__":
    bodies = icons()
    names = sys.argv[1:] or sorted(f[:-5] for f in os.listdir(BODIES) if f.endswith(".html"))
    os.makedirs(OUT, exist_ok=True)
    for name in names:
        path = os.path.join(OUT, f"{name}.html")
        with open(path, "w") as f:
            f.write(page(name, bodies))
        print(path)
