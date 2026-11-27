# nsh-shadcn

[shadcn/ui](https://ui.shadcn.com) for [NSH (Native Shell)](https://nsh.dev), in Rust: the same components, variants,
sizes and look, drawn with native views on macOS, iOS, Android, Windows, Linux and the web.

It is an [nshm](https://nsh.dev/docs/nshm.md): one Rust crate that a Rust app calls and a TSX app imports.

**Live demo and docs: <https://caoxing9.github.io/nsh-shadcn/>**: every component running in the browser on NSH's web
runtime, with how to use it.

> Status: every component of shadcn/ui's new-york-v4 registry except `form` (it is react-hook-form), each checked
> against shadcn's own rendering in light and dark. See [DESIGN.md](https://github.com/caoxing9/nsh-shadcn/blob/main/DESIGN.md) for how a component is ported and
> verified, and each component's module doc for where NSH differs from the browser.

## In a Rust app

```rust
use nsh_core::prelude::*;
use nsh_shadcn as ui;

#[component]
fn Toolbar() -> impl IntoElement {
    view()
        .flex_row()
        .gap_2()
        .child(ui::button("Save").on_press(|| {}))
        .child(ui::button("Cancel").outline())
        .child(ui::button("Delete").destructive().sm())
}
```

## Install

```sh
cargo add nsh-shadcn   # a Rust app
nsh add nsh-shadcn     # any NSH app, TSX included
```

## In a TSX app

```tsx
import { Button } from "nsh-shadcn";

<Button label="Save" variant="outline" size="sm" onPress={save} />;
```

## Components

| Family | Components |
|---|---|
| Actions | Button, Button Group, Toggle, Toggle Group |
| Basics | Alert, Aspect Ratio, Avatar, Badge, Breadcrumb, Card, Empty, Item, Kbd, Label, Pagination, Progress, Separator, Skeleton, Spinner, Table |
| Forms | Checkbox, Field, Input, Input Group, Input OTP, Native Select, Radio Group, Slider, Switch, Textarea |
| Overlays | Alert Dialog, Dialog, Drawer, Hover Card, Popover, Sheet, Sonner (toasts), Tooltip |
| Menus | Combobox, Command, Context Menu, Dropdown Menu, Menubar, Navigation Menu¹, Select |
| Layout | Accordion, Calendar, Carousel, Chart, Collapsible, Resizable, Scroll Area, Sidebar¹, Tabs |
| Chat | Attachment, Bubble, Marker, Message, Message Scroller, Direction |

¹ Partial. Navigation Menu opens on press (no hover-open, shared viewport or indicator). Sidebar has no mobile sheet,
collapsed-state tooltips, sub-menus, menu badges/actions or rail.

Not ported: `form` (a react-hook-form wrapper; Field covers the layout).

## The docs site

`tools/site.sh` builds it in `site/`: the gallery's web build (`nsh package --target web`) and a page per component
that runs it live. It needs an LLVM with wasm32 support on `PATH` (macOS: `brew install llvm`).

## License

MIT. The components' class strings come from shadcn/ui (MIT, © shadcn); see [LICENSE](https://github.com/caoxing9/nsh-shadcn/blob/main/LICENSE).
