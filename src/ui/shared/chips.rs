//! The composer's chips, as Teable's chat editor draws them: an inline box
//! centred on the line (the editor's `align-middle` class on the box that
//! holds it), with a faint accent border and fill, a thumbnail or icon and
//! a name. An attachment chip opens its preview when pressed: an image fills
//! the screen over a dim, a file shows a card (on the phone, a sheet) with
//! its type, size and upload state.

use std::{cell::RefCell, collections::HashMap, sync::Arc};

use nsh_base::{
    button, dialog,
    rich_editor::{
        model::Node,
        view::{NodeView, NodeViewCx, NodeViews},
    },
};
use nsh_core::{ResizeMode, element::FontWeight, prelude::*};
use serde_json::Value;

use super::composer_doc::{self, ATTACHMENT, MENTION};
use crate::{
    i18n::t,
    store,
    ui::kit::{text, *},
};

/// The composer `key`'s node views: its chips.
pub(crate) fn node_views(key: &str) -> NodeViews {
    NodeViews::new()
        .with(ATTACHMENT, Arc::new(AttachmentView { composer: key.to_owned() }))
        .with(MENTION, Arc::new(MentionView))
}

fn attr<'a>(node: &'a Node, name: &str) -> &'a str {
    node.attrs().get(name).and_then(Value::as_str).unwrap_or("")
}

/// A chip's box: a little under the line's height (centred on its x height,
/// it then reaches no further than the text), the accent faint around and
/// under it, a little apart from the text after it.
fn chip_box(el: nsh_base::ButtonElement) -> nsh_base::ButtonElement {
    el.flex_row()
        .items_center()
        .gap(px(4.0))
        .h(px(18.0))
        .ml(px(1.0))
        .mr(px(4.0))
        .rounded(px(R_MD))
        .border_1()
        .border_color(tint(ACCENT, 0.18))
        .bg(tint(ACCENT, 0.06))
}

struct AttachmentView {
    composer: String,
}

impl NodeView for AttachmentView {
    fn create(&self, _cx: &NodeViewCx, node: &Node) -> AnyElement {
        let id = attr(node, "id").to_owned();
        let name = attr(node, "name").to_owned();
        let image = attr(node, "kind") == "image";
        let composer = self.composer.clone();
        // A file dims while it uploads; the change fades.
        let uploading = {
            let id = id.clone();
            computed(move || !image && store::pending_file(&id).is_none_or(|f| f.uploading()))
        };
        let fade = animated_value(if uploading.get_untracked() { 0.5 } else { 1.0 });
        // Only a change animates: the chip is made again with each edit of
        // its line, and starting the fade then would only cost a frame.
        let was = std::cell::Cell::new(uploading.get_untracked());
        effect(move || {
            let now = uploading.get();
            if was.replace(now) != now {
                fade.timing(if now { 0.5 } else { 1.0 }, 180.0, Easing::EaseOut);
            }
        });
        let lead = if image {
            match thumbnail(&id) {
                Some(uri) => image_el(uri).w(px(14.0)).h(px(14.0)).rounded(px(3.0)).object_fit(ResizeMode::Cover).into_element(),
                None => icon(Icon::Image, 13.0).text_color(FG_MUTED).into_element(),
            }
        } else {
            icon(Icon::FileText, 13.0).text_color(FG_MUTED).into_element()
        };
        let open = id.clone();
        chip_box(button())
            .on_press(move || open_preview(&composer, &open))
            .accessibility_label(name.clone())
            .pl(px(if image { 3.0 } else { 6.0 }))
            .pr(px(7.0))
            .hover(|s| s.bg(tint(ACCENT, 0.12)))
            .active(|s| s.bg(tint(ACCENT, 0.16)))
            .bind_opacity(&fade)
            .child(lead)
            .child(text(name).text_size(px(TEXT_SM)).line_height(px(16.0)).text_color(FG).number_of_lines(1).max_w(px(180.0)))
            .into_element()
    }
}

struct MentionView;

impl NodeView for MentionView {
    fn create(&self, _cx: &NodeViewCx, node: &Node) -> AnyElement {
        let path = attr(node, "path").to_owned();
        let dir = node.attrs().get("dir").and_then(Value::as_bool).unwrap_or(false);
        // The path's last part: enough to tell files apart in a line.
        let tail = path.trim_end_matches('/').rsplit('/').next().unwrap_or(&path).to_owned();
        let tail = if dir { format!("{tail}/") } else { tail };
        // Not a button: a press beside it places the caret, as on text.
        view()
            .flex_row()
            .items_center()
            .gap(px(3.0))
            .h(px(18.0))
            .ml(px(1.0))
            .mr(px(4.0))
                .px(px(5.0))
            .rounded(px(R_MD))
            .border_1()
            .border_color(tint(ACCENT, 0.18))
            .bg(tint(ACCENT, 0.06))
            .label(path)
            .child(icon(if dir { Icon::Folder } else { Icon::FileText }, 12.0).text_color(ACCENT))
            .child(text(tail).text_size(px(TEXT_SM)).line_height(px(16.0)).font_family(|| MONO).text_color(ACCENT).number_of_lines(1).max_w(px(200.0)))
            .into_element()
    }
}

fn image_el(uri: SharedString) -> nsh_core::element::Image {
    image(uri)
}

thread_local! {
    /// The chips' thumbnails by image id, made once. A chip is made again
    /// with each edit of its line, and an image source's `data:` URI is
    /// decoded and hashed each time it is set: the screenshot itself (a
    /// few MB) made every keystroke take ~90 ms, a 42 px thumbnail (a few
    /// KB) takes nothing.
    static URIS: RefCell<HashMap<String, SharedString>> = RefCell::new(HashMap::new());
}

/// The side of a chip's thumbnail in pixels: 14 pt at 3x.
const THUMB_PX: u32 = 42;

/// The pending image `id`'s chip thumbnail (the image itself when it does
/// not decode).
fn thumbnail(id: &str) -> Option<SharedString> {
    if let Some(uri) = URIS.with_borrow(|m| m.get(id).cloned()) {
        return Some(uri);
    }
    let img = store::pending_image(id)?;
    let uri = SharedString::from(small(&img.data).unwrap_or_else(|| format!("data:{};base64,{}", img.mime, img.data)));
    URIS.with_borrow_mut(|m| m.insert(id.to_owned(), uri.clone()));
    Some(uri)
}

/// The image (base64) filling a `THUMB_PX` square, as a PNG data URI.
fn small(data: &str) -> Option<String> {
    use base64::Engine;
    let b64 = base64::engine::general_purpose::STANDARD;
    let bytes = b64.decode(data).ok()?;
    let thumb = image::load_from_memory(&bytes).ok()?.resize_to_fill(THUMB_PX, THUMB_PX, image::imageops::FilterType::Triangle);
    let mut png = std::io::Cursor::new(Vec::new());
    thumb.write_to(&mut png, image::ImageFormat::Png).ok()?;
    Some(format!("data:image/png;base64,{}", b64.encode(png.into_inner())))
}

/// The pending image `id` at full size, for its preview.
fn full_image(id: &str) -> Option<SharedString> {
    let img = store::pending_image(id)?;
    Some(SharedString::from(format!("data:{};base64,{}", img.mime, img.data)))
}

/// Forgets the data URIs of images no composer holds any more.
pub(crate) fn forget_images_except(known: &std::collections::HashSet<String>) {
    URIS.with_borrow_mut(|m| m.retain(|id, _| known.contains(id)));
}

/// The chip on preview: its composer and id.
#[derive(Clone, Default)]
struct Preview(Option<(String, String)>);

fn open_preview(composer: &str, id: &str) {
    if MOBILE && store::pending_image(id).is_none() {
        // A file on the phone: a sheet, as the composer's other menus.
        file_sheet(composer.to_owned(), id.to_owned());
        return;
    }
    dismiss_keyboard();
    global::<Preview>().update(|p| p.0 = Some((composer.to_owned(), id.to_owned())));
}

fn close_preview() {
    global::<Preview>().update(|p| p.0 = None);
}

/// Deletes the chip `id` from composer `composer` (its attachment goes
/// with it: the store detaches what no chip names).
fn remove(composer: &str, id: &str) {
    if let Some(session) = composer_doc::existing(composer) {
        composer_doc::remove_chips(&session, |x| x == id);
    }
}

/// The previews of composer `key`'s chips, presented over the window.
pub(crate) fn preview_layer(key: Memo<Option<String>>) -> impl IntoElement {
    let shown = computed(move || {
        let (composer, id) = global::<Preview>().with(|p| p.0.clone())?;
        (key.get().as_deref() == Some(composer.as_str())).then_some((composer, id))
    });
    view().child(dynamic(move || {
        let (composer, id) = shown.get()?;
        Some(untracked(move || {
            if let Some(img) = store::pending_image(&id) {
                image_preview(&id, img.name).into_element()
            } else if let Some(file) = store::pending_file(&id) {
                file_card(composer, file).into_element()
            } else {
                // Its attachment went (sent, failed): nothing to show.
                close_preview();
                view().into_element()
            }
        }))
    }))
}

/// A full-window backdrop that closes the preview when pressed.
fn backdrop(alpha: f32) -> nsh_base::ButtonElement {
    button()
        .on_press(close_preview)
        .accessibility_label(t("Close"))
        .absolute()
        .top(px(0.0))
        .bottom(px(0.0))
        .left(px(0.0))
        .right(px(0.0))
        .bg(AdaptiveColor::rgb(0x000000, 0x000000).opacity(alpha))
}

/// The image, fit to the window over a dim; a press anywhere or Escape
/// closes it. The dialog fades in and out; the image grows into place.
fn image_preview(id: &str, name: String) -> impl IntoElement {
    let grow = animated_value(0.94);
    grow.spring(1.0, Spring { stiffness: 380.0, damping: 30.0, ..Spring::default() });
    let uri = full_image(id);
    dialog().animation(ModalAnimation::Fade).on_cancel(close_preview).flex_1().child(
        view()
            .flex_1()
            .child(backdrop(0.78))
            .child(
                button()
                    .on_press(close_preview)
                    .accessibility_label(name.clone())
                    .flex_1()
                    .items_center()
                    .justify_center()
                    .gap(px(S3))
                    .px(px(if MOBILE { S4 } else { 48.0 }))
                    .pt_safe_area_plus(px(if MOBILE { S4 } else { 48.0 }))
                    .pb_safe_area_plus(px(if MOBILE { S4 } else { 32.0 }))
                    .bind_scale(&grow)
                    .when_some(uri, |v, uri| v.child(image_el(uri).flex_1().w_full().object_fit(ResizeMode::Contain)))
                    .child(text(name).text_size(px(TEXT_SM)).text_color(AdaptiveColor::rgb(0xffffff, 0xffffff)).number_of_lines(1)),
            ),
    )
}

/// A file's type, size and upload state on a card, with Remove.
fn file_card(composer: String, file: store::PendingFile) -> impl IntoElement {
    let grow = animated_value(0.96);
    grow.spring(1.0, Spring { stiffness: 380.0, damping: 30.0, ..Spring::default() });
    let id = file.id.clone();
    let uploading = computed(move || store::pending_file(&id).is_none_or(|f| f.uploading()));
    let row = |label: &'static str, value: String| {
        view()
            .flex_row()
            .gap(px(S3))
            .child(text(t(label)).text_size(px(TEXT_BASE)).text_color(FG_MUTED).w(px(64.0)))
            .child(text(value).text_size(px(TEXT_BASE)).text_color(FG).flex_1().number_of_lines(2).selectable(true))
    };
    let id = file.id.clone();
    dialog().animation(ModalAnimation::Fade).on_cancel(close_preview).flex_1().child(
        view()
            .flex_1()
            .items_center()
            .justify_center()
            .child(backdrop(0.32))
            .child(
                glass_strong(view())
                    .w(px(360.0))
                    .max_w(relative(0.92))
                    .p(px(S6))
                    .gap(px(S4))
                    .rounded(px(22.0))
                    .bind_scale(&grow)
                    .child(
                        view()
                            .flex_row()
                            .items_center()
                            .gap(px(S3))
                            .child(
                                view()
                                    .w(px(36.0))
                                    .h(px(36.0))
                                    .rounded(px(R_LG))
                                    .bg(tint(ACCENT, 0.08))
                                    .items_center()
                                    .justify_center()
                                    .child(icon(Icon::FileText, 18.0).text_color(ACCENT)),
                            )
                            .child(text(file.name.clone()).text_size(px(TEXT_LG)).font_weight(FontWeight(500)).flex_1().number_of_lines(2)),
                    )
                    .child(
                        view()
                            .gap(px(S2))
                            .child(row("Type", if file.mime.is_empty() { t("Unknown").into() } else { file.mime.clone() }))
                            .child(row("Size", format_size(file.size)))
                            .child(row("Status", String::new()).child(dynamic(move || {
                                let up = uploading.get();
                                Some(untracked(move || {
                                    text(if up { t("Uploading…") } else { t("Ready to send") })
                                        .text_size(px(TEXT_BASE))
                                        .text_color(if up { FG_MUTED } else { SUCCESS })
                                }))
                            }))),
                    )
                    .child(
                        view()
                            .flex_row()
                            .justify_end()
                            .gap(px(S2))
                            .child(btn(t("Remove file"), Variant::Secondary, Sz::Md, Some(Icon::Trash2), move || {
                                close_preview();
                                remove(&composer, &id);
                            }))
                            .child(btn(t("Close"), Variant::Primary, Sz::Md, None, close_preview)),
                    ),
            ),
    )
}

/// The phone's file preview: a sheet with its facts and Remove.
fn file_sheet(composer: String, id: String) {
    let Some(file) = store::pending_file(&id) else { return };
    let kind = if file.mime.is_empty() { t("Unknown").to_owned() } else { file.mime.clone() };
    let status = if file.uploading() { t("Uploading…") } else { t("Ready to send") };
    show_sheet(Some(file.name.clone()), vec![
        SheetItem::new(format!("{kind} · {}", format_size(file.size)), || {}).icon(Icon::FileText),
        SheetItem::new(status, || {}).icon(if file.uploading() { Icon::Upload } else { Icon::CircleCheck }),
        SheetItem::destructive(t("Remove file"), move || remove(&composer, &id)).icon(Icon::Trash2),
    ]);
}

/// A size for people: bytes, KB or MB.
pub(crate) fn format_size(bytes: usize) -> String {
    match bytes {
        b if b < 1024 => format!("{b} B"),
        b if b < 1024 * 1024 => format!("{:.0} KB", b as f64 / 1024.0),
        b => format!("{:.1} MB", b as f64 / (1024.0 * 1024.0)),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn sizes_read_as_people_say_them() {
        assert_eq!(super::format_size(2048), "2 KB");
        assert_eq!(super::format_size(3 * 1024 * 1024 + 300 * 1024), "3.3 MB");
    }
}
