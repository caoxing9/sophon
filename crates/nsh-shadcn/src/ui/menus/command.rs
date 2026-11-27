//! [`Command`], after shadcn/ui's `command.tsx` (new-york-v4, on cmdk), on
//! nsh-base's `Command`: a search field over the items, filtered natively
//! by label and keywords, in a virtualized list under their groups'
//! headings; the keys move the highlight, Return or a press chooses.
//!
//! shadcn composes `Command`, `CommandInput`, `CommandList`,
//! `CommandEmpty`, `CommandGroup`, `CommandItem`, `CommandShortcut` and
//! `CommandSeparator`; nsh-base's command takes its items as data, so here
//! they are one component: `items` are the `CommandItem`s ([`CommandItem`]:
//! a label, an icon, a shortcut's text), an item's `group` its
//! `CommandGroup`'s heading, `placeholder` the input's, `empty` the
//! `CommandEmpty`'s text; a separator comes between two groups shown, as
//! cmdk's `CommandSeparator` between groups. The app's `class` is the
//! root's (`rounded-lg border shadow-md`). Changes from the classes:
//!
//! - nsh-base's list has its rows flat (a heading, items, a separator),
//!   with no view per group: `CommandGroup`'s `p-1` is `mx-1` on each row,
//!   `mt-1` on a heading and a separator (the space above a group and
//!   below the one before) and `pb-1` on the list (below the last group);
//!   `[&_[cmdk-group-heading]]:…` are the heading's own classes. The
//!   separator's `-mx-1` is left out: the list's rows already span it.
//! - `data-[selected=true]:` (cmdk's highlighted item) is `focus:`, which
//!   nsh-base projects on the highlighted row while the field keeps the
//!   keyboard focus; `data-[disabled=true]:` is `disabled:`.
//! - `[&_svg]:…` size and color an item's icon; the search icon's classes
//!   are its own. A row adds `flex-col items-stretch` so the content
//!   nsh-base renders in it fills the row (its own view is the row's
//!   `flex items-center gap-2`), which a shortcut's `ml-auto` needs.
//! - The list's `scroll-py-1 overflow-x-hidden overflow-y-auto` are
//!   nsh-base's scroll view (NSH has no `scroll-py-*`); `max-h-[300px]`
//!   stays.
//! - The input adds `shrink`: a CSS flex item shrinks by default, an NSH
//!   one does not, and the input's `w-full` beside the search icon would
//!   run past the bar.
//! - The root's `h-full` is left out: NSH resolves a percentage height
//!   against a parent its content sizes (as Yoga does), so the command
//!   grew its list to its `max-h-[300px]` where a browser keeps it at its
//!   rows' height. An app sizes it with `h-*` where it should fill.
//! - `CommandDialog` is not part of this family (it is a `Dialog`, the
//!   overlays family's): put a `Command` in one.

use nsh_base::CommandEntry;
use nsh_core::prelude::*;

use crate::icon::{IconName, icon};

const ROOT: &str = "flex w-full flex-col overflow-hidden rounded-md bg-popover \
    text-popover-foreground";

const INPUT_WRAPPER: &str = "flex h-9 items-center gap-2 border-b border-border px-3";
const SEARCH_ICON: &str = "size-4 shrink-0 opacity-50";
const INPUT: &str = "flex h-10 w-full shrink rounded-md bg-transparent py-3 text-sm outline-hidden \
    placeholder:text-muted-foreground disabled:cursor-not-allowed disabled:opacity-50";

const LIST: &str = "max-h-[300px] pb-1";

const EMPTY: &str = "py-6 text-center text-sm";

/// `CommandGroup`'s heading (`[&_[cmdk-group-heading]]:…`), with the
/// group's padding.
const HEADING: &str = "mx-1 mt-1 px-2 py-1.5 text-xs font-medium text-muted-foreground";

const SEPARATOR: &str = "mt-1 h-px bg-border";

const ITEM: &str = "relative flex cursor-default items-center gap-2 rounded-sm px-2 py-1.5 text-sm \
    outline-hidden select-none disabled:pointer-events-none disabled:opacity-50 focus:bg-accent \
    focus:text-accent-foreground mx-1 flex-col items-stretch";

const ITEM_CONTENT: &str = "flex items-center gap-2";
const ICON: &str = "pointer-events-none shrink-0 size-4 text-muted-foreground";
const SHORTCUT: &str = "ml-auto text-xs tracking-widest text-muted-foreground";

/// An item of a [`Command`]: shadcn's `CommandItem` as data.
#[derive(Debug, Clone, Default, PartialEq, JsObject)]
#[non_exhaustive]
pub struct CommandItem {
    /// What identifies it in `onSelect`: unique among the items.
    pub id: SharedString,
    /// Its label: what its row shows and the query matches.
    pub label: SharedString,
    /// A Lucide icon before the label.
    #[prop(skip_none)]
    pub icon: Option<IconName>,
    /// `CommandShortcut`'s text after the label (`⌘P`).
    #[prop(skip_none)]
    pub shortcut: Option<SharedString>,
    /// More words the query matches.
    #[prop(default)]
    pub keywords: Vec<SharedString>,
    /// Its `CommandGroup`'s heading: adjacent items of one group show
    /// under it.
    #[prop(skip_none)]
    pub group: Option<SharedString>,
    /// It cannot be chosen; half opacity.
    #[prop(default)]
    pub disabled: bool,
}

impl CommandItem {
    /// An item of `id`, labelled `label`.
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            ..Self::default()
        }
    }

    /// Its icon.
    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    /// Its shortcut's text.
    pub fn shortcut(mut self, keys: impl Into<SharedString>) -> Self {
        self.shortcut = Some(keys.into());
        self
    }

    /// Its group's heading.
    pub fn group(mut self, heading: impl Into<SharedString>) -> Self {
        self.group = Some(heading.into());
        self
    }

    /// Whether it cannot be chosen.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// nsh-base's item.
    fn base(&self) -> nsh_base::CommandItem {
        let mut item = nsh_base::CommandItem::new(self.id.clone(), self.label.clone())
            .keywords(self.keywords.iter().cloned());
        item.group = self.group.clone();
        item.disabled = self.disabled;
        item
    }
}

/// The props of [`Command`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct CommandProps {
    /// The items, in order.
    #[prop(default)]
    pub items: Prop<Vec<CommandItem>>,
    /// `CommandInput`'s placeholder.
    pub placeholder: Prop<Option<SharedString>>,
    /// `CommandEmpty`'s text, while no item matches.
    pub empty: Prop<Option<SharedString>>,
    /// The search field takes the keyboard focus once shown.
    #[prop(default)]
    pub auto_focus: Prop<bool>,
    /// What screen readers call the list.
    pub accessibility_label: Prop<Option<SharedString>>,
}

/// The events of a [`Command`].
#[derive(Debug, Clone, PartialEq, Event)]
pub enum CommandEvents {
    /// The user chose an item: its id.
    #[args(id)]
    Select(SharedString),
    /// The user typed in the search field: the query.
    #[args(query)]
    QueryChange(SharedString),
}

/// shadcn's Command: a search field over a list of commands.
#[component]
pub fn Command(props: CommandProps, cx: Scope<CommandEvents>) -> impl IntoElement {
    let items = props.items;
    let base = computed(move || items.with(|items| items.iter().map(CommandItem::base).collect::<Vec<_>>()));
    let empty = props.empty;
    nsh_base::command()
        .app_class(Some(SharedString::from(ROOT)))
        .items(base)
        .placeholder(props.placeholder)
        .auto_focus(props.auto_focus)
        .accessibility_label(props.accessibility_label)
        .search_bar_class(INPUT_WRAPPER)
        .search_prefix(icon(IconName::Search).app_class(Some(SharedString::from(SEARCH_ICON))))
        .search_class(INPUT)
        .list_class(LIST)
        .item_class(ITEM)
        .group_class(HEADING)
        .separator_class(SEPARATOR)
        .render_item(move |entry: &CommandEntry| {
            let item = items.with_untracked(|items| items.iter().find(|i| i.id == entry.id).cloned());
            let (glyph, shortcut) = item.map(|i| (i.icon, i.shortcut)).unwrap_or_default();
            view()
                .app_class(Some(SharedString::from(ITEM_CONTENT)))
                .child(glyph.map(|name| icon(name).app_class(Some(SharedString::from(ICON)))))
                .child(text(entry.label.clone()))
                .child(shortcut.map(|keys| text(keys).app_class(Some(SharedString::from(SHORTCUT)))))
        })
        .empty(dynamic(move || {
            empty
                .get()
                .map(|words| text(words).app_class(Some(SharedString::from(EMPTY))).into_element())
        }))
        .on_select(move |id: &SharedString| cx.emit(CommandEvents::Select(id.clone())))
        .on_query_change(move |query: &SharedString| cx.emit(CommandEvents::QueryChange(query.clone())))
}

pub(crate) fn register(registry: &mut Registry) {
    registry.register::<Command>();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_is_known_to_nsh() {
        super::super::menu_common::assert_known(
            [
                ROOT, INPUT_WRAPPER, SEARCH_ICON, INPUT, LIST, EMPTY, HEADING, SEPARATOR, ITEM,
                ITEM_CONTENT, ICON, SHORTCUT,
            ]
            .iter()
            .map(|c| crate::cn::cn([*c]).to_string()),
        );
    }
}
