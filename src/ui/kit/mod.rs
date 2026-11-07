//! Sophon's design system: tokens, icons and controls. Every screen builds
//! from these, never from hand-styled `nsh/base` elements.

pub(crate) mod controls;
pub(crate) mod icon;
pub(crate) mod menu;
pub(crate) mod tokens;

pub(crate) use controls::*;
pub(crate) use icon::{Icon, icon};
pub(crate) use menu::*;
pub(crate) use tokens::*;
