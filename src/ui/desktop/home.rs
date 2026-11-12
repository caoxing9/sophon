//! The main surface with no workspace open. Launching, or switching host,
//! opens the workspace open last (`store::restore_last_open`); until that
//! was looked for this stays empty. After it, the mark and one way forward:
//! an empty surface reads as something broken.

use crate::i18n::t;
use nsh_core::prelude::*;

use crate::store;
use crate::ui::kit::*;

pub(crate) fn home() -> impl IntoElement {
    // With no workspaces (none yet, or not connected) there is none to wait for.
    let settled = computed(|| store::last_open_restored() || store::workspaces().is_empty());
    view().flex_1().items_center().justify_center().pb(sp(S8)).child(when(settled, || {
        view()
            .items_center()
            .gap(sp(S6))
            .child(mark(72.0, Lift::Large))
            .child(btn(t("New workspace"), Variant::Primary, Sz::Md, Some(Icon::SquarePen), || store::open_new_workspace(None)))
    }))
}
