//! The phone's menus: a glass panel that springs up from the bottom over a
//! dimmed screen, its rows with icons, and Cancel apart under it (Lody's
//! and iOS 26's bottom popovers). A tap on the dim, Cancel or any row puts
//! it away; a row runs its action once it has gone.
//!
//! Callers say what to show ([`show_sheet`]); the shell mounts the one
//! layer that draws it ([`sheet_layer`]), last, over everything. It is a
//! layer of the app's own, not a native presentation: closing one sheet
//! and opening the next (or the photo picker) never races a dismissal.

use std::{cell::RefCell, rc::Rc, time::Duration};

use nsh_base::button;
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

/// The sheet on screen; `leaving` while it slides away.
#[derive(Clone, Default)]
struct Shown {
    sheet: Option<(Option<String>, Vec<SheetItem>)>,
    leaving: bool,
    /// Bumped per sheet, so a new one mounts afresh (and rises).
    serial: u64,
}

/// How long the sheet takes to leave; a row's action runs after it.
const LEAVE: Duration = Duration::from_millis(220);

thread_local! {
    /// The layer's owner: what runs after a sheet has gone outlives the
    /// sheet's rows (a row's own timer would go with the row).
    static LAYER: RefCell<Option<Owner>> = const { RefCell::new(None) };
}

/// Shows `items` in the bottom sheet, under `title` when given.
pub(crate) fn show_sheet(title: Option<String>, items: Vec<SheetItem>) {
    dismiss_keyboard();
    global::<Shown>().update(|s| {
        s.sheet = Some((title, items));
        s.leaving = false;
        s.serial += 1;
    });
}

/// Slides the sheet away, then runs `then` (another sheet, a picker).
fn close_then(then: Option<Rc<dyn Fn()>>) {
    let serial = global::<Shown>().with_untracked(|s| s.serial);
    if global::<Shown>().with_untracked(|s| s.sheet.is_none() || s.leaving) {
        return;
    }
    global::<Shown>().update(|s| s.leaving = true);
    let finish = move || {
        // A sheet shown meanwhile stays.
        if global::<Shown>().with_untracked(|s| s.serial == serial) {
            global::<Shown>().update(|s| {
                s.sheet = None;
                s.leaving = false;
            });
        }
        if let Some(then) = &then {
            then();
        }
    };
    let owner = LAYER.with_borrow(Clone::clone);
    match owner {
        Some(owner) => owner.run(|| {
            set_timeout(LEAVE, finish);
        }),
        None => finish(),
    }
}

fn close() {
    close_then(None);
}

/// The layer the sheets show in: the shell's last child, over everything.
pub(crate) fn sheet_layer() -> impl IntoElement {
    LAYER.with_borrow_mut(|o| *o = Some(Owner::current()));
    let serial = computed(|| global::<Shown>().with(|s| s.sheet.as_ref().map(|_| s.serial)));
    view().absolute().top(px(0.0)).bottom(px(0.0)).left(px(0.0)).right(px(0.0)).pointer_events(nsh_core::PointerEvents::BoxNone).child(dynamic(move || {
        serial.get()?;
        let (title, items) = global::<Shown>().with_untracked(|s| s.sheet.clone())?;
        Some(untracked(move || panel(title, items).into_element()))
    }))
}

fn panel(title: Option<String>, items: Vec<SheetItem>) -> impl IntoElement {
    // The panel rises from below the screen's edge on a soft spring.
    let rise = animated_value(420.0);
    rise.spring(0.0, Spring { stiffness: 320.0, damping: 32.0, ..Spring::default() });
    let dim = animated_value(0.0);
    dim.timing(1.0, 200.0, Easing::EaseOut);
    // Leaving: down and out, as fast as the action waits for.
    let leaving = computed(|| global::<Shown>().with(|s| s.leaving));
    effect(move || {
        if leaving.get() {
            rise.timing(420.0, LEAVE.as_millis() as f32, Easing::EaseIn);
            dim.timing(0.0, LEAVE.as_millis() as f32, Easing::EaseIn);
        }
    });
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
                .bg(AdaptiveColor::rgb(0x000000, 0x000000).opacity(0.28))
                .bind_opacity(&dim),
        )
        .child(
            view()
                .px(sp(S2))
                .pb_safe_area_plus(sp(S2))
                .gap(sp(S2))
                .bind_translate_y(&rise)
                .child(
                    liquid_glass(view())
                        .rounded(px(28.0))
                        .overflow_hidden()
                        .child(view().items_center().pt(sp(S2)).child(view().w(px(36.0)).h(px(5.0)).rounded(px(R_FULL)).bg(tint(FG, 0.18))))
                        .when_some(title, |v, title| {
                            v.child(
                                text(title)
                                    .px(sp(S4))
                                    .pt(sp(S2))
                                    .pb(sp(S1))
                                    .text_size(tx(TEXT_SM))
                                    .font_weight(FontWeight(600))
                                    .text_color(FG_MUTED)
                                    .number_of_lines(1),
                            )
                        })
                        .child(scroll().max_h(px(480.0)).child(rows.pb(sp(S1)))),
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
                            .child(text(crate::i18n::t("Cancel")).text_size(tx(TEXT_LG)).font_weight(FontWeight(600))),
                    ),
                ),
        )
}

fn row(item: SheetItem, divider: bool) -> impl IntoElement {
    let ink = if item.destructive { DANGER } else { FG };
    let run = item.run.clone();
    button()
        // The action runs once the sheet has gone, so the next sheet
        // rises in its place.
        .on_press(move || close_then(Some(run.clone())))
        .flex_row()
        .items_center()
        .gap(sp(S3))
        .min_h(px(52.0))
        .px(sp(S4))
        .active(|s| s.bg(highlight()))
        .child(match item.glyph {
            Some(g) => view().w(px(22.0)).items_center().child(icon(g, 20.0).text_color(if item.destructive { DANGER } else { FG_MUTED })),
            None => view(),
        })
        .child(
            view()
                .flex_1()
                .min_w(px(0.0))
                .py(sp(S3))
                .when(divider, |v| v)
                .child(text(item.label).text_size(tx(TEXT_LG)).text_color(ink).number_of_lines(2)),
        )
        .child(match item.checked {
            true => icon(Icon::Check, 18.0).text_color(ACCENT).into_element(),
            false => view().into_element(),
        })
}
