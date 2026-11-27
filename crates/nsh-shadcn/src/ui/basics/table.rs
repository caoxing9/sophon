//! [`Table`] and its parts, after shadcn/ui's `table.tsx` (new-york-v4),
//! on `nsh_base`'s `Table` parts (the table, row group, row, column header
//! and cell roles for screen readers).
//!
//! NSH has no table layout: a row is a flex row of its cells, so a column
//! is as wide as the app makes each of its cells, the same width class on
//! the column's head and cells (`w-[100px]`, `flex-1`), where HTML sizes a
//! column by its widest cell. A cell that spans columns (`colSpan`) is as
//! wide as those columns together.
//!
//! The classes are shadcn's, word for word, except:
//!
//! - `border-b` / `border-t` are `border-b border-border` /
//!   `border-t border-border` (shadcn's global `* { @apply border-border }`).
//! - The table's container (`relative w-full overflow-x-auto`) and the
//!   table are one view, `relative w-full text-sm`: NSH has no
//!   `overflow-x-auto` (a wide table goes in the app's horizontal
//!   `scroll()`), nor `caption-bottom` (the app places the caption last).
//! - `TableHeader`'s `[&_tr]:border-b` is the rows' own `border-b`.
//! - `TableBody`'s `[&_tr:last-child]:border-0` and `TableFooter`'s
//!   `[&>tr]:last:border-b-0` drop the last row's border: NSH cannot select
//!   the last child, so the group clips it (`overflow-hidden` around its
//!   rows, pulled up a point with `-mb-px`), which draws the same pixels.
//! - `TableRow`'s `data-[state=selected]:bg-muted` is the `selected`
//!   prop's; `has-aria-expanded:bg-muted/50` (a row holding an expanded
//!   control) is left out: NSH has no `:has()`.
//! - `TableHead` and `TableCell` are views, not table cells: `align-middle`
//!   centers their content with `justify-center` (they lay it out in a
//!   column). `[&:has([role=checkbox])]:pr-0` and
//!   `[&>[role=checkbox]]:translate-y-[2px]` are the app's `pr-0` and the
//!   checkbox's `translate-y-[2px]`.
//! - `TableCaption` adds `text-center`, the browser's own style of a
//!   `<caption>`.

use nsh_core::prelude::*;

use super::cls;
use crate::cn::cn;

const TABLE: &str = "relative w-full text-sm";
/// `[&_tr:last-child]:border-0`, `[&>tr]:last:border-b-0`: the group clips
/// its last row's border.
const CLIP: &str = "overflow-hidden";
const CLIPPED: &str = "-mb-px";
const FOOTER: &str = "border-t border-border bg-muted/50 font-medium";
const ROW: &str = "border-b border-border transition-colors hover:bg-muted/50";
/// `data-[state=selected]:bg-muted`.
const ROW_SELECTED: &str = "bg-muted";
const HEAD: &str = "h-10 px-2 text-left align-middle font-medium whitespace-nowrap text-foreground";
const CELL: &str = "p-2 align-middle whitespace-nowrap";
/// `align-middle` in a view that lays out its content in a column.
const MIDDLE: &str = "justify-center";
const CAPTION: &str = "mt-4 text-sm text-muted-foreground";
/// A `<caption>`'s own centering.
const CAPTION_CENTER: &str = "text-center";

/// The props of the parts that hold only children: [`Table`],
/// [`TableHeader`], [`TableBody`] and [`TableFooter`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct TablePartProps {
    /// What the part holds.
    pub children: Prop<Children>,
}

/// shadcn's Table: rows of cells under a header, small text, as wide as
/// its container.
#[component]
pub fn Table(props: TablePartProps, _cx: Scope<NoEvent>) -> impl IntoElement {
    nsh_base::table()
        .app_class(cls(TABLE))
        .child(props.children)
}

/// shadcn's TableHeader: the rows of column headers.
#[component]
pub fn TableHeader(props: TablePartProps, _cx: Scope<NoEvent>) -> impl IntoElement {
    nsh_base::table_header().child(props.children)
}

/// shadcn's TableBody: the rows of data, the last without a border.
#[component]
pub fn TableBody(props: TablePartProps, _cx: Scope<NoEvent>) -> impl IntoElement {
    nsh_base::table_body()
        .app_class(cls(CLIP))
        .child(view().app_class(cls(CLIPPED)).child(props.children))
}

/// shadcn's TableFooter: a muted row group under the body (a total).
#[component]
pub fn TableFooter(props: TablePartProps, _cx: Scope<NoEvent>) -> impl IntoElement {
    nsh_base::table_footer()
        .app_class(cls(cn([FOOTER, CLIP])))
        .child(view().app_class(cls(CLIPPED)).child(props.children))
}

/// The props of [`TableRow`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct TableRowProps {
    /// The row is selected (shadcn's `data-state="selected"`): a muted fill.
    #[prop(default)]
    pub selected: Prop<bool>,
    /// Its cells.
    pub children: Prop<Children>,
}

/// shadcn's TableRow: cells side by side over a border, muted while
/// hovered.
#[component]
pub fn TableRow(props: TableRowProps, _cx: Scope<NoEvent>) -> impl IntoElement {
    nsh_base::table_row()
        .app_class(move || {
            cls(cn([
                ROW,
                if props.selected.get() {
                    ROW_SELECTED
                } else {
                    ""
                },
            ]))
        })
        .child(props.children)
}

/// The props of [`TableHead`] and [`TableCell`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct TableCellProps {
    /// The cell's text.
    pub text: Prop<Option<SharedString>>,
    /// Content after the text (a badge, a checkbox).
    pub children: Prop<Children>,
}

/// shadcn's TableHead: a column's header, 40 points tall.
#[component]
pub fn TableHead(props: TableCellProps, _cx: Scope<NoEvent>) -> impl IntoElement {
    let label = props.text;
    nsh_base::table_head()
        .app_class(cls(cn([HEAD, MIDDLE])))
        .child(dynamic(move || label.get().map(|t| text(t).into_element())))
        .child(props.children)
}

/// shadcn's TableCell: 8 points of padding around its content.
#[component]
pub fn TableCell(props: TableCellProps, _cx: Scope<NoEvent>) -> impl IntoElement {
    let label = props.text;
    nsh_base::table_cell()
        .app_class(cls(cn([CELL, MIDDLE])))
        .child(dynamic(move || label.get().map(|t| text(t).into_element())))
        .child(props.children)
}

/// The props of [`TableCaption`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct TableCaptionProps {
    /// The caption.
    pub caption: Prop<Option<SharedString>>,
    /// Content after the caption.
    pub children: Prop<Children>,
}

/// shadcn's TableCaption: a muted, centered line under the table (place it
/// last).
#[component]
pub fn TableCaption(props: TableCaptionProps, _cx: Scope<NoEvent>) -> impl IntoElement {
    let caption = props.caption;
    nsh_base::table_caption()
        .app_class(cls(cn([CAPTION, CAPTION_CENTER])))
        .child(dynamic(move || {
            caption.get().map(|t| text(t).into_element())
        }))
        .child(props.children)
}

pub(crate) fn register(registry: &mut Registry) {
    registry.register::<Table>();
    registry.register::<TableHeader>();
    registry.register::<TableBody>();
    registry.register::<TableFooter>();
    registry.register::<TableRow>();
    registry.register::<TableHead>();
    registry.register::<TableCell>();
    registry.register::<TableCaption>();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_is_known_to_nsh() {
        super::super::assert_known([
            TABLE,
            CLIP,
            CLIPPED,
            FOOTER,
            ROW,
            ROW_SELECTED,
            HEAD,
            CELL,
            MIDDLE,
            CAPTION,
            CAPTION_CENTER,
            // What an app adds: a checkbox's cell and the checkbox in it.
            "pr-0 translate-y-[2px]",
        ]);
    }
}
