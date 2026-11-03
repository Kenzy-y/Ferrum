# UI components

Crate `ui` (UI layer): the design system. Panels are composed only from
these components.

**Components.** `Button` (primary / secondary / ghost, small / medium),
`IconButton`, `Kbd`, `CountBadge`, `StatusLetter`, `EditorTab`, `PanelTab`,
`SegmentedTab`, `Panel`, `SectionHeader`, `Card`, `ListRow`, `ActivityButton`,
`SearchField`, `TextField`, `TextArea`, `Icon`, `Label`.

**How they work.**
- Stateless `RenderOnce` elements; all values come from `cx.theme()`.
  Labels arrive already localized: the crate has no user-facing strings.
- Interactive components share `Interactivity`: background per state,
  hover / pressed styles, a focus ring shown on keyboard focus (Tab),
  activation by click, Enter or Space (`on_activate`), disabled opacity.
  `force_state(InteractionState)` renders a state statically (gallery,
  tests).
- Text editing (`TextField`, `TextArea`) is gpui-component's `Input` /
  `Textarea` with `appearance(false)` inside our frame;
  `sync_component_theme(cx)` maps our tokens onto gpui-component's theme.
- Icons are Lucide SVGs in `icons/lucide` (ISC), served by `ui::Assets`
  under `icons/<name>.svg`.

**Extending.** A new component reads tokens only; if a value is missing, add
a token to `theme` first. Interactive components embed `Interactivity` and
call `interactive_builders!`. A new icon: copy the SVG into `icons/lucide`
and add a line to `icon_names!`.

**Gallery.** `crates/ferrum/src/gallery` shows every component in every
state; it is temporary (stage 1).
