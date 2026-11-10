//! Component sizes: GPUI Component's `xs`, `sm`, `md` (the default) and `lg`.

/// How large a control is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Size {
    XSmall,
    Small,
    #[default]
    Medium,
    Large,
}

impl Size {
    /// A control's height (points): 20, 24, 32, 32 (Large adds padding only).
    pub fn height(self) -> f32 {
        match self {
            Size::XSmall => 20.0,
            Size::Small => 24.0,
            Size::Medium | Size::Large => 32.0,
        }
    }

    /// A labelled control's side padding.
    pub fn padding_x(self) -> f32 {
        match self {
            Size::XSmall => 4.0,
            Size::Small => 8.0,
            Size::Medium => 10.0,
            Size::Large => 12.0,
        }
    }

    /// Its text.
    pub fn text(self) -> f32 {
        match self {
            Size::XSmall | Size::Small => 12.0,
            Size::Medium | Size::Large => 14.0,
        }
    }

    /// Its icon.
    pub fn icon(self) -> f32 {
        match self {
            Size::XSmall => 12.0,
            Size::Small => 14.0,
            Size::Medium | Size::Large => 16.0,
        }
    }

    /// The gap between its icon and label.
    pub fn gap(self) -> f32 {
        match self {
            Size::XSmall | Size::Small => 4.0,
            Size::Medium | Size::Large => 6.0,
        }
    }
}
