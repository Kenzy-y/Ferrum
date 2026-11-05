//! Color tokens. Keys match `themes/*.toml`.

use crate::Color;
use crate::macros::token_struct;

token_struct! {
    /// Interface colors.
    ColorTokens: Color {
        /// Title bar, activity bar, status bar, gaps between panels.
        bg_app = "bg.app",
        /// Sidebar panel.
        bg_sidebar = "bg.sidebar",
        /// Editor, terminal, main panels.
        bg_editor = "bg.editor",
        /// Strip behind editor tabs.
        bg_editor_tabs = "bg.editor.tabs",
        /// Cards, diff panes, hint bars.
        bg_card = "bg.card",
        /// Floating widgets (find, debug toolbar).
        bg_overlay = "bg.overlay",
        bg_minimap = "bg.minimap",
        /// Icon tile of a list entry.
        bg_tile = "bg.tile",
        /// Icon tile of the selected or featured entry.
        bg_tile_accent = "bg.tile.accent",
        /// Title bar search, find field.
        bg_input = "bg.input",
        /// Fields inside sidebars.
        bg_input_inset = "bg.input.inset",
        bg_input_disabled = "bg.input.disabled",
        bg_button_primary = "bg.button.primary",
        bg_button_primary_hover = "bg.button.primary.hover",
        bg_button_primary_pressed = "bg.button.primary.pressed",
        bg_button_secondary = "bg.button.secondary",
        bg_button_secondary_hover = "bg.button.secondary.hover",
        bg_button_secondary_pressed = "bg.button.secondary.pressed",
        /// Icon buttons, menu items.
        bg_ghost_hover = "bg.ghost.hover",
        bg_ghost_pressed = "bg.ghost.pressed",
        /// Close button of the window controls.
        bg_window_close_hover = "bg.window_close.hover",
        bg_list_hover = "bg.list.hover",
        bg_list_pressed = "bg.list.pressed",
        bg_list_selected = "bg.list.selected",
        bg_tab_active = "bg.tab.active",
        bg_tab_hover = "bg.tab.hover",
        bg_activity_active = "bg.activity.active",
        bg_activity_hover = "bg.activity.hover",
        bg_kbd = "bg.kbd",
        bg_badge = "bg.badge",
        bg_line_active = "bg.line.active",
        /// Line where the debugger is paused.
        bg_line_debug = "bg.line.debug",
        /// "Paused on breakpoint" banner.
        bg_notice = "bg.notice",
        bg_diff_added = "bg.diff.added",
        bg_diff_removed = "bg.diff.removed",
        bg_scrollbar_thumb = "bg.scrollbar.thumb",
        /// Hovered or dragged scrollbar thumb.
        bg_scrollbar_thumb_hover = "bg.scrollbar.thumb.hover",
        /// The visible part of the text on the minimap.
        bg_minimap_viewport = "bg.minimap.viewport",
        /// Selected text in a focused editor.
        bg_selection = "bg.selection",
        /// Selected text in an editor without focus.
        bg_selection_inactive = "bg.selection.inactive",
        /// Text cursor in the editor.
        cursor = "cursor",
        /// Underline of an error.
        diagnostic_error = "diagnostic.error",
        /// Underline of a warning.
        diagnostic_warning = "diagnostic.warning",
        /// Underline of an information.
        diagnostic_info = "diagnostic.info",
        /// Dots under a hint.
        diagnostic_hint = "diagnostic.hint",
        border_panel = "border.panel",
        border_input = "border.input",
        border_overlay = "border.overlay",
        border_divider = "border.divider",
        /// Short vertical separators in the title bar.
        border_separator = "border.separator",
        /// Keyboard focus ring.
        border_focus = "border.focus",
        /// Focus ring on accent backgrounds (primary buttons).
        border_focus_on_accent = "border.focus.on_accent",
        guide_indent = "guide.indent",
        /// Primary buttons, active markers, logo.
        accent = "accent",
        /// Headings, active tab, main text.
        text_primary = "text.primary",
        /// Menus, tree items, status bar.
        text_secondary = "text.secondary",
        /// Section headers, inactive tabs, placeholders.
        text_subtle = "text.subtle",
        /// Hints, empty states.
        text_muted = "text.muted",
        /// Ignored files.
        text_ignored = "text.ignored",
        text_disabled = "text.disabled",
        /// Text on primary buttons.
        text_on_accent = "text.on_accent",
        /// Links, active segmented tab.
        text_accent = "text.accent",
        /// Selected list item.
        text_selected = "text.selected",
        /// Keyboard shortcut labels.
        text_kbd = "text.kbd",
        text_line_number = "text.line_number",
        text_line_number_active = "text.line_number.active",
        /// Folders of the breadcrumbs above the editor.
        text_breadcrumb = "text.breadcrumb",
        /// The file in the breadcrumbs.
        text_breadcrumb_active = "text.breadcrumb.active",
        text_success = "text.success",
        /// "M" modified marker.
        text_warning = "text.warning",
        text_error = "text.error",
        icon = "icon",
        /// Selected activity bar item.
        icon_active = "icon.active",
        /// Window controls.
        icon_strong = "icon.strong",
        icon_file_rust = "icon.file.rust",
        /// Function glyph in the Outline and the breadcrumbs (other symbol
        /// kinds use syntax colors).
        icon_symbol_function = "icon.symbol.function",
        /// Unsaved-changes dot on a tab.
        marker_dirty = "marker.dirty",
        marker_breakpoint = "marker.breakpoint",
        /// Paused line number and arrow.
        marker_debug_line = "marker.debug_line",
    }
}

token_struct! {
    /// Syntax highlighting colors.
    SyntaxTokens: Color {
        plain = "plain",
        punctuation = "punctuation",
        keyword = "keyword",
        type_name = "type",
        function = "function",
        macro_name = "macro",
        attribute = "attribute",
        string = "string",
        /// Not in the mockups; chosen to differ from keywords.
        number = "number",
        constant = "constant",
        lifetime = "lifetime",
        comment = "comment",
        doc_comment = "doc_comment",
        inlay_hint = "inlay_hint",
    }
}
