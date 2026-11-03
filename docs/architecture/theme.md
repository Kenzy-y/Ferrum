# Theme

Crate `theme` (UI layer).

**Purpose.** Every color, size, spacing, radius, font size and font family
used by the interface. A hardcoded value in a component is a bug.

**Key types.**
- `Theme` — color theme (`themes/<id>.toml`) + `Density` metrics
  (`density/<density>.toml`). `Theme::built_in(id, density)`;
  `set_theme(theme, cx)` stores it in the app; components read it with
  `cx.theme()` (`ActiveTheme`).
- `ColorTokens`, `SyntaxTokens` — colors keyed by dotted names
  (`"bg.app"` → `colors.bg_app`). State colors (`.hover`, `.pressed`,
  `.disabled`, `border.focus`) are explicit tokens so themes can override
  them.
- `Metrics` — `SizeTokens`, `SpaceTokens`, `RadiusTokens`, `FontSizeTokens`
  in logical pixels; `Density::Comfortable` (mockups) and `Compact`.
- `Color` — `#RRGGBB[AA]`, converts to `gpui::Hsla` / `gpui::Fill`.
- `register_fonts(cx)` — embeds Inter and JetBrains Mono (`fonts/`, OFL).

**Rules.** Every key is required and unknown keys are rejected, so a theme is
always complete. Values come from `docs/design/mockups/*.svg`.

**Extending.** New token: add the key to every theme / density file and a
line to `colors.rs` or `metrics_tokens.rs` (the `token_struct!` macro
generates the field and its key). New theme: add a TOML file and list it in
`BUILT_IN_THEMES`. Theme import and user themes are planned for stage 9.
