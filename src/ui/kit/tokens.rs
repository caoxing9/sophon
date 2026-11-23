//! Sophon's design tokens: the "Ivory & Clay" palette (warm, Claude-like), on
//! this rhythm — a 14 pt interface base, 12 pt metadata, a 4/8/12/16/24
//! spacing ramp, pill buttons 32 pt tall (28 pt in tight chrome), 12 pt
//! cards. `theme.json` gives `nsh/base` (Markdown, menus) the same colors.

#![allow(dead_code)]

use nsh_core::{Hsla, color::rgb, element::AdaptiveColor};

const fn pair(light: u32, dark: u32) -> AdaptiveColor {
    AdaptiveColor::rgb(light, dark)
}

// The palette's colours: surfaces lightest to strongest, text, lines and
// the one call to action per surface. Functions, not constants: the theme
// (`store::theme`) picks them as the app starts and when it changes, which
// mounts the UI again (as a font change does), so each is read as views
// are made.
macro_rules! palette_fns {
    ($($name:ident => $field:ident),* $(,)?) => {
        $(
            #[allow(non_snake_case)]
            #[inline]
            pub(crate) fn $name() -> AdaptiveColor {
                PALETTE.with(|p| p.get().$field)
            }
        )*
    };
}

palette_fns! {
    SURFACE0 => surface0,
    SURFACE1 => surface1,
    SURFACE2 => surface2,
    SURFACE3 => surface3,
    SURFACE4 => surface4,
    POPOVER => popover,
    SIDEBAR => sidebar,
    SIDEBAR_SELECTED => sidebar_selected,
    FG => fg,
    FG_MUTED => fg_muted,
    FG_FAINT => fg_faint,
    BORDER => border,
    BORDER_STRONG => border_strong,
    ACCENT => accent,
    ACCENT_FG => accent_fg,
}

/// Every themed colour, light and dark.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) struct Palette {
    surface0: AdaptiveColor,
    surface1: AdaptiveColor,
    surface2: AdaptiveColor,
    surface3: AdaptiveColor,
    surface4: AdaptiveColor,
    popover: AdaptiveColor,
    sidebar: AdaptiveColor,
    sidebar_selected: AdaptiveColor,
    fg: AdaptiveColor,
    fg_muted: AdaptiveColor,
    fg_faint: AdaptiveColor,
    border: AdaptiveColor,
    border_strong: AdaptiveColor,
    accent: AdaptiveColor,
    accent_fg: AdaptiveColor,
    /// What `highlight()` washes over a surface.
    wash: AdaptiveColor,
}

impl Palette {
    /// Ivory & Clay, by hand: the default theme.
    pub(crate) const IVORY: Palette = Palette {
        surface0: pair(0xfaf9f5, 0x262624),
        surface1: pair(0xf5f4ee, 0x2a2a27),
        surface2: pair(0xf0eee6, 0x30302e),
        surface3: pair(0xe8e6dc, 0x3a3a37),
        surface4: pair(0xd9d6ca, 0x4a4945),
        popover: pair(0xfdfcf9, 0x2f2f2c),
        sidebar: pair(0xf3f1ea, 0x1f1e1d),
        sidebar_selected: pair(0xe8e5da, 0x33322f),
        fg: pair(0x1f1e1d, 0xf5f4ee),
        fg_muted: pair(0x6b6960, 0xa6a39a),
        fg_faint: pair(0xa09d93, 0x6e6c66),
        border: pair(0xe6e3d9, 0x3a3936),
        border_strong: pair(0xd6d2c4, 0x4a4844),
        accent: pair(0xc96442, 0xd97757),
        accent_fg: pair(0xffffff, 0xffffff),
        wash: pair(0x3d3929, 0xfaf9f5),
    };

    /// A palette made from a background, a text colour and an accent for
    /// each appearance: the surfaces, lines and quieter texts are mixes of
    /// the background and the text.
    pub(crate) fn derived(light: Tone, dark: Tone) -> Palette {
        let (l, d) = (Derived::of(light, false), Derived::of(dark, true));
        let p = |f: fn(&Derived) -> u32| pair(f(&l), f(&d));
        Palette {
            surface0: p(|t| t.surface0),
            surface1: p(|t| t.surface1),
            surface2: p(|t| t.surface2),
            surface3: p(|t| t.surface3),
            surface4: p(|t| t.surface4),
            popover: p(|t| t.popover),
            sidebar: p(|t| t.sidebar),
            sidebar_selected: p(|t| t.sidebar_selected),
            fg: p(|t| t.fg),
            fg_muted: p(|t| t.fg_muted),
            fg_faint: p(|t| t.fg_faint),
            border: p(|t| t.border),
            border_strong: p(|t| t.border_strong),
            accent: p(|t| t.accent),
            accent_fg: p(|t| t.accent_fg),
            wash: p(|t| t.fg),
        }
    }

    /// This palette with another accent (`None`: its own).
    pub(crate) fn with_accent(mut self, accent: Option<u32>) -> Palette {
        if let Some(a) = accent {
            self.accent = pair(a, a);
            let on = on_color(a);
            self.accent_fg = pair(on, on);
        }
        self
    }
}

/// One appearance's colours a theme names: `0xRRGGBB` each.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) struct Tone {
    pub(crate) bg: u32,
    pub(crate) fg: u32,
    pub(crate) accent: u32,
}

/// One appearance's colours, mixed from a [`Tone`].
struct Derived {
    surface0: u32,
    surface1: u32,
    surface2: u32,
    surface3: u32,
    surface4: u32,
    popover: u32,
    sidebar: u32,
    sidebar_selected: u32,
    fg: u32,
    fg_muted: u32,
    fg_faint: u32,
    border: u32,
    border_strong: u32,
    accent: u32,
    accent_fg: u32,
}

impl Derived {
    fn of(t: Tone, dark: bool) -> Derived {
        let toward_fg = |amount: f32| mix(t.bg, t.fg, amount);
        // The sidebar a step back from the conversation: darker in the
        // dark, a touch toward the text in the light.
        let sidebar = if dark { mix(t.bg, 0x000000, 0.18) } else { toward_fg(0.035) };
        Derived {
            surface0: t.bg,
            surface1: toward_fg(0.025),
            surface2: toward_fg(0.05),
            surface3: toward_fg(0.09),
            surface4: toward_fg(0.16),
            popover: if dark { toward_fg(0.05) } else { mix(t.bg, 0xffffff, 0.5) },
            sidebar,
            sidebar_selected: mix(sidebar, t.fg, if dark { 0.10 } else { 0.07 }),
            fg: t.fg,
            fg_muted: mix(t.fg, t.bg, 0.42),
            fg_faint: mix(t.fg, t.bg, 0.62),
            border: toward_fg(0.09),
            border_strong: toward_fg(0.15),
            accent: t.accent,
            accent_fg: on_color(t.accent),
        }
    }
}

/// `a` moved `amount` (0 to 1) of the way to `b`, channel by channel.
pub(crate) fn mix(a: u32, b: u32, amount: f32) -> u32 {
    let ch = |c: u32, shift: u32| ((c >> shift) & 0xff) as f32;
    let one = |shift: u32| {
        let v = ch(a, shift) + (ch(b, shift) - ch(a, shift)) * amount.clamp(0.0, 1.0);
        (v.round() as u32).min(255) << shift
    };
    one(16) | one(8) | one(0)
}

/// How light a colour reads, 0 (black) to 1 (white): WCAG's relative luminance.
pub(crate) fn luminance(c: u32) -> f32 {
    let lin = |v: u32| {
        let s = v as f32 / 255.0;
        if s <= 0.03928 { s / 12.92 } else { ((s + 0.055) / 1.055).powf(2.4) }
    };
    0.2126 * lin((c >> 16) & 0xff) + 0.7152 * lin((c >> 8) & 0xff) + 0.0722 * lin(c & 0xff)
}

/// Text that reads on `c`: near black on a light colour, white on a dark one.
pub(crate) fn on_color(c: u32) -> u32 {
    if luminance(c) > 0.45 { 0x1a1a1a } else { 0xffffff }
}

/// The colour themes to choose from: (key, name, swatch, light, dark).
/// Ivory & Clay (`""`) is the hand-made default; the rest are made from
/// their tones (`Palette::derived`).
pub(crate) const THEME_PRESETS: &[(&str, &str, u32, Tone, Tone)] = &[
    ("", "Ivory & Clay", 0xc96442, Tone { bg: 0xfaf9f5, fg: 0x1f1e1d, accent: 0xc96442 }, Tone { bg: 0x262624, fg: 0xf5f4ee, accent: 0xd97757 }),
    ("graphite", "Graphite", 0x6b7280, Tone { bg: 0xffffff, fg: 0x1a1a1a, accent: 0x339cff }, Tone { bg: 0x181818, fg: 0xececec, accent: 0x339cff }),
    ("midnight", "Midnight", 0x3b82f6, Tone { bg: 0xf5f7fb, fg: 0x111827, accent: 0x3b82f6 }, Tone { bg: 0x0f1420, fg: 0xe5e9f0, accent: 0x60a5fa }),
    ("forest", "Forest", 0x2f855a, Tone { bg: 0xf4f7f2, fg: 0x1c2a1f, accent: 0x2f855a }, Tone { bg: 0x141c17, fg: 0xe6efe8, accent: 0x48bb78 }),
    ("rose", "Rose", 0xd6336c, Tone { bg: 0xfbf6f7, fg: 0x2a1d21, accent: 0xd6336c }, Tone { bg: 0x1f1719, fg: 0xf4e9ec, accent: 0xf06595 }),
    ("mono", "Mono", 0x000000, Tone { bg: 0xffffff, fg: 0x000000, accent: 0x000000 }, Tone { bg: 0x000000, fg: 0xffffff, accent: 0xffffff }),
];

/// The preset `key`'s palette; `None` for Ivory & Clay (`Palette::IVORY`)
/// or a key this version does not know.
pub(crate) fn palette_preset(key: &str) -> Option<Palette> {
    if key.is_empty() {
        return None;
    }
    THEME_PRESETS.iter().find(|p| p.0 == key).map(|p| Palette::derived(p.3, p.4))
}

/// The accent of preset `key` (an unknown one: Ivory & Clay's) for one appearance.
pub(crate) fn preset_accent(key: &str, dark: bool) -> u32 {
    let p = THEME_PRESETS.iter().find(|p| p.0 == key).unwrap_or(&THEME_PRESETS[0]);
    if dark { p.4.accent } else { p.3.accent }
}

thread_local! {
    /// The palette in use: kept beside the theme's global, as the text
    /// scale is, since every view made reads it.
    static PALETTE: std::cell::Cell<Palette> = const { std::cell::Cell::new(Palette::IVORY) };
}

/// Draws the views made from now on in `palette` (the theme's job: it
/// mounts the UI again after).
pub(crate) fn set_palette(palette: Palette) {
    PALETTE.with(|p| p.set(palette));
}

pub(crate) const DESTRUCTIVE: AdaptiveColor = pair(0xb53333, 0xe5675a);

// Status: the text/icon band and the louder dot band.
pub(crate) const SUCCESS: AdaptiveColor = pair(0x3f7a4a, 0x7cbf8a);
pub(crate) const DANGER: AdaptiveColor = pair(0xa8402f, 0xe9907f);
pub(crate) const WARNING: AdaptiveColor = pair(0x8a5e14, 0xd9a85c);
pub(crate) const DOT_SUCCESS: AdaptiveColor = pair(0x23a55a, 0x3ccf74);
pub(crate) const DOT_RUNNING: AdaptiveColor = pair(0x3b82f6, 0x60a5fa);
pub(crate) const DOT_WARNING: AdaptiveColor = pair(0xd48a1f, 0xf0a53a);
pub(crate) const DOT_DANGER: AdaptiveColor = pair(0xe5484d, 0xff6b6b);
/// A merged pull request's (GitHub's violet).
pub(crate) const VIOLET: AdaptiveColor = pair(0x8250df, 0xa78bfa);
pub(crate) const DOT_IDLE: AdaptiveColor = pair(0xb4b4be, 0x55555f);

/// The hover / pressed / selected fill of chrome controls: translucent, so
/// one control works on the sidebar and the main surface alike.
pub(crate) fn highlight() -> AdaptiveColor {
    let wash = PALETTE.with(|p| p.get().wash);
    AdaptiveColor::new(wash.light.opacity(0.06), wash.dark.opacity(0.07))
}

/// A sidebar row under the pointer: the chosen row's fill, a step lighter,
/// so the two read as one colour at two strengths.
pub(crate) fn sidebar_hover() -> AdaptiveColor {
    SIDEBAR_SELECTED().opacity(0.6)
}

pub(crate) fn tint(color: AdaptiveColor, alpha: f32) -> AdaptiveColor {
    color.opacity(alpha)
}

/// A text size from the ramp, at the size the user set (`fontSize` in the
/// settings: the base size in points, 14 by default). Read as each view is
/// made: a change mounts the UI again.
pub(crate) fn tx(size: f32) -> nsh_core::geometry::Points {
    nsh_core::geometry::px((size * crate::store::ui_scale().0 * 2.0).round() / 2.0)
}

/// A spacing or a control's height from the ramps, tighter in compact mode.
pub(crate) fn sp(size: f32) -> nsh_core::geometry::Points {
    nsh_core::geometry::px((size * crate::store::ui_scale().1).round())
}

// Type ramp (points).
pub(crate) const TEXT_XS: f32 = 11.0;
pub(crate) const TEXT_SM: f32 = 12.0;
pub(crate) const TEXT_BASE: f32 = 14.0;
pub(crate) const TEXT_CONTENT: f32 = 15.0;
pub(crate) const TEXT_LG: f32 = 16.0;
pub(crate) const TEXT_XL: f32 = 18.0;
pub(crate) const TEXT_2XL: f32 = 22.0;
pub(crate) const TEXT_CODE: f32 = 12.5;

// Spacing ramp.
pub(crate) const S1: f32 = 4.0;
pub(crate) const S2: f32 = 8.0;
pub(crate) const S3: f32 = 12.0;
pub(crate) const S4: f32 = 16.0;
pub(crate) const S6: f32 = 24.0;
pub(crate) const S8: f32 = 32.0;

// Radii.
pub(crate) const R_SM: f32 = 4.0;
pub(crate) const R_MD: f32 = 6.0;
pub(crate) const R_LG: f32 = 8.0;
pub(crate) const R_XL: f32 = 12.0;
pub(crate) const R_FULL: f32 = 999.0;

/// A phone or tablet: the app is a remote for a computer's agents, sized
/// for fingers (Apple's 44 pt touch target).
pub(crate) const MOBILE: bool = cfg!(any(target_os = "ios", target_os = "android", target_arch = "wasm32"));

// Control heights: touch values on mobile, pointer values on desktop.
pub(crate) const CONTROL_MD: f32 = if MOBILE { 44.0 } else { 32.0 };
pub(crate) const CONTROL_SM: f32 = if MOBILE { 32.0 } else { 28.0 };

pub(crate) const CONTENT_MAX_WIDTH: f32 = 820.0;
pub(crate) const SIDEBAR_WIDTH: f32 = 272.0;

/// The code font, unless the settings name another (`mono_font`).
pub(crate) const MONO: &str = "Menlo";

/// The font of code, terminals and numbers: the one the settings name
/// (`monoFontFamily`), else Menlo. Read as each view is made: a change
/// mounts the UI again.
pub(crate) fn mono_font() -> nsh_core::prelude::SharedString {
    crate::store::fonts_untracked().1.unwrap_or_else(|| MONO.to_owned()).into()
}

/// Workspace label colors, by name.
pub(crate) fn label_tone(name: &str) -> AdaptiveColor {
    match name {
        "sky" => pair(0x0ea5e9, 0x38bdf8),
        "emerald" => pair(0x10b981, 0x34d399),
        "orange" => pair(0xf97316, 0xfb923c),
        "pink" => pair(0xec4899, 0xf472b6),
        "indigo" => pair(0x6366f1, 0x818cf8),
        "teal" => pair(0x14b8a6, 0x2dd4bf),
        "red" => pair(0xef4444, 0xf87171),
        "amber" => pair(0xf59e0b, 0xfbbf24),
        "blue" => pair(0x3b82f6, 0x60a5fa),
        _ => pair(0x8b5cf6, 0xa78bfa),
    }
}

pub(crate) const LABEL_COLORS: [&str; 10] =
    ["violet", "sky", "emerald", "orange", "pink", "indigo", "teal", "red", "amber", "blue"];

/// Liquid glass: a translucent fill the backdrop blur shows through.
pub(crate) fn glass_fill() -> AdaptiveColor {
    let p = POPOVER();
    AdaptiveColor::new(p.light.opacity(0.64), p.dark.opacity(0.60))
}

/// A denser glass for text-heavy surfaces (menus, dialogs, the composer).
pub(crate) fn glass_fill_strong() -> AdaptiveColor {
    let p = POPOVER();
    AdaptiveColor::new(p.light.opacity(0.93), p.dark.opacity(0.92))
}

/// The glass edge: a light catching rim.
pub(crate) fn glass_edge() -> AdaptiveColor {
    AdaptiveColor::new(Hsla::from(rgb(0xffffff)).opacity(0.75), Hsla::from(rgb(0xffffff)).opacity(0.10))
}

pub(crate) const GLASS_BLUR: f32 = 28.0;
pub(crate) const R_GLASS: f32 = 18.0;
