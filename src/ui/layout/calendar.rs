//! [`Calendar`], after shadcn/ui's `calendar.tsx` (new-york-v4), which
//! styles react-day-picker; here on `nsh_base`'s `Calendar` (the days of
//! the month, picking a date or a range, the keys, the month and year
//! views).
//!
//! The classes are shadcn's `classNames`, word for word, put on the parts
//! `nsh_base` draws, except where NSH has no such class or the parts
//! differ:
//!
//! - `--cell-size` (`--spacing(8)`) is written out: `size-8`, `h-8`,
//!   `min-w-8`.
//! - `nsh_base` gives every item one class (`itemClass`): a day cell's
//!   classes (react-day-picker's `day`, `today`, `outside`, `range_*`)
//!   are its `data-today:`, `data-outside:`, `data-range-*:` and
//!   `selected:` classes, and what is inside each item (the day's button,
//!   a weekday's label, the arrows, the caption) is drawn by kind, with
//!   `group-*:` classes for the cell's states (`group-hover:` for the
//!   ghost button's `hover:`, `group-selected:` for
//!   `data-[selected-single=true]:`).
//! - The header is `nsh_base`'s row of Previous, the caption and Next
//!   (react-day-picker's `nav` is placed over the caption): the arrows
//!   go to its ends with `first:mr-auto` / `last:ml-auto`, the caption
//!   between. With several months their captions sit side by side in the
//!   middle rather than over each month.
//! - The caption's month and year are `nsh_base`'s toggles, which open its
//!   month and year views (react-day-picker's caption is a label); with
//!   `caption_layout="dropdown"` they are drawn as shadcn's dropdowns
//!   (`dropdown_root`, `caption_label`), and they open those views in
//!   place of a native `<select>`.
//! - `nsh_base` gives the weekdays' row the weeks' class (`mt-2 flex
//!   w-full`, its `flex` left out: `nsh_base` lays the rows out and hides
//!   the weeks a month does not use); the month's `-mt-2` takes the
//!   weekdays' margin back.
//! - The weekday's `text-[0.8rem]` inherits the page's `leading-normal`
//!   in a browser; here it says so.
//! - The month and year views (`nsh_base`'s, not react-day-picker's) are
//!   drawn as ghost buttons, the shown one `bg-primary`.
//! - `showOutsideDays={false}` hides them (`data-outside:invisible`).

use nsh_base::{CalendarEntry, CalendarItemKind, Date};
use nsh_core::prelude::*;

use crate::cn::cn;
use crate::icon::{IconName, icon};

/// How the caption shows the month: shadcn's `captionLayout`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PropEnum)]
#[non_exhaustive]
pub enum CalendarCaptionLayout {
    /// "June 2025".
    #[default]
    Label,
    /// The month and the year in dropdowns.
    Dropdown,
}

/// What a press picks: react-day-picker's `mode`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PropEnum)]
#[non_exhaustive]
pub enum CalendarMode {
    /// One date.
    #[default]
    Single,
    /// A start and an end.
    Range,
}

const MONTHS: [&str; 12] = [
    "January", "February", "March", "April", "May", "June", "July", "August", "September",
    "October", "November", "December",
];
const WEEKDAYS: [&str; 7] = ["Su", "Mo", "Tu", "We", "Th", "Fr", "Sa"];

/// shadcn's root classes (`root`, the calendar's own).
pub(crate) const ROOT: &str = "bg-background p-3 w-fit flex flex-col gap-4";
/// shadcn's `month_caption` (with the `nav` at its ends).
pub(crate) const HEADER: &str = "flex h-8 w-full items-center justify-center";
/// shadcn's `dropdowns`, around the caption in the dropdown layout.
pub(crate) const HEADER_DROPDOWN: &str =
    "flex h-8 w-full items-center justify-center text-sm font-medium";
/// shadcn's `months`.
pub(crate) const BODY: &str = "relative flex flex-col gap-4 md:flex-row";
/// shadcn's `month` (and `month_grid`): a column of the weekdays and
/// weeks, `-mt-2` taking back the weekdays' `mt-2`.
pub(crate) const MONTH: &str = "flex w-full flex-col -mt-2";
/// shadcn's `week` (`nsh_base` gives the weekdays the same row class),
/// without `flex`: the row is `nsh_base`'s, and a `display` of the app's
/// would show the sixth week it hides in a month of five.
pub(crate) const ROW: &str = "mt-2 w-full";

/// Every item's classes: react-day-picker's day cell (`day`, `today`,
/// `outside`, `range_start`, `range_middle`, `range_end`, `disabled`), and
/// the `group` its content reads.
pub(crate) fn item_classes(show_outside_days: bool) -> SharedString {
    cn([
        "group relative flex items-center justify-center p-0 text-center select-none \
         first:mr-auto last:ml-auto",
        "data-today:rounded-md data-today:bg-accent data-today:text-accent-foreground",
        "data-outside:text-muted-foreground",
        "disabled:text-muted-foreground disabled:opacity-50",
        "data-range-start:rounded-l-md data-range-start:bg-accent",
        "data-range-middle:rounded-none",
        "data-range-end:rounded-r-md data-range-end:bg-accent",
        // `[&:first-child[data-selected=true]_button]:rounded-l-md` and
        // `last-child`'s: a range's band is rounded at the week's ends.
        "overflow-hidden first:rounded-l-md last:rounded-r-md",
        "focus-visible:z-10 focus-visible:rounded-md focus-visible:ring-[3px] focus-visible:ring-ring/50",
        if show_outside_days { "" } else { "data-outside:invisible" },
    ])
}

/// The ghost button's classes (`buttonVariants({ variant: "ghost" })`),
/// following the item's states.
const GHOST: &str = "inline-flex shrink-0 items-center justify-center gap-2 rounded-md text-sm \
    font-medium whitespace-nowrap transition-all outline-none group-hover:bg-accent \
    group-hover:text-accent-foreground dark:group-hover:bg-accent/50";

/// shadcn's `button_previous` / `button_next`.
pub(crate) fn nav_classes() -> SharedString {
    cn([GHOST, "size-8 p-0 select-none group-disabled:opacity-50"])
}

/// shadcn's `CalendarDayButton`.
pub(crate) fn day_classes() -> SharedString {
    cn([
        GHOST,
        "flex aspect-square w-full min-w-8 flex-col gap-1 leading-none font-normal \
         group-selected:bg-primary group-selected:text-primary-foreground \
         dark:group-hover:text-accent-foreground",
        "group-data-range-start:rounded-md group-data-range-start:rounded-l-md \
         group-data-range-start:bg-primary group-data-range-start:text-primary-foreground",
        "group-data-range-end:rounded-md group-data-range-end:rounded-r-md \
         group-data-range-end:bg-primary group-data-range-end:text-primary-foreground",
        "group-data-range-middle:rounded-none group-data-range-middle:bg-accent \
         group-data-range-middle:text-accent-foreground",
    ])
}

/// shadcn's `weekday`.
pub(crate) const WEEKDAY: &str =
    "flex-1 rounded-md text-[0.8rem] leading-normal font-normal text-muted-foreground select-none";
/// shadcn's `caption_label` in the label layout.
pub(crate) const CAPTION_LABEL: &str = "font-medium select-none text-sm";
/// shadcn's `dropdown_root` and `caption_label` in the dropdown layout.
pub(crate) const DROPDOWN_ROOT: &str = "relative rounded-md border border-input shadow-xs";
pub(crate) const DROPDOWN_LABEL: &str =
    "font-medium select-none flex h-8 items-center gap-1 rounded-md pr-1 pl-2 text-sm";
pub(crate) const DROPDOWN_CHEVRON: &str = "size-3.5 text-muted-foreground";
/// A month or a year of `nsh_base`'s month and year views.
pub(crate) fn picker_classes() -> SharedString {
    cn([GHOST, "h-8 w-full px-2 group-selected:bg-primary group-selected:text-primary-foreground"])
}

/// What an item shows, by kind.
fn render(entry: &CalendarEntry, layout: CalendarCaptionLayout) -> AnyElement {
    let class = |c: &str| Some(SharedString::from(c.to_owned()));
    let month_name = |m: i32| MONTHS[((m - 1).clamp(0, 11)) as usize];
    match entry.kind {
        CalendarItemKind::Previous | CalendarItemKind::Next => {
            let name = match entry.kind {
                CalendarItemKind::Previous => IconName::ChevronLeft,
                _ => IconName::ChevronRight,
            };
            view()
                .app_class(Some(nav_classes()))
                .child(icon(name).app_class(class("size-4")))
                .into_element()
        }
        CalendarItemKind::MonthToggle | CalendarItemKind::YearToggle => {
            let month = entry.kind == CalendarItemKind::MonthToggle;
            let gap = if month { "" } else { "ml-1.5" };
            match layout {
                CalendarCaptionLayout::Label => {
                    // A space between the month and the year.
                    let (label, space) = if month {
                        (month_name(entry.value).to_owned(), "")
                    } else {
                        (entry.value.to_string(), "ml-1")
                    };
                    text(label).app_class(Some(cn([CAPTION_LABEL, space]))).into_element()
                }
                CalendarCaptionLayout::Dropdown => {
                    let label = if month {
                        month_name(entry.value)[..3].to_owned()
                    } else {
                        entry.value.to_string()
                    };
                    view()
                        .app_class(Some(cn([DROPDOWN_ROOT, gap])))
                        .child(
                            view()
                                .app_class(class(DROPDOWN_LABEL))
                                .child(text(label))
                                .child(icon(IconName::ChevronDown).app_class(class(DROPDOWN_CHEVRON))),
                        )
                        .into_element()
                }
            }
        }
        CalendarItemKind::Weekday => text(WEEKDAYS[(entry.value.clamp(0, 6)) as usize])
            .app_class(class(WEEKDAY))
            .into_element(),
        CalendarItemKind::Day => view()
            .app_class(Some(day_classes()))
            .child(text(entry.value.to_string()))
            .into_element(),
        CalendarItemKind::Month => view()
            .app_class(Some(picker_classes()))
            .child(text(month_name(entry.value)[..3].to_owned()))
            .into_element(),
        _ => view()
            .app_class(Some(picker_classes()))
            .child(text(entry.value.to_string()))
            .into_element(),
    }
}

/// The props of [`Calendar`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct CalendarProps {
    /// `single` (a date) or `range` (a start and an end).
    #[prop(default)]
    pub mode: Prop<CalendarMode>,
    /// The selected date or range at first (react-day-picker's
    /// `defaultSelected`; its `selected`, held by the app, is
    /// `nsh_base::Calendar`'s `value`).
    #[prop(default)]
    pub default_selected: Prop<Date>,
    /// `label` ("June 2025", the default) or `dropdown`.
    #[prop(default)]
    pub caption_layout: Prop<CalendarCaptionLayout>,
    /// The days of the months around fill the first and last weeks
    /// (default true).
    #[prop(default = true)]
    pub show_outside_days: Prop<bool>,
    /// How many months show side by side.
    #[prop(default = 1)]
    pub number_of_months: Prop<u32>,
    /// The day the weeks start on, `0` Sunday.
    #[prop(default)]
    pub week_starts_on: Prop<u32>,
    /// The dates that cannot be picked (react-day-picker's `disabled`).
    pub disabled_matcher: Prop<Option<nsh_base::Matcher>>,
}

/// The events of [`Calendar`].
#[derive(Debug, Clone, PartialEq, Event)]
pub enum CalendarEvent {
    /// A date, or a range's end, was picked (react-day-picker's
    /// `onSelect`).
    #[args(selected)]
    Select(Date),
}

/// shadcn's Calendar: a month to pick a date or a range of dates from.
#[component]
pub fn Calendar(props: CalendarProps, cx: Scope<CalendarEvent>) -> impl IntoElement {
    let layout = props.caption_layout;
    let outside = props.show_outside_days;
    nsh_base::calendar()
        .range(move || props.mode.get() == CalendarMode::Range)
        .default_value(props.default_selected)
        .number_of_months(props.number_of_months)
        .first_day_of_week(props.week_starts_on)
        .disabled_matcher(props.disabled_matcher)
        .previous_accessibility_label(Some("Go to the Previous Month".into()))
        .next_accessibility_label(Some("Go to the Next Month".into()))
        .header_class(move || {
            Some(SharedString::from(match layout.get() {
                CalendarCaptionLayout::Label => HEADER,
                CalendarCaptionLayout::Dropdown => HEADER_DROPDOWN,
            }))
        })
        .body_class(Some(SharedString::from(BODY)))
        .month_class(Some(SharedString::from(MONTH)))
        .row_class(Some(SharedString::from(ROW)))
        .item_class(move || Some(item_classes(outside.get())))
        .render_item(move |entry: &CalendarEntry| render(entry, layout.get()))
        .on_change(move |date: &Date| cx.emit(CalendarEvent::Select(*date)))
        .app_class(Some(SharedString::from(ROOT)))
}

pub(crate) fn register(registry: &mut Registry) {
    registry.register::<Calendar>();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_is_known_to_nsh() {
        let all = [
            SharedString::from(ROOT),
            HEADER.into(),
            HEADER_DROPDOWN.into(),
            BODY.into(),
            MONTH.into(),
            ROW.into(),
            item_classes(true),
            item_classes(false),
            nav_classes(),
            day_classes(),
            WEEKDAY.into(),
            CAPTION_LABEL.into(),
            DROPDOWN_ROOT.into(),
            DROPDOWN_LABEL.into(),
            DROPDOWN_CHEVRON.into(),
            picker_classes(),
            "ml-1 ml-1.5 size-4".into(),
        ];
        let unknown: Vec<_> = all.iter().flat_map(|c| crate::cn::unknown_classes(c)).collect();
        assert!(unknown.is_empty(), "classes NSH does not know: {unknown:?}");
    }
}
