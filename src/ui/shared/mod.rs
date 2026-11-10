//! Product components the desktop and the phone share.

pub(crate) mod agent;
pub(crate) mod agents;
pub(crate) mod chips;
pub(crate) mod composer;
pub(crate) mod composer_doc;
pub(crate) mod modals;
pub(crate) mod model_picker;
pub(crate) mod notice;
pub(crate) mod preview;
pub(crate) mod projects;
pub(crate) mod settings;
pub(crate) mod timeline;
pub(crate) mod tool_detail;

/// The followed computer's state as a small mark: the loader while it is
/// being reached (what shows then is this device's copy), a dot once it
/// answers or cannot be reached.
pub(crate) fn conn_mark() -> impl nsh_core::prelude::IntoElement {
    use crate::store::{self, Conn};
    use crate::ui::kit::*;
    use nsh_core::prelude::*;
    let state = computed(|| match store::conn() {
        Conn::Connected { .. } => Some(Status::Done),
        Conn::Down(_) => Some(Status::Failed),
        _ => None,
    });
    view().w(px(16.0)).h(px(16.0)).items_center().justify_center().child(dynamic(move || {
        let state = state.get();
        Some(untracked(move || match state {
            Some(status) => status_dot(move || status).into_element(),
            None => eclipse(16.0).into_element(),
        }))
    }))
}
