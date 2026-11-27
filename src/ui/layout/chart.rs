//! [`ChartContainer`], [`ChartTooltipContent`] and [`ChartLegendContent`],
//! after shadcn/ui's `chart.tsx` (new-york-v4), which wraps recharts; here
//! on `nsh_base::plot` (the scales, the marks, the hover, its tooltip),
//! with the marks [`chart_bar`], [`chart_area`] and [`chart_line`] painted
//! as shadcn's examples paint recharts' (`fill="var(--color-desktop)"`,
//! `radius={4}`).
//!
//! The classes are shadcn's, word for word, except where NSH has no such
//! class or the parts differ:
//!
//! - `ChartContainer` is recharts' chart too: it holds the rows, the
//!   scales, the grid (`<CartesianGrid vertical={false} />`), the x axis
//!   (`<XAxis tickLine={false} axisLine={false} tickMargin={10} />`), the
//!   tooltip (`<ChartTooltip content={<ChartTooltipContent />} />`) and
//!   the legend (`<ChartLegend content={<ChartLegendContent />} />`) as
//!   props; its children are the marks. It is a column (`flex-col`), the
//!   legend under the plot, as recharts lays its legend out inside the
//!   chart's box.
//! - Its descendant selectors (`[&_.recharts-cartesian-axis-tick_text]:fill-muted-foreground`,
//!   `[&_.recharts-cartesian-grid_line[stroke='#ccc']]:stroke-border/50`,
//!   `[&_.recharts-rectangle.recharts-tooltip-cursor]:fill-muted`,
//!   `[&_.recharts-curve.recharts-tooltip-cursor]:stroke-border`,
//!   `[&_.recharts-dot[stroke='#fff']]:stroke-transparent`) are the
//!   classes of those parts: the axis labels' `text-muted-foreground`,
//!   the grid's `stroke-border/50`, the cursor's `bg-muted` (a band) or
//!   `bg-border` (a line), the hover dot without a ring.
//! - `ChartConfig`'s CSS variables (`--color-desktop`) are the series'
//!   `color`: a theme color (`chart-1`) or a hex color (`#2563eb`), which
//!   the parts name in their classes (`bg-chart-1`, `bg-[#2563eb]`).
//! - recharts' margins (5 points, the x axis 30 high, the legend's height)
//!   are the plot's margins.
//! - A band's cursor (`fill-muted` behind the bars) is a view of the
//!   container's under the marks: `nsh_base`'s cross line is drawn over
//!   them. A point's cursor is the cross line.
//! - The tooltip goes beside the pointer, 10 points from it, as
//!   recharts' does.

use std::sync::Arc;

use nsh_base::plot::{
    BandScale, BarCornerRadii, Curve, Scale, TickFormat, TooltipPlacement, area, bar, grid, line,
    plot, plot_axis,
};
use nsh_core::element::{RowAccessor, Rows};
use nsh_core::prelude::*;

use crate::cn::cn;

/// A series of a chart: shadcn's `ChartConfig` entry.
#[derive(Debug, Clone, Default, PartialEq, JsObject)]
#[non_exhaustive]
pub struct ChartConfigItem {
    /// The field of the rows it shows (`desktop`).
    pub key: SharedString,
    /// What the tooltip and the legend call it (`Desktop`).
    pub label: SharedString,
    /// Its color: a theme color (`chart-1`) or a hex color (`#2563eb`).
    pub color: SharedString,
}

impl ChartConfigItem {
    /// The series `key`, called `label`, in `color`.
    pub fn new(
        key: impl Into<SharedString>,
        label: impl Into<SharedString>,
        color: impl Into<SharedString>,
    ) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            color: color.into(),
        }
    }
}

/// A row of a tooltip: recharts' payload entry.
#[derive(Debug, Clone, Default, PartialEq, JsObject)]
#[non_exhaustive]
pub struct ChartTooltipItem {
    /// What it is called.
    pub name: SharedString,
    /// Its value.
    pub value: f64,
    /// Its color, as [`ChartConfigItem::color`].
    pub color: SharedString,
}

impl ChartTooltipItem {
    /// `name`: `value`, in `color`.
    pub fn new(name: impl Into<SharedString>, value: f64, color: impl Into<SharedString>) -> Self {
        Self {
            name: name.into(),
            value,
            color: color.into(),
        }
    }
}

/// The mark before each tooltip row: shadcn's `indicator`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PropEnum)]
#[non_exhaustive]
pub enum ChartTooltipIndicator {
    /// A square.
    #[default]
    Dot,
    /// A bar as tall as the row.
    Line,
    /// A dashed line.
    Dashed,
}

/// Where a legend is: shadcn's `verticalAlign`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PropEnum)]
#[non_exhaustive]
pub enum ChartLegendAlign {
    /// Over the chart.
    Top,
    /// Under it.
    #[default]
    Bottom,
}

/// The class `prefix-color` for a series' color (`bg-chart-1`,
/// `bg-[#2563eb]`).
pub(crate) fn color_class(prefix: &str, color: &str) -> String {
    if color.starts_with('#') {
        format!("{prefix}-[{color}]")
    } else {
        format!("{prefix}-{color}")
    }
}

/// shadcn's `ChartContainer` classes (a column: see the module doc).
pub(crate) const CONTAINER: &str = "flex aspect-video justify-center text-xs flex-col";
/// The tick labels of the x axis: `fill-muted-foreground`, `tickMargin`.
pub(crate) const TICK: &str = "pt-4 text-xs text-muted-foreground";
/// `CartesianGrid`'s lines.
pub(crate) const GRID: &str = "stroke-border/50";
/// The tooltip cursor over a band, and over a point.
pub(crate) const CURSOR_BAND: &str = "bg-muted";
pub(crate) const CURSOR_LINE: &str = "w-px h-full bg-border";

/// shadcn's `ChartTooltipContent` box.
pub(crate) const TOOLTIP: &str = "grid min-w-[8rem] items-start gap-1.5 rounded-lg border \
    border-border/50 bg-background px-2.5 py-1.5 text-xs shadow-xl";

/// A tooltip row's classes.
pub(crate) fn tooltip_row_classes(indicator: ChartTooltipIndicator) -> SharedString {
    cn([
        "flex w-full flex-wrap items-stretch gap-2",
        if indicator == ChartTooltipIndicator::Dot { "items-center" } else { "" },
    ])
}

/// A tooltip row's indicator, in `color`.
pub(crate) fn indicator_classes(indicator: ChartTooltipIndicator, nest: bool, color: &str) -> SharedString {
    let colors = format!("{} {}", color_class("border", color), color_class("bg", color));
    cn([
        "shrink-0 rounded-[2px]",
        &colors,
        match indicator {
            ChartTooltipIndicator::Dot => "h-2.5 w-2.5",
            ChartTooltipIndicator::Line => "w-1",
            ChartTooltipIndicator::Dashed => "w-0 border-[1.5px] border-dashed bg-transparent",
        },
        if nest && indicator == ChartTooltipIndicator::Dashed { "my-0.5" } else { "" },
    ])
}

/// The label and value of a tooltip row.
pub(crate) fn tooltip_text_classes(nest: bool) -> SharedString {
    cn([
        "flex flex-1 justify-between leading-none",
        if nest { "items-end" } else { "items-center" },
    ])
}

/// shadcn's `ChartLegendContent` classes.
pub(crate) fn legend_classes(align: ChartLegendAlign) -> SharedString {
    cn([
        "flex items-center justify-center gap-4",
        match align {
            ChartLegendAlign::Top => "pb-3",
            ChartLegendAlign::Bottom => "pt-3",
        },
    ])
}
pub(crate) const LEGEND_ITEM: &str = "flex items-center gap-1.5";
pub(crate) const LEGEND_SWATCH: &str = "h-2 w-2 shrink-0 rounded-[2px]";

/// A number as `toLocaleString` writes it in English: thousands grouped.
pub(crate) fn format_value(value: f64) -> String {
    if !value.is_finite() {
        return String::new();
    }
    let negative = value < 0.0;
    let whole = value.abs().trunc() as u64;
    let digits = whole.to_string();
    let mut grouped = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            grouped.push(',');
        }
        grouped.push(c);
    }
    let fraction = value.abs().fract();
    if fraction > 1e-9 {
        let text = format!("{:.3}", fraction);
        grouped.push_str(text.trim_start_matches('0').trim_end_matches('0'));
    }
    if negative { format!("-{grouped}") } else { grouped }
}

/// The props of [`ChartTooltipContent`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct ChartTooltipContentProps {
    /// What the rows are about (the month).
    pub label: Prop<Option<SharedString>>,
    /// The rows: each series' name, value and color.
    #[prop(default)]
    pub items: Prop<Vec<ChartTooltipItem>>,
    /// `dot` (the default), `line` or `dashed`.
    #[prop(default)]
    pub indicator: Prop<ChartTooltipIndicator>,
    /// No label.
    #[prop(default)]
    pub hide_label: Prop<bool>,
    /// No indicators.
    #[prop(default)]
    pub hide_indicator: Prop<bool>,
}

/// shadcn's ChartTooltipContent: the label and a row per series.
#[component]
pub fn ChartTooltipContent(props: ChartTooltipContentProps, _cx: Scope<NoEvent>) -> impl IntoElement {
    view()
        .app_class(Some(SharedString::from(TOOLTIP)))
        .child(dynamic(move || tooltip_body(&props)))
}

fn tooltip_body(props: &ChartTooltipContentProps) -> Vec<AnyElement> {
    let items = props.items.get();
    let indicator = props.indicator.get();
    let label = props.label.get().filter(|_| !props.hide_label.get());
    let nest = items.len() == 1 && indicator != ChartTooltipIndicator::Dot;
    let label_view = |label: &SharedString| {
        text(label.clone())
            .app_class(Some(SharedString::from("font-medium")))
            .into_element()
    };
    let mut out = Vec::new();
    if !nest && let Some(label) = &label {
        out.push(label_view(label));
    }
    let mut rows = view().app_class(Some(SharedString::from("grid gap-1.5")));
    for item in &items {
        let mut names = view().app_class(Some(SharedString::from("grid gap-1.5")));
        if nest && let Some(label) = &label {
            names = names.child(label_view(label));
        }
        names = names.child(
            text(item.name.clone()).app_class(Some(SharedString::from("text-muted-foreground"))),
        );
        rows = rows.child(
            view()
                .app_class(Some(tooltip_row_classes(indicator)))
                .child(match props.hide_indicator.get() {
                    true => view().hidden().into_element(),
                    false => view()
                        .app_class(Some(indicator_classes(indicator, nest, &item.color)))
                        .into_element(),
                })
                .child(
                    view()
                        .app_class(Some(tooltip_text_classes(nest)))
                        .child(names)
                        .child(text(format_value(item.value)).app_class(Some(SharedString::from(
                            "font-mono font-medium text-foreground tabular-nums",
                        )))),
                ),
        );
    }
    out.push(rows.into_element());
    out
}

/// The props of [`ChartLegendContent`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct ChartLegendContentProps {
    /// The series; none: its chart's.
    #[prop(default)]
    pub items: Prop<Vec<ChartConfigItem>>,
    /// Over (`top`) or under (`bottom`, the default) the chart: which side
    /// its padding is on.
    #[prop(default)]
    pub vertical_align: Prop<ChartLegendAlign>,
}

/// What a chart's parts find of it.
#[derive(Clone)]
struct ChartContext {
    config: Prop<Vec<ChartConfigItem>>,
}

/// shadcn's ChartLegendContent: a swatch and the label of each series.
#[component]
pub fn ChartLegendContent(props: ChartLegendContentProps, _cx: Scope<NoEvent>) -> impl IntoElement {
    let chart = use_context::<ChartContext>();
    view()
        .app_class(move || Some(legend_classes(props.vertical_align.get())))
        .child(dynamic(move || {
            let mut items = props.items.get();
            if items.is_empty()
                && let Some(chart) = &chart
            {
                items = chart.config.get();
            }
            items
                .into_iter()
                .map(|item| {
                    let swatch = cn([LEGEND_SWATCH, &color_class("bg", &item.color)]);
                    view()
                        .app_class(Some(SharedString::from(LEGEND_ITEM)))
                        .child(view().app_class(Some(swatch)))
                        .child(text(item.label))
                        .into_element()
                })
                .collect::<Vec<_>>()
        }))
}

/// The props of [`ChartContainer`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct ChartContainerProps {
    /// The series (shadcn's `config`).
    #[prop(default)]
    pub config: Prop<Vec<ChartConfigItem>>,
    /// The rows (recharts' `data`): a column per series' `key`.
    #[prop(default)]
    pub data: Prop<Rows>,
    /// What each row is (the month): the tooltip's label.
    #[prop(default)]
    pub labels: Prop<Vec<SharedString>>,
    /// The x axis' texts, one per row (`tickFormatter`'s); none: no axis.
    pub x_axis_labels: Prop<Option<Vec<SharedString>>>,
    /// How the x axis places rows (a band per row for bars).
    pub x_scale: Prop<Option<Scale>>,
    /// How the y axis places values.
    pub y_scale: Prop<Option<Scale>>,
    /// The grid's lines along the y axis (`CartesianGrid vertical={false}`).
    #[prop(default)]
    pub grid: Prop<Vec<f64>>,
    /// The tooltip, beside the pointer (default true).
    #[prop(default = true)]
    pub tooltip: Prop<bool>,
    /// The tooltip's indicator.
    #[prop(default)]
    pub tooltip_indicator: Prop<ChartTooltipIndicator>,
    /// The tooltip has no label.
    #[prop(default)]
    pub tooltip_hide_label: Prop<bool>,
    /// The tooltip's cursor: a band over the row's bars, a line at its
    /// point (default true; recharts' `cursor`).
    #[prop(default = true)]
    pub cursor: Prop<bool>,
    /// The legend under the chart.
    #[prop(default)]
    pub legend: Prop<bool>,
    /// The chart's margin left and right (recharts' `margin`, default 5).
    #[prop(default = px(5.0))]
    pub margin_x: Prop<Points>,
    /// What screen readers say of the chart.
    pub accessibility_label: Prop<Option<SharedString>>,
    /// The marks: [`chart_bar`], [`chart_area`], [`chart_line`] or any of
    /// `nsh_base::plot`'s.
    pub children: Prop<Children>,
}

/// The events of [`ChartContainer`].
#[derive(Debug, Clone, PartialEq, Event)]
pub enum ChartContainerEvent {
    /// The pointer came over another row, or left (`None`).
    #[args(index)]
    HoverChange(Option<u32>),
}

/// shadcn's ChartContainer: a chart of its `config`'s series, with
/// shadcn's grid, axis, tooltip and legend.
#[component]
pub fn ChartContainer(props: ChartContainerProps, cx: Scope<ChartContainerEvent>) -> impl IntoElement {
    provide_context(ChartContext { config: props.config });
    let hovered = signal(None::<u32>);
    let band = move || matches!(props.x_scale.get(), Some(Scale::Band(_)));
    let tooltip_items = move || {
        let index = hovered.get()? as usize;
        let data = props.data.get();
        let items = props
            .config
            .get()
            .into_iter()
            .filter_map(|item| {
                let value = *data.column(&item.key)?.get(index)?;
                Some(ChartTooltipItem::new(item.label, value, item.color))
            })
            .collect::<Vec<_>>();
        Some((props.labels.get().get(index).cloned(), items))
    };
    let tooltip = chart_tooltip_content()
        .indicator(props.tooltip_indicator)
        .hide_label(props.tooltip_hide_label)
        .label(move || tooltip_items().and_then(|(label, _)| label))
        .items(move || tooltip_items().map(|(_, items)| items).unwrap_or_default());
    let axis = move || props.x_axis_labels.get().is_some();
    let plot = plot()
        .data(props.data)
        .x_scale(props.x_scale)
        .y_scale(props.y_scale)
        .accessibility_label(props.accessibility_label)
        // A line over the marks at a point; a band's cursor is drawn
        // behind the bars (below), as recharts draws it.
        .cross_line_class(move || {
            (props.cursor.get() && !band()).then(|| SharedString::from(CURSOR_LINE))
        })
        .dot_class(Some(SharedString::from("size-2 rounded-full")))
        .tooltip_placement(TooltipPlacement::Pointer)
        .tooltip_offset(px(10.0))
        .on_hover_change(move |index: &Option<u32>| {
            hovered.set(*index);
            cx.emit(ChartContainerEvent::HoverChange(*index));
        })
        .flex_1()
        .mt(px(5.0))
        .mx(move || Length::from(props.margin_x.get()))
        .mb(move || px(if axis() { 35.0 } else { 5.0 }))
        .child(band_cursor(props.x_scale, props.cursor, props.data, hovered))
        .child(dynamic(move || {
            let lines = props.grid.get();
            (!lines.is_empty()).then(|| {
                grid()
                    .y(Some(lines))
                    .app_class(Some(SharedString::from(GRID)))
                    .into_element()
            })
        }))
        .child(props.children)
        .child(dynamic(move || {
            props.x_axis_labels.get().map(|labels| {
                let labels: Arc<[SharedString]> = labels.into();
                plot_axis()
                    .format(Some(TickFormat::from_fn(move |value, _| {
                        labels.get(value as usize).cloned().unwrap_or_default()
                    })))
                    .label_class(Some(SharedString::from(TICK)))
                    .into_element()
            })
        }));
    let plot = plot.tooltip_content(tooltip.style_when(move || !props.tooltip.get(), |s| s.hidden()));
    view()
        .app_class(Some(SharedString::from(CONTAINER)))
        .child(plot)
        .child(dynamic(move || {
            props.legend.get().then(|| chart_legend_content().into_element())
        }))
}

/// The cursor of a band scale, behind the marks: the hovered row's band,
/// placed as the scale places it (d3's `scaleBand`, centered).
fn band_cursor(
    scale: Prop<Option<Scale>>,
    cursor: Prop<bool>,
    data: Prop<Rows>,
    hovered: Signal<Option<u32>>,
) -> View {
    let width = signal(0.0_f32);
    let rect = move || {
        let Some(Scale::Band(band)) = scale.get() else { return None };
        if !cursor.get() {
            return None;
        }
        let index = hovered.get()? as f32;
        let n = data.with(|d| d.len()).max(1) as f32;
        let (inner, outer) = (band.padding_inner.0, band.padding_outer.0);
        let step = width.get() / (n - inner + 2.0 * outer).max(1.0);
        let start = (width.get() - step * (n - inner)) / 2.0;
        Some((start + index * step, step * (1.0 - inner)))
    };
    view()
        .absolute()
        .inset_0()
        .pointer_events(PointerEvents::None)
        .on_layout(move |b: &Bounds<Points>| width.set(b.size.width.0))
        .child(
            view()
                .absolute()
                .top(px(0.0))
                .bottom(px(0.0))
                .left(move || Length::from(px(rect().map_or(0.0, |r| r.0))))
                .w(move || Length::from(px(rect().map_or(0.0, |r| r.1))))
                .style_when(move || rect().is_none(), |s| s.hidden())
                .app_class(Some(SharedString::from(CURSOR_BAND))),
        )
}

/// A band x scale laid out as recharts' `BarChart` lays its categories out
/// (`barCategoryGap="10%"`, `barGap={4}`).
pub fn chart_band_scale() -> Scale {
    let mut band = BandScale::default();
    band.padding_inner = Percentage(0.1);
    band.padding_outer = Percentage(0.05);
    band.group_padding = Percentage(0.04);
    Scale::Band(band)
}

/// A linear y scale from `low` to `high`.
pub fn chart_linear_scale(low: f64, high: f64) -> Scale {
    Scale::Linear {
        domain: Some([low, high]),
        reverse: false,
    }
}

/// recharts' `<Bar dataKey={key} fill="var(--color-…)" radius={4} />`.
pub fn chart_bar(key: impl Into<SharedString>, color: &str) -> nsh_base::plot::BarElement {
    bar()
        .value(Some(RowAccessor::key(key)))
        .corner_radii(BarCornerRadii::all(px(4.0)))
        .app_class(Some(SharedString::from(color_class("fill", color))))
}

/// recharts' `<Area dataKey={key} type="natural" fill="var(--color-…)"
/// fillOpacity={0.4} stroke="var(--color-…)" />`.
pub fn chart_area(key: impl Into<SharedString>, color: &str) -> nsh_base::plot::AreaElement {
    let fill = color_class("fill", color);
    let stroke = color_class("stroke", color);
    area()
        .y1(Some(RowAccessor::key(key)))
        .curve(Curve::Natural)
        .hover_dot_class(Some(SharedString::from(format!(
            "size-2 rounded-full {}",
            color_class("bg", color)
        ))))
        .app_class(Some(SharedString::from(format!("{fill}/40 {stroke}"))))
}

/// recharts' `<Line dataKey={key} type="natural" stroke="var(--color-…)"
/// strokeWidth={2} dot={false} />`.
pub fn chart_line(key: impl Into<SharedString>, color: &str) -> nsh_base::plot::LineElement {
    line()
        .y(Some(RowAccessor::key(key)))
        .curve(Curve::Natural)
        .hover_dot_class(Some(SharedString::from(format!(
            "size-2 rounded-full {}",
            color_class("bg", color)
        ))))
        .app_class(Some(SharedString::from(format!("{} stroke-2", color_class("stroke", color)))))
}

pub(crate) fn register(registry: &mut Registry) {
    registry
        .register::<ChartContainer>()
        .register::<ChartTooltipContent>()
        .register::<ChartLegendContent>();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_read_as_in_english() {
        assert_eq!(format_value(1286.0), "1,286");
        assert_eq!(format_value(12486.0), "12,486");
        assert_eq!(format_value(186.0), "186");
        assert_eq!(format_value(0.5), "0.5");
    }

    #[test]
    fn every_class_is_known_to_nsh() {
        let mut all: Vec<SharedString> = [
            CONTAINER, TICK, GRID, CURSOR_BAND, CURSOR_LINE, TOOLTIP, LEGEND_ITEM, LEGEND_SWATCH,
            "font-medium grid gap-1.5 text-muted-foreground font-mono font-medium text-foreground \
             tabular-nums size-2 rounded-full",
        ]
        .into_iter()
        .map(SharedString::from)
        .collect();
        for color in ["chart-1", "chart-5", "#2563eb"] {
            for indicator in [
                ChartTooltipIndicator::Dot,
                ChartTooltipIndicator::Line,
                ChartTooltipIndicator::Dashed,
            ] {
                all.push(tooltip_row_classes(indicator));
                for nest in [false, true] {
                    all.push(indicator_classes(indicator, nest, color));
                    all.push(tooltip_text_classes(nest));
                }
            }
            for prefix in ["fill", "stroke", "bg"] {
                all.push(color_class(prefix, color).into());
            }
            all.push(format!("{}/40 stroke-2", color_class("fill", color)).into());
        }
        all.push(legend_classes(ChartLegendAlign::Top));
        all.push(legend_classes(ChartLegendAlign::Bottom));
        let unknown: Vec<_> = all.iter().flat_map(|c| crate::cn::unknown_classes(c)).collect();
        assert!(unknown.is_empty(), "classes NSH does not know: {unknown:?}");
    }
}
