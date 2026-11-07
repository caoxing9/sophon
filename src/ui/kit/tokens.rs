//! Sophon's design tokens: the "Ivory & Clay" palette (warm, Claude-like), on
//! Paseo's rhythm — a 14 pt interface base, 12 pt metadata, a 4/8/12/16/24
//! spacing ramp, pill buttons 32 pt tall (28 pt in tight chrome), 12 pt
//! cards. `theme.json` gives `nsh/base` (Markdown, menus) the same colors.

#![allow(dead_code)]

use nsh_core::{Hsla, color::rgb, element::AdaptiveColor};

const fn pair(light: u32, dark: u32) -> AdaptiveColor {
    AdaptiveColor::rgb(light, dark)
}

// Surfaces, lightest to strongest.
pub(crate) const SURFACE0: AdaptiveColor = pair(0xfaf9f5, 0x262624);
pub(crate) const SURFACE1: AdaptiveColor = pair(0xf5f4ee, 0x2a2a27);
pub(crate) const SURFACE2: AdaptiveColor = pair(0xf0eee6, 0x30302e);
pub(crate) const SURFACE3: AdaptiveColor = pair(0xe8e6dc, 0x3a3a37);
pub(crate) const SURFACE4: AdaptiveColor = pair(0xd9d6ca, 0x4a4945);
pub(crate) const POPOVER: AdaptiveColor = pair(0xfdfcf9, 0x2f2f2c);
pub(crate) const SIDEBAR: AdaptiveColor = pair(0xf3f1ea, 0x1f1e1d);
pub(crate) const SIDEBAR_SELECTED: AdaptiveColor = pair(0xe8e5da, 0x33322f);

// Text.
pub(crate) const FG: AdaptiveColor = pair(0x1f1e1d, 0xf5f4ee);
pub(crate) const FG_MUTED: AdaptiveColor = pair(0x6b6960, 0xa6a39a);
pub(crate) const FG_FAINT: AdaptiveColor = pair(0xa09d93, 0x6e6c66);

// Lines.
pub(crate) const BORDER: AdaptiveColor = pair(0xe6e3d9, 0x3a3936);
pub(crate) const BORDER_STRONG: AdaptiveColor = pair(0xd6d2c4, 0x4a4844);

// The one call to action per surface.
pub(crate) const ACCENT: AdaptiveColor = pair(0xc96442, 0xd97757);
pub(crate) const ACCENT_FG: AdaptiveColor = pair(0xffffff, 0xffffff);
pub(crate) const DESTRUCTIVE: AdaptiveColor = pair(0xb53333, 0xe5675a);

// Status: the text/icon band and the louder dot band.
pub(crate) const SUCCESS: AdaptiveColor = pair(0x3f7a4a, 0x7cbf8a);
pub(crate) const DANGER: AdaptiveColor = pair(0xa8402f, 0xe9907f);
pub(crate) const WARNING: AdaptiveColor = pair(0x8a5e14, 0xd9a85c);
pub(crate) const DOT_SUCCESS: AdaptiveColor = pair(0x23a55a, 0x3ccf74);
pub(crate) const DOT_RUNNING: AdaptiveColor = pair(0x3b82f6, 0x60a5fa);
pub(crate) const DOT_WARNING: AdaptiveColor = pair(0xd48a1f, 0xf0a53a);
pub(crate) const DOT_DANGER: AdaptiveColor = pair(0xe5484d, 0xff6b6b);
pub(crate) const DOT_IDLE: AdaptiveColor = pair(0xb4b4be, 0x55555f);

/// The hover / pressed / selected fill of chrome controls: translucent, so
/// one control works on the sidebar and the main surface alike.
pub(crate) fn highlight() -> AdaptiveColor {
    AdaptiveColor::new(Hsla::from(rgb(0x3d3929)).opacity(0.06), Hsla::from(rgb(0xfaf9f5)).opacity(0.07))
}

pub(crate) fn tint(color: AdaptiveColor, alpha: f32) -> AdaptiveColor {
    color.opacity(alpha)
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
/// for fingers (Apple's 44 pt touch target), as Paseo's touch sizing.
pub(crate) const MOBILE: bool = cfg!(any(target_os = "ios", target_os = "android"));

// Control heights: touch values on mobile, pointer values on desktop.
pub(crate) const CONTROL_MD: f32 = if MOBILE { 44.0 } else { 32.0 };
pub(crate) const CONTROL_SM: f32 = if MOBILE { 32.0 } else { 28.0 };

pub(crate) const CONTENT_MAX_WIDTH: f32 = 820.0;
pub(crate) const SIDEBAR_WIDTH: f32 = 272.0;

pub(crate) const MONO: &str = "Menlo";

/// Workspace label colors, by Paseo's names.
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
    AdaptiveColor::new(Hsla::from(rgb(0xfdfcf9)).opacity(0.64), Hsla::from(rgb(0x2c2c29)).opacity(0.60))
}

/// A denser glass for text-heavy surfaces (menus, dialogs, the composer).
pub(crate) fn glass_fill_strong() -> AdaptiveColor {
    AdaptiveColor::new(Hsla::from(rgb(0xfdfcf9)).opacity(0.93), Hsla::from(rgb(0x2f2f2c)).opacity(0.92))
}

/// The glass edge: a light catching rim.
pub(crate) fn glass_edge() -> AdaptiveColor {
    AdaptiveColor::new(Hsla::from(rgb(0xffffff)).opacity(0.75), Hsla::from(rgb(0xffffff)).opacity(0.10))
}

pub(crate) const GLASS_BLUR: f32 = 28.0;
pub(crate) const R_GLASS: f32 = 18.0;
