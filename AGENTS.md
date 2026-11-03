# Ferrum — master instructions for agents

This file is the single shared source of rules for every agent working on Ferrum. Read it in full at the start of every session. If a task from the owner contradicts this file, do the owner's task and explicitly say at the end which rule was broken. If a rule here looks wrong, do not silently work around it: propose an edit.

**Always communicate with the owner in Russian.** Code and this file are in English.

---

## 1. Project essence

Ferrum is a native IDE for Rust, written in Rust. The goal is to do what VS Code does, but noticeably faster, lighter on memory, and with near-zero CPU usage when idle.

**Priorities (the higher one wins on conflict):**

1. Responsiveness and low resource usage. The UI thread never blocks.
2. Faithful match to the design mockups.
3. Modularity and extensibility: new features are added, not bolted on by rewriting old ones.
4. Code readability. Outside hot paths, readability beats micro-optimizations.

**Platforms:** Windows, macOS, Linux — equal. Nothing is done until it works on all three.

**Languages:** Rust is the primary language with the deepest integration (cargo, rust-analyzer, CodeLLDB, crates). Other languages are supported through language packs (see section 4). The IDE core knows nothing about Rust.

**Working title.** "Ferrum" is not final. The product name and all identifiers derived from it (display name, config directory name, the `.ferrum/` workspace folder, the WIT package namespace, URL scheme) are defined once as constants in the `app_identity` crate. Never hardcode the name anywhere else; user-visible text gets it from there via `i18n`. The plugin API namespace is finalized together with the name before the first public plugin API release.

**Non-goals:** running VS Code extensions; WebView, Electron, HTML UI; remote development in the first version (but the architecture allows it, see section 10).

---

## 2. Stack

| Area | Choice |
|---|---|
| UI | GPUI (the exact `gpui-pre` snapshot version on crates.io that gpui-component is built against, see ADR 0001) + gpui-component |
| Text buffer | `ropey` |
| Highlighting, outline | `tree-sitter` (incremental parsing); grammars from plugins — tree-sitter wasm grammars |
| Language servers | `lsp-types` + own LSP client |
| Debugging | own DAP client (own serde types); Rust adapter — CodeLLDB |
| Git | `gix` |
| Terminal | `alacritty_terminal` (emulation), rendering — our own on GPUI |
| Project search | `ignore`, `grep-searcher`, `grep-regex` |
| File watching | `notify` |
| Plugins | `wasmtime` (Component Model) + WIT; author SDK via `wit-bindgen` |
| Settings | `serde` + `toml` + `schemars` |
| Localization | Fluent (`fluent-bundle`) |
| Config paths | `directories` |
| Errors | `thiserror` in libraries, `anyhow` only in the binary and glue code |
| Logging | `tracing` + `tracing-subscriber` |
| Benchmarks | `criterion` |
| License checks | `cargo-deny` |

GPUI and gpui-component versions must be compatible: use the exact `gpui-pre` version that the chosen gpui-component version requires, both pinned with `=`. Updating GPUI is a separate task with a separate commit, never done in passing.

Async: use GPUI executors (foreground and background). Library code is written against `futures` traits and is runtime-agnostic. Do not add `tokio` without the owner's approval; if some dependency requires it, it lives isolated in one crate behind a channel boundary.

---

## 3. Licenses and third-party code

- Dependencies must have permissive licenses only. Allowed: `MIT`, `Apache-2.0`, `Apache-2.0 WITH LLVM-exception`, `BSD-2-Clause`, `BSD-3-Clause`, `ISC`, `Zlib`, `Unlicense`, `CC0-1.0`, `MIT-0`, `0BSD`, `BSL-1.0`, `Unicode-3.0`. For fonts and icons additionally `OFL-1.1`.
- Forbidden: `GPL`, `AGPL`, `LGPL`, `SSPL` and any copyleft license. `MPL-2.0` and anything not listed — only with the owner's approval.
- The list is pinned in `deny.toml`; `cargo deny check` runs in CI. Licenses outside the list (e.g. `MPL-2.0`) are allowed only as per-crate exceptions approved by the owner, never globally.
- **Do not copy code from other IDEs and editors** (Zed, Lapce, VS Code, Helix and others), even if their license allows it. Studying ideas is fine; we write our own code. Using libraries as dependencies is fine if the license is on the list above.
- Ferrum is open source under the dual license **`MIT OR Apache-2.0`** (the Rust ecosystem standard). The root contains `LICENSE-MIT` and `LICENSE-APACHE`; `[workspace.package]` sets `license = "MIT OR Apache-2.0"` and all crates inherit it. `extension_api` and the SDK use the same license; plugin authors license their plugins however they like. Outside contributions are accepted under the same dual license; no CLA is required.

---

## 4. Architecture

### 4.1 Workspace

```
ferrum/
├── AGENTS.md  CLAUDE.md  ROADMAP.md  Cargo.toml  deny.toml  rust-toolchain.toml
├── docs/
│   ├── design/mockups/      # mockups — source of truth for design
│   ├── decisions/           # architecture decision records (ADR)
│   └── architecture/        # subsystem descriptions, written as they appear
├── crates/
│   ├── ferrum/              # binary: only main() and wiring
│   ├── app_identity/        # product name and identifiers, single source
│   ├── workbench/           # window layout, panels, tabs, status bar
│   ├── ui/                  # design system: components on top of gpui-component
│   ├── theme/               # design tokens, themes, theme loading and import
│   ├── i18n/                # localization, UI strings
│   ├── editor/              # editor rendering and input (GPUI)
│   ├── text/                # buffer, edits, undo, selections, multi-cursor
│   ├── syntax/              # tree-sitter: highlighting, outline, folding
│   ├── language/            # LanguageService trait, language registry
│   ├── lsp/                 # generic LSP client
│   ├── lang_rust/           # built-in Rust language pack
│   ├── project/             # workspace, file tree, recent projects
│   ├── fs/                  # Fs trait, real and fake implementations
│   ├── process/             # process spawning trait
│   ├── search/              # project-wide search and replace
│   ├── git/                 # repository operations via gix
│   ├── terminal/            # terminal
│   ├── debugger/            # DAP client
│   ├── commands/            # command registry, palette, keymaps
│   ├── settings/            # settings registry and layers
│   ├── extension_api/       # WIT interfaces and SDK for plugin authors
│   ├── extension_host/      # running plugins in wasmtime
│   └── extension_registry/  # plugin sources, install, update
└── extensions/              # built-in declarative plugins (themes, icons)
```

Crates are created when the ROADMAP reaches them, not all at once.

### 4.2 Layers and dependency direction

1. **Core without UI:** `app_identity`, `text`, `syntax`, `fs`, `process`, `lsp`, `git`, `search`, `settings`, `commands`, `language`, `i18n`. These crates **do not depend on GPUI** and can be tested without a window.
2. **Services:** `project`, `lang_rust`, `debugger`, `terminal` (logic), `extension_host`, `extension_registry`.
3. **UI:** `theme`, `ui`, `editor`, `workbench`.
4. **Application:** `ferrum`.

Dependencies only point down the layers. If a lower crate needs an upper one, the boundary is wrong: stop and propose a solution.

### 4.3 Key abstractions

- **`Fs` and `ProcessRunner`.** All file system access and process spawning go through these traits only, never directly through `std::fs` or `std::process` outside the `fs` and `process` crates. This is the root for remote development and for tests with a fake file system.
- **Commands.** Every user action is a command with a stable ID of the form `area.action` (`editor.save`, `git.commit`, `workbench.toggleTerminal`). The palette, keybindings, menus and plugins invoke actions only by ID. No action is mouse-only.
- **Contributions.** Panels, status bar items, commands, settings, themes and languages are registered through registries. Built-in features use the same contribution points as plugins, just compiled natively.
- **`LanguageService`.** The UI talks to language support through this trait and checks the server's capabilities: if the server cannot do something, the matching UI element is simply not shown.
- **State and events.** Inside the UI — GPUI entities and subscriptions. Services expose async methods and event streams. Global mutable state is forbidden; registries are populated at startup and are read-only afterwards.

### 4.4 Languages

- A language pack is declarative: file extensions, tree-sitter grammar and highlight queries, LSP server command and its settings, debugger configuration, snippets. Optionally — wasm logic.
- A new language is added as a pack without changing the core.
- Rust is the built-in `lang_rust` pack, native for performance but implementing the same traits. **Everything specific to rust-analyzer** (runnables, macro expansion, `serverStatus`, code lens commands, snippetTextEdit, its own semantic token types) lives only in `lang_rust`. The words "rust" and "rust-analyzer" do not appear in core logic outside `lang_rust`.
- rust-analyzer runs as a separate process over LSP. Do not embed it as a library (`ra_ap_*`): that ties the IDE to one server. The server path is configurable; the server must be replaceable without core changes.

---

## 5. Plugins

**Technology.** WebAssembly Component Model via wasmtime; interfaces are defined in WIT, package `ferrum:extension`. Authors write plugins in Rust with the SDK from `extension_api` (or in other languages that compile to components).

**Manifest** `extension.toml`: id, name, version, compatible API version, contributions, requested permissions. Many plugins are just a manifest and assets with no code: themes, icons, snippets, grammars, keymaps, LSP server configurations.

**Plugin capabilities are broad.** The owner's position: the performance and look of a third-party plugin are the responsibility of whoever installed it. So plugins can:

- register commands, keybindings, settings; subscribe to events (open, save, text change, selection change, etc.);
- read and edit buffers via the API; add decorations (inline text, highlights, gutter marks), diagnostics, code lenses;
- build their own UI: panels, tabs, status bar items, popups — from a tree of primitives (flex containers, text, icons, buttons, inputs, lists, trees, tables) with any styles, plus a **canvas** primitive with draw commands for fully custom graphics;
- spawn processes, access the network and read files — if the user granted the matching permission.

**What a plugin cannot do** (this protects the IDE, it does not limit freedom):

- run on the UI thread: plugin code runs in the background, the UI receives a ready element tree;
- freeze the IDE: long calls are interrupted via epoch interruption, the time limit is configurable; exceeding it produces a notification, not a hang;
- crash the IDE: a panic or error in a plugin disables the plugin and shows a report; the IDE keeps running;
- eat all memory: a per-instance memory limit is set in settings.

**Transparency.** A plugin monitor shows CPU time, memory and call latency per plugin, so the user sees the cost of each plugin.

**Permissions** are declared in the manifest and confirmed by the user on install: project files, any files, network, process spawning, clipboard.

**API versioning:**

- The WIT package follows semver: `ferrum:extension@MAJOR.MINOR.PATCH`.
- `api = "1.2"` in a plugin manifest means ">= 1.2 and < 2.0".
- Within a major version, changes are additive only. A breaking change is a new major version, and the host keeps supporting the previous major version through an adapter.
- Experimental features live in a separate `ferrum:extension-unstable` interface, opted into explicitly, and may break.
- Before 1.0 the API is 0.x and may break, but every breaking change is recorded in the API CHANGELOG.
- Any change to WIT interfaces requires the owner's approval.

**Plugin registry.** A separate git repository with an `index.toml`: id, version, API version, archive URL, SHA-256. The IDE downloads the index, verifies the checksum, installs. The client works through an `ExtensionSource` trait, so the git repository can later be replaced with an HTTP server without touching the rest of the code.

**VS Code compatibility — format import only**, done once at install or import time with conversion to the internal format (no runtime interpretation): JSON color themes, snippets, icon themes. Not supported: running VS Code extensions, TextMate grammars.

---

## 6. Settings

Goal: very detailed settings in the future without rewriting the system.

- **Schema first.** Every crate and plugin declares its settings as a struct with `serde`, `schemars` and defaults; a field's doc comment becomes its description in the UI. The struct is registered in the registry under its namespace (`editor`, `terminal`, `git`, …).
- **Layers** (each overrides the previous one): built-in defaults → user `settings.toml` in the config directory → workspace `.ferrum/settings.toml` → per-language overrides `[lang.rust]`. The architecture must allow new layers (e.g. per folder).
- **Live application.** Settings are observable; changes apply without restart wherever possible.
- **The settings UI is generated from the schema:** a new field automatically appears in the UI, with search.
- **Keybindings** live in a separate `keymap.toml` with contexts (editor, terminal, file tree, …) and per-platform defaults (Ctrl on Windows and Linux, Cmd on macOS).
- Do not hardcode anything tunable: timeouts, sizes, delays, limits go either into settings or into named constants with a comment.
- No code reads config files bypassing the `settings` crate.

---

## 7. Design and UI

- **Source of truth** — the mockups in `docs/design/mockups/`. Implement as close to them as possible. Design changes only with the owner's approval.
- **Tokens.** Colors, spacing, radii, font sizes come only from tokens in the `theme` crate. A hardcoded color or spacing value in a component is a bug. Tokens are structured so that light and high-contrast themes and a compact density mode (spacing via density tokens) can be added later.
- **Base dark theme** (values eyedropped from the mockups, approximate, refine against the mockups):

| Token | Color | Where |
|---|---|---|
| `bg.app` | `#18191D` | title bar, status bar, background between panels |
| `bg.sidebar` | `#15161A` | sidebar |
| `bg.editor` | `#121317` | editor, terminal |
| `bg.input` | `#212226` | search bar, input fields |
| `bg.button.secondary` | `#242730` | secondary buttons |
| `bg.kbd` | `#202028` | keyboard shortcut badges |
| `bg.line.active` | `#1D1E22` | current line |
| `bg.list.selected` | `#2A2722` | selected file, selected result |
| `bg.tab.active` | `#282629` | active tab |
| `accent` | `#D79273` | primary buttons, accents, breakpoint dots |
| `text.primary` | `#F7F8FC` | headings, main text |
| `text.secondary` | `#848589` | subtitles |
| `text.muted` | `#666972` | hints, empty states |
| `text.line_number` | `#4C4D51` | line numbers |
| `text.line_number.active` | `#B9BABE` | current line number |
| `diff.added.bg` | `#20352E` | added lines in diffs |

  Take syntax colors from the editor mockups and define them as `syntax.*` tokens.
- **Components.** The `ui` crate is the design system on top of gpui-component (buttons, badges, tabs, panels, lists, inputs). Panels are composed only from `ui` components.
- **Fonts and icons** are embedded in the binary. UI font — Inter, code font — JetBrains Mono (verify against mockups), icons — Lucide or our own SVGs. Licenses: OFL-1.1 and ISC.
- **Localization.** All user-visible strings go through the `i18n` crate (Fluent `.ftl` files); default locales: `en` and `ru`. User-facing string literals directly in UI code are forbidden.
- **Keyboard.** Every action is keyboard-accessible; focus is always visible.

---

## 8. Performance

Budgets (measured on mid-range hardware, release build):

- A frame takes no longer than 8 ms. Anything that may take more than 1 ms runs in the background.
- Forbidden on the UI thread: file I/O, spawning processes, parsing large inputs, waiting on the network or LSP.
- Cold start to an interactive window — target under 300 ms.
- Idle — about 0% CPU. No constantly ticking timers; cursor blink uses a timer and stops when the window is unfocused.
- Shell memory without language servers on a medium project — target under 150 MB.

Rules:

- All long lists are virtualized (`uniform_list` or equivalent): editor, file tree, search results, commits, terminal output.
- A 100,000+ line file opens and scrolls smoothly.
- Everything is incremental: tree-sitter, LSP sync (changed ranges only), 100–200 ms debounce for diagnostics and hints, cancel stale requests via `$/cancelRequest`, inlay hints and semantic tokens requested only for the visible range.
- Language servers run at lowered process priority. One server per workspace, even with multiple windows.
- No O(document size) work per keystroke.
- Hot paths (`text`, `syntax`, search) are covered by `criterion` benchmarks. Changes to them come with before/after numbers.

---

## 9. How to write code

- Rust stable, edition 2024, version pinned in `rust-toolchain.toml`.
- All dependency versions live in `[workspace.dependencies]` of the root `Cargo.toml`; crates use `dep.workspace = true`.
- Lints live in `[workspace.lints]`: clippy `pedantic` as warnings (with explicitly listed exceptions), `unwrap_used` and `expect_used` as warnings (allowed in tests), `unsafe_code = "deny"`. `unsafe` only with the owner's permission and a `// SAFETY:` comment on every block. `missing_docs` warns everywhere and is an error in `extension_api`.
- CI builds with `-D warnings`. Disabling lints, tests or checks to make CI pass is forbidden.
- A targeted `#[allow(clippy::...)]` for a single `pedantic` lint on a specific item is acceptable with a justification comment next to it. Disabling lints globally, or allowing one to hide a real problem, is forbidden.
- Errors: typed errors with `thiserror` in libraries; `anyhow` only in `ferrum` and glue code. No panics across the plugin boundary.
- Minimal visibility: `pub(crate)` by default; a crate's public API is deliberate and documented.
- One module — one responsibility. A file over ~500 lines is a reason to split it.
- Descriptive names, no abbreviations like `mgr`, `ctx2`, `tmp`.
- Code, comments, identifiers and commit messages are in English. Owner-facing documents (`ROADMAP.md`, `docs/decisions/`) are in Russian.
- Logging via `tracing`; slow operations are wrapped in spans.
- Tests: unit tests next to the code, integration tests in each crate's `tests/`. Use the fake `Fs` for file system, a mock server for LSP, the GPUI test context for UI logic. Every bug fix comes with a regression test.
- Conventional Commits: `feat(editor): ...`, `fix(lsp): ...`.
- CI (GitHub Actions), matrix Windows, macOS, Linux: `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `cargo deny check`.

---

## 10. Roots for the future

Do **not implement** these now, but make no decisions that block them. When designing a module, check against this list:

- remote development (backend on a server, UI local) — hence the `Fs` and `ProcessRunner` traits and the logic/UI split;
- multiple windows and multi-root workspaces;
- very detailed settings, per-folder settings layers, settings sync;
- light and high-contrast themes, compact UI density;
- additional UI languages;
- a plugin marketplace server instead of a git repository;
- session restore (open files, layout, panel state);
- huge-file mode;
- screen reader accessibility;
- replacing rust-analyzer with another server.

---

## 11. Agent workflow

There are no intermediaries between the owner and the agent. Stages and tasks live in `ROADMAP.md`.

**At the start of a session:**

1. Read `ROADMAP.md`: the current stage and the log of recent sessions.
2. If the owner gave a specific task — do it. Otherwise take the first unfinished task of the current stage.
3. Read only the `docs/architecture/` documents relevant to the task. Do not re-read the whole repository.

**Before large changes, stop and ask the owner** (briefly, one message with options): a new crate not listed in section 4.1; a new dependency; a change to WIT interfaces or the public API of a core crate; a deviation from the architecture or design. Record the decision in `docs/decisions/NNNN-title.md` (context, decision, consequences — up to 20 lines, in Russian).

**While working:**

- Small, coherent changes. Do not rewrite working code without a reason.
- Before finishing: `cargo fmt`, `cargo clippy` with no warnings, `cargo test` for the affected crates.
- No `TODO` without an entry in `ROADMAP.md`.
- When you create a new subsystem, briefly describe it in `docs/architecture/<subsystem>.md` (purpose, key types, how to extend).

**At the end of a session:**

1. Mark completed items in `ROADMAP.md`.
2. Add a log entry to `ROADMAP.md` in Russian: date, what was done, what remains, known issues — up to 5 lines.
3. If a project-wide rule changed, update this file and explicitly tell the owner what you changed.

**Forbidden:** changing the design without approval; adding dependencies with licenses outside the list; copying code from other IDEs; disabling lints and tests; doing tasks from future stages "while at it"; bloating this file with details that belong in `docs/`.
