# nsh-shadcn design

shadcn/ui for NSH (Native Shell), in Rust: every component of shadcn's
`new-york-v4` registry with the same props, variants, sizes and look. One
crate, an nshm: Rust apps call the components, TSX apps import them
(`#[native_module("nsh-shadcn")]`). `nsh-base` owns behavior (focus,
keyboard, popups, accessibility); this crate owns the presentation.

## The rule: shadcn's classes, word for word

A component draws with the Tailwind class strings of its shadcn source
(`apps/v4/registry/new-york-v4/ui/<name>.tsx`), applied with `.app_class(..)`.
NSH parses Tailwind 4 natively and its default theme is shadcn's neutral
tokens, so the same strings give the same look, and a shadcn update is a diff
of strings. Typed style methods are used only where a class cannot say it.

Where NSH has no equivalent, the class string changes, and the component's
module doc lists each change. The known ones:

| shadcn | NSH | Why |
|---|---|---|
| `border` (color from the global `* { @apply border-border }`) | `border border-border` | NSH has no global rules |
| `[&_svg]:…`, `has-[>svg]:…`, other descendant selectors | a prop of the component (`icon`, …) | no descendant selectors |
| `data-[state=open]:…` | `aria-expanded:…` / `data-<name>:` | NSH has the boolean data form only |
| `aria-invalid:…` | an `invalid` prop | NSH has no such state |
| `peer-*`, `before:`, `after:` | an element of the component | not supported |
| `animate-in fade-in-0 zoom-in-95` (tw-animate-css) | NSH's popup transitions | no keyframe utilities |

What the port taught, true of every component:

- **Inherited text size.** NSH's default text is 14 points; a browser body is 16px with a 1.5 line height, which
  shadcn text without a size class inherits. A component whose text relies on that inheritance sets `text-base`.
- **Flex items don't shrink.** NSH (like Yoga) defaults `flex-shrink` to 0; text in a row that must wrap or truncate
  gets `shrink` / `min-w-0`. A `flex-1` item adds no width to a `w-fit` parent: use `flex-auto` there.
- **No `calc()`, viewport units or arbitrary property lists.** `calc(...)` lengths become the fixed lengths they
  compute to, `vh` becomes `%`, `transition-[color,box-shadow]` becomes `transition` (colors, opacity, shadows —
  the focus ring is a shadow — and transforms).
- **No child or sibling selectors.** `has-*`, `group-has-*`, `*:`, `[&_…]`, `peer-*` become props, or a context the
  parent provides and its parts read (a part registering itself with a counter when the parent must know it is there).
- **Radix states** become NSH's: `data-[state=open]` → `aria-expanded:` (or the component's open state),
  `data-[state=checked|on]` → `checked:` / `aria-pressed:`, `data-[highlighted]` → `focus:`, `data-[disabled]` →
  `disabled:`, `data-[state=active]` → `selected:`. Boolean `data-<name>:` and `group-data-<name>/<group>:` work.
- **Variant order differs:** NSH sorts breakpoints (`sm:`, `md:`) after `group-*` / `data-*` variants.
- **nsh-base trigger slots wrap their content.** The wrapper gets the press, focus and `aria-expanded`: a Button in
  a trigger slot takes the press itself (nothing opens), and state variants on the slot content never fire. Triggers
  are plain views drawn with Button's classes; open-state looks come from the component's open state.
- **Enter/exit animations** (tw-animate-css `animate-in …`) are NSH's popup, modal and sheet transitions: popups use
  `opacity-0 scale-95` plus an 8-point slide from the side, over 150 ms.
- **Reference pages resolve class conflicts as tailwind-merge does.** A browser applies conflicting classes by CSS
  order, not by position, so `tools/reference/<id>.html` writes the merged class list.

Every component has a unit test that parses all its class strings with NSH's
own parser (`nsh_core::tailwind::parse`, on the default theme) and fails on
any class NSH does not know: an unknown class would otherwise be dropped at
run time with a log line.

## API

- `src/ui/<family>/<name>.rs`: one `#[component]` per shadcn part, its props named
  as shadcn's (`variant`, `size`, `disabled`, …), its enums `PropEnum`s whose
  variants are shadcn's values (`ButtonSize::IconSm` is `"icon-sm"`). Every
  public enum is `#[non_exhaustive]`.
- `src/lib.rs`: a constructor per component taking its required content
  (`button("Save")`), and the component's types.
- Shorthands for enum props are inherent methods: `.outline()`, `.sm()`.
- The app restyles a component with `.app_class("…")`, as `className` in
  shadcn: its classes come after the component's, and in one class string the
  later class wins (no `tailwind-merge` needed).

## Verifying a component: side by side with shadcn

1. `examples/gallery` shows the component as shadcn's docs do, with a
   light/dark switch.
2. `tools/reference.py <component>` writes `reference/<component>.html`:
   the same layout with shadcn's exact classes and theme on Tailwind 4 in a
   browser. A browser at 880×640, device scale 2, saves
   `reference/<component>-{light,dark}.png`.
3. `cd examples/gallery && nsh test --target macos` saves
   `screenshots/<component>-{light,dark}.png` from a real macOS window.
4. `reference/compare.html?c=<component>` shows both and their difference
   (black is identical).

Known residue that is not the component's: text widths differ by fractions of
a point between Chrome and CoreText, and colors outside sRGB (the light
`destructive`) are drawn in Display P3 by Chrome and clipped to sRGB by NSH.
