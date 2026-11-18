//! Git in the workspace header: ahead / behind. The actions (commit, push, pull, …) are the command
//! palette's: the agents do most of them, and a button for them was noise.

use nsh_core::prelude::*;
use crate::ui::kit::text;

use crate::ui::kit::*;
use crate::store;

pub(crate) fn git_controls() -> impl IntoElement {
    let git = computed(store::git);
    view()
        .flex_row()
        .items_center()
        .gap(sp(S2))
        .child(dynamic(move || {
            let st = git.with(|g| g.status.clone()).filter(|s| s.is_git && s.has_remote)?;
            if st.ahead + st.behind == 0 {
                return None;
            }
            Some(untracked(move || {
                text(format!("↑{} ↓{}", st.ahead, st.behind)).text_size(tx(TEXT_SM)).font_family(mono_font).text_color(FG_MUTED)
            }))
        }))
}

