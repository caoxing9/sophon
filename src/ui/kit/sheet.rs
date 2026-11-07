//! The phone's menus: a glass panel that springs up from the bottom over a
//! dimmed screen, its rows with icons, and Cancel apart under it (Lody's
//! and iOS 26's bottom popovers). A tap on the dim, Cancel or any row puts
//! it away; a row runs its action as it goes.
//!
//! Callers say what to show ([`show_sheet`]); the shell mounts the one
//! layer that draws it ([`sheet_layer`]).

use std::rc::Rc;

use nsh_base::{button, dialog};
use nsh_core::{element::FontWeight, prelude::*};

use super::{controls::*, icon::*, tokens::*};
use super::controls::text;

/// One row of a sheet.
#[derive(Clone)]
pub(crate) struct SheetItem {
    label: String,
    glyph: Option<Icon>,
    destructive: bool,
    checked: bool,
    run: Rc<dyn Fn()>,
}

impl SheetItem {
    pub(crate) fn new(label: impl Into<String>, run: impl Fn() + 'static) -> Self {
        Self { label: label.into(), glyph: None, destructive: false, checked: false, run: Rc::new(run) }
    }

    pub(crate) fn destructive(label: impl Into<String>, run: impl Fn() + 'static) -> Self {
        Self { destructive: true, ..Self::new(label, run) }
    }

    /// Its leading icon.
    pub(crate) fn icon(mut self, glyph: Icon) -> Self {
        self.glyph = Some(glyph);
        self
    }

    /// Marked as the current choice.
    pub(crate) fn checked(mut self, checked: bool) -> Self {
        self.checked = checked;
        self
    }
}

#[derive(Clone, Default)]
struct Shown(Option<(Option<String>, Vec<SheetItem>)>);

/// Shows `items` in the bottom sheet, under `title` when given.
pub(crate) fn show_sheet(title: Option<String>, items: Vec<SheetItem>) {
    dismiss_keyboard();
    global::<Shown>().set(Shown(Some((title, items))));
}

fn close() {
    global::<Shown>().set(Shown(None));
}

/// The layer the sheets show in, over everything (the phone's shell).
pub(crate) fn sheet_layer() -> impl IntoElement {
    let open = computed(|| global::<Shown>().with(|s| s.0.is_some()));
    when(open, || {
        dialog().animation(ModalAnimation::Fade).on_cancel(close).flex_1().child(dynamic(|| {
            let (title, items) = global::<Shown>().with(|s| s.0.clone())?;
            Some(untracked(move || panel(title, items).into_element()))
        }))
    })
}

fn panel(title: Option<String>, items: Vec<SheetItem>) -> impl IntoElement {
    // The panel rises from below the screen's edge on a soft spring.
    let rise = animated_value(420.0);
    rise.spring(0.0, Spring { stiffness: 320.0, damping: 32.0, ..Spring::default() });
    let count = items.len();
    let mut rows = view();
    for (i, item) in items.into_iter().enumerate() {
        rows = rows.child(row(item, i + 1 < count));
    }
    view()
        .flex_1()
        .justify_end()
        .child(
            // The dim: a tap on it closes.
            button()
                .on_press(close)
                .absolute()
                .top(px(0.0))
                .bottom(px(0.0))
                .left(px(0.0))
                .right(px(0.0))
                .bg(AdaptiveColor::rgb(0x000000, 0x000000).opacity(0.28)),
        )
        .child(
            view()
                .px(px(S2))
                .pb_safe_area_plus(px(S2))
                .gap(px(S2))
                .bind_translate_y(&rise)
                .child(
                    liquid_glass(view())
                        .rounded(px(28.0))
                        .overflow_hidden()
                        .child(view().items_center().pt(px(S2)).child(view().w(px(36.0)).h(px(5.0)).rounded(px(R_FULL)).bg(tint(FG, 0.18))))
                        .when_some(title, |v, title| {
                            v.child(
                                text(title)
                                    .px(px(S4))
                                    .pt(px(S2))
                                    .pb(px(S1))
                                    .text_size(px(TEXT_SM))
                                    .font_weight(FontWeight(600))
                                    .text_color(FG_MUTED)
                                    .number_of_lines(1),
                            )
                        })
                        .child(scroll().max_h(px(480.0)).child(rows.pb(px(S1)))),
                )
                .child(
                    liquid_glass(view()).rounded(px(R_FULL)).child(
                        button()
                            .on_press(close)
                            .h(px(56.0))
                            .items_center()
                            .justify_center()
                            .rounded(px(R_FULL))
                            .active(|s| s.bg(highlight()))
                            .child(text(crate::i18n::t("Cancel")).text_size(px(TEXT_LG)).font_weight(FontWeight(600))),
                    ),
                ),
        )
}

fn row(item: SheetItem, divider: bool) -> impl IntoElement {
    let ink = if item.destructive { DANGER } else { FG };
    let run = item.run.clone();
    button()
        .on_press(move || {
            close();
            // After the sheet has gone: iOS will not present (the photo
            // picker, a dialog) while another presentation is leaving.
            let run = run.clone();
            set_timeout(std::time::Duration::from_millis(450), move || run());
        })
        .flex_row()
        .items_center()
        .gap(px(S3))
        .min_h(px(52.0))
        .px(px(S4))
        .active(|s| s.bg(highlight()))
        .child(match item.glyph {
            Some(g) => view().w(px(22.0)).items_center().child(icon(g, 20.0).text_color(if item.destructive { DANGER } else { FG_MUTED })),
            None => view(),
        })
        .child(
            view()
                .flex_1()
                .min_w(px(0.0))
                .py(px(S3))
                .when(divider, |v| v)
                .child(text(item.label).text_size(px(TEXT_LG)).text_color(ink).number_of_lines(2)),
        )
        .child(match item.checked {
            true => icon(Icon::Check, 18.0).text_color(ACCENT).into_element(),
            false => view().into_element(),
        })
}
