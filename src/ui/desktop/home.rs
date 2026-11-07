//! The main surface with no workspace open. Launching opens the one open
//! last (`store::restore_last_open`), so this shows for a host with no
//! workspaces at all: the mark and one way forward. With workspaces it
//! stays a quiet, empty surface (the sidebar is where to pick one).

use crate::i18n::t;
use nsh_core::prelude::*;

use crate::store;
use crate::ui::kit::*;

pub(crate) fn home() -> impl IntoElement {
    let none = computed(|| store::workspaces().is_empty());
    view().flex_1().items_center().justify_center().pb(px(S8)).child(when(none, || {
        view()
            .items_center()
            .gap(px(S6))
            .child(mark(72.0).shadow_lg())
            .child(btn(t("New workspace"), Variant::Primary, Sz::Md, Some(Icon::SquarePen), || store::open_new_workspace(None)))
    }))
}
