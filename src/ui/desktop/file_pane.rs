//! A file tab: the file in `nsh/base`'s code editor, saved back through the
//! daemon (refused when the file changed on disk meanwhile).

use crate::i18n::{t, tf};
use nsh_base::editor;
use nsh_core::prelude::*;
use crate::ui::kit::text;

use crate::ui::kit::*;
use crate::store;

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
                .gap(px(S2))
                .h(px(40.0))
                .px(px(S4))
                .border_b_1()
                .border_color(BORDER)
                .child(icon(Icon::FileText, 15.0).text_color(FG_MUTED))
                .child(text(path.clone()).text_size(px(TEXT_BASE)).font_family(|| MONO).number_of_lines(1).flex_1())
                .child(when(move || file.with(|f| f.as_ref().is_some_and(|f| f.dirty())), || {
                    text(t("Unsaved")).text_size(px(TEXT_SM)).text_color(FG_MUTED)
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
        .child(dynamic(move || {
            let (loaded, kind, error) = state.get()?;
            let language = language.clone();
            let edit_path = edit_path.clone();
            Some(untracked(move || {
                if let Some(e) = error {
                    return note(Icon::CircleAlert, e);
                }
                if !loaded {
                    return note(Icon::RotateCw, t("Loading…").into());
                }
                if kind != "text" {
                    return note(Icon::Info, tf("This {} file can't be shown as text", &[&kind.to_string()]));
                }
                editor()
                    .value(move || file.with(|f| f.as_ref().map(|f| SharedString::from(f.text.clone()))))
                    .language(SharedString::from(language.clone()))
                    .on_change(move |v: &SharedString| store::edit_file(&edit_path, v))
                    .text_size(px(TEXT_CODE))
                    .flex_1()
                    .into_element()
            }))
        }))
}

fn note(glyph: Icon, message: String) -> AnyElement {
    view()
        .flex_1()
        .items_center()
        .justify_center()
        .gap(px(S2))
        .child(icon(glyph, 18.0).text_color(FG_FAINT))
        .child(text(message).text_size(px(TEXT_BASE)).text_color(FG_MUTED))
        .into_element()
}
