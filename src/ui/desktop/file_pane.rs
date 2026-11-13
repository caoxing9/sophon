//! A file tab: the file in `nsh/base`'s code editor, saved back through the
//! daemon (refused when the file changed on disk meanwhile).

use crate::i18n::{t, tf};
use nsh_base::editor;
use nsh_core::prelude::*;
use crate::ui::kit::text;

use crate::ui::kit::*;
use crate::store;
use crate::ui::shared::preview;

pub(crate) fn file_pane(path: String) -> impl IntoElement {
    let file = {
        let path = path.clone();
        computed(move || store::file(&path))
    };
    let state = computed(move || file.with(|f| f.as_ref().map(|f| (f.loaded, f.kind.clone(), f.error.clone()))));
    let language = path.rsplit('/').next().unwrap_or(&path).to_owned();
    let (save_path, edit_path, down_path) = (path.clone(), path.clone(), path.clone());
    view()
        .flex_1()
        .child(
            view()
                .flex_row()
                .items_center()
                .gap(sp(S2))
                .h(px(40.0))
                .px(sp(S4))
                .border_b_1()
                .border_color(BORDER)
                .child(icon(Icon::FileText, 15.0).text_color(FG_MUTED))
                .child(text(path.clone()).text_size(tx(TEXT_BASE)).font_family(mono_font).number_of_lines(1).flex_1())
                .child(when(move || file.with(|f| f.as_ref().is_some_and(|f| f.dirty())), || {
                    text(t("Unsaved")).text_size(tx(TEXT_SM)).text_color(FG_MUTED)
                }))
                // A copy on this device (the host may be another computer):
                // a file that can't be shown as text too.
                .child(icon_btn(Icon::Download, Sz::Sm, t("Download…"), move || store::download_file(&down_path)))
                .child(
                    btn(
                        move || if file.with(|f| f.as_ref().is_some_and(|f| f.saving)) { t("Saving…") } else { t("Save") },
                        Variant::Primary,
                        Sz::Sm,
                        None,
                        move || store::save_file(&save_path),
                    )
                    .disabled(move || !file.with(|f| f.as_ref().is_some_and(|f| f.dirty()))),
                ),
        )
        // A PDF, an image, a video: shown as what it is.
        .when(preview::kind_of(&path).is_some(), |v| v.child(preview::file_preview(path.clone())))
        .when(preview::kind_of(&path).is_none(), |v| v.child(dynamic(move || {
            let (loaded, kind, error) = state.get()?;
            let language = language.clone();
            let edit_path = edit_path.clone();
            let other = edit_path.clone();
            Some(untracked(move || {
                // Too large to read as text, or not text: it still
                // downloads, and opens in its own app.
                if let Some(e) = error {
                    return preview::fallback(other, Icon::CircleAlert, e);
                }
                if !loaded {
                    return view().flex_1().items_center().justify_center().child(eclipse(36.0)).into_element();
                }
                if kind != "text" {
                    return preview::fallback(other, Icon::File, tf("This {} file can't be shown as text", &[&kind.to_string()]));
                }
                editor()
                    .value(move || file.with(|f| f.as_ref().map(|f| SharedString::from(f.text.clone()))))
                    .language(SharedString::from(language.clone()))
                    .on_change(move |v: &SharedString| store::edit_file(&edit_path, v))
                    .text_size(tx(TEXT_CODE))
                    .flex_1()
                    .into_element()
            }))
        })))
}
