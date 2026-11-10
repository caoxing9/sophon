//! The colors components draw with: the app's theme (`nsh_base::theme()`,
//! shadcn's tokens) and the ones GPUI Component adds beside them (success,
//! warning, info, link), which a theme may set by name and otherwise take
//! GPUI Component's defaults. States are derived as GPUI Component derives
//! them: a hover is the fill a step toward the text, a press a step further.

use nsh_core::{Hsla, color::rgb, element::AdaptiveColor};

pub use nsh_base::theme;

fn named(name: &str, light: u32, dark: u32) -> AdaptiveColor {
    theme().color(name).unwrap_or_else(|| AdaptiveColor::rgb(light, dark))
}

/// Success: green-500 / green-600.
pub fn success() -> AdaptiveColor {
    named("success", 0x22c55e, 0x16a34a)
}

/// Warning: yellow-500 / yellow-600.
pub fn warning() -> AdaptiveColor {
    named("warning", 0xeab308, 0xca8a04)
}

/// Information: cyan-500 / cyan-600.
pub fn info() -> AdaptiveColor {
    named("info", 0x06b6d4, 0x0891b2)
}

/// Danger: the theme's `destructive`.
pub fn danger() -> AdaptiveColor {
    theme().destructive
}

/// A link's text: the theme's `link`, else the foreground.
pub fn link() -> AdaptiveColor {
    theme().color("link").unwrap_or_else(|| theme().foreground)
}

/// White, the text on a strong fill in both appearances.
pub fn on_strong() -> AdaptiveColor {
    AdaptiveColor::new(rgb(0xffffff), rgb(0xffffff))
}

/// `color` a step `toward` another (oklab), in each appearance.
pub fn mix(color: AdaptiveColor, toward: AdaptiveColor, amount: f32) -> AdaptiveColor {
    AdaptiveColor::new(color.light.mix_oklab(toward.light, 1.0 - amount), color.dark.mix_oklab(toward.dark, 1.0 - amount))
}

/// The fill under the pointer: a tenth toward the text.
pub fn hovered(fill: AdaptiveColor) -> AdaptiveColor {
    mix(fill, theme().foreground, 0.1)
}

/// The fill while pressed: a fifth toward the text.
pub fn pressed(fill: AdaptiveColor) -> AdaptiveColor {
    mix(fill, theme().foreground, 0.2)
}

/// Fully transparent.
pub fn clear() -> AdaptiveColor {
    AdaptiveColor::new(Hsla::transparent_black(), Hsla::transparent_black())
}
