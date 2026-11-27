# Working on nsh-shadcn

shadcn/ui (new-york-v4) ported 1:1 to NSH 0.7.1, in Rust. Read [DESIGN.md](DESIGN.md) first: it is the contract.

## Setup

- The NSH CLI 0.7.1 (`nsh --version`), from <https://nsh.dev> or the `v0.7.1` release of
  <https://github.com/longbridge/nsh>; Rust stable.
- shadcn/ui's source, the truth for classes, props and structure:

  ```sh
  git clone --depth 1 --filter=blob:none --sparse https://github.com/shadcn-ui/ui.git shadcn-ui
  git -C shadcn-ui sparse-checkout set apps/v4/registry/new-york-v4
  ```

- NSH's docs at 0.7.1: `git clone https://github.com/longbridge/nsh.git`, then
  `git -C nsh show "v0.7.1:website/docs/<page>.md"` (also `website/elements/`, `website/modules/`, `docs/dev/`).
  Quote the `rev:path`: in zsh `$T:w…` is a history modifier.

## Where things are

| What | Where |
|---|---|
| The finished example to copy the pattern from | `src/ui/button.rs`, `examples/gallery/src/pages/button.rs`, `examples/gallery/tests/button.test.tsx`, `tools/reference/button.html` |
| A family of components | `src/ui/<family>/` (`mod.rs` declares the files, re-exports the public API, registers them) |
| Gallery pages of a family | `examples/gallery/src/pages/<family>.rs` (helpers `section`, `row` in `pages/mod.rs`) |
| Icons | `src/icon.rs` (`IconName`, `icon(..)`); more Lucide SVGs: `https://unpkg.com/lucide-static@1.81.0/icons/<name>.svg` |
| shadcn's components and the examples its docs show | `shadcn-ui/apps/v4/registry/new-york-v4/ui/<name>.tsx`, `…/new-york-v4/examples/` |
| NSH behavior components (`nsh_base::{select, popover, dialog, …}`) and the core API | the `nsh-base` and `nsh-core` 0.7.1 crates' sources (in Cargo's registry once built) |
| Prior art (behavior, API ideas; not the look) | [nsh-omarchy](https://github.com/huacnlee/nsh-omarchy) (a Rust nshm on nsh-base), [shadcn-nsh](https://github.com/aningdev/shadcn-nsh) (shadcn in TSX on NSH) |

## How to port a component

1. Read its shadcn `.tsx`. Copy every class string verbatim into the Rust file; adapt only what NSH cannot parse, as
   DESIGN.md says, and list each adaptation in the module doc.
2. Behavior comes from `nsh_base` (focus, keyboard, popups, accessibility). Do not write your own popup, focus trap or
   keyboard handling when nsh-base has the component. Structure follows shadcn's parts (`Card`, `CardHeader`, …).
3. One `#[component]` per shadcn part that has its own classes; props named as shadcn's; enums are `PropEnum`,
   `#[non_exhaustive]`, variants named as shadcn's values. A constructor per part in the family's `mod.rs`
   taking the required content (`badge("New")`, `card()`); shorthands for enum props as inherent methods.
4. A unit test per component asserting `crate::cn::unknown_classes(..)` is empty for every class string it can draw
   with (every variant × size × state), as `button.rs` does.
5. A gallery page per shadcn component (`id` = shadcn's file name, `title` = its name): the examples shadcn's docs
   show, within the 880 × 640 content area (about 816 × 540 under the header). A component whose content opens
   (dialog, menu, select) has a trigger; its test presses it before the screenshot.
6. `tools/reference/<id>.html`: the same page with shadcn's own markup and classes (states written out as the DOM has
   them: `data-state="checked"`, `aria-expanded="true"`, open popups positioned as Radix places them), same texts,
   same layout, class conflicts resolved as tailwind-merge would. First line `<!-- title: … -->`.
   `python3 tools/reference.py <id>` must run.
7. `examples/gallery/tests/<family>.test.tsx`: behavior through the UI (press, type, check what shows), and a light
   and a dark screenshot of every page to `../../screenshots/<id>-{light,dark}.png` (paths relative to
   `examples/gallery`), as `tests/button.test.tsx` does.

## Verify

```sh
cargo test --lib                       # at the crate root: every class known, every unit test
cargo clippy --all-targets             # no warnings
cd examples/gallery && nsh test --target macos tests/<family>.test.tsx   # real macOS windows open and close
```

Render the reference page in a browser at 880 × 640, device scale 2 (`reference/<id>-{light,dark}.png`, `#dark`
for dark), open `reference/compare.html?c=<id>`, and fix what does not look like shadcn's component, in light and dark.

## Rules

- Code, comments, docs, commit messages and pull requests in English.
- Do not depend on a component of another family from inside a component; where shadcn composes one (Field uses
  Label), draw it with that component's classes and note it in the module doc.
- Every change goes through a pull request. A change to a component's public API lists it in the PR.
