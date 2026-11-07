//! Metric tokens. Keys match `density/*.toml`.

use crate::macros::token_struct;

token_struct! {
    /// Fixed sizes of interface elements.
    SizeTokens: f32 {
        titlebar_height = "titlebar.height",
        /// Left padding before the logo.
        titlebar_inset = "titlebar.inset",
        titlebar_logo_size = "titlebar.logo.size",
        titlebar_logo_radius = "titlebar.logo.radius",
        /// Gap between the wordmark and the menus.
        titlebar_menus_offset = "titlebar.menus.offset",
        titlebar_search_width = "titlebar.search.width",
        window_control_width = "window_control.width",
        /// Gap between the title bar and the panels.
        layout_gap_top = "layout.gap.top",
        statusbar_height = "statusbar.height",
        activity_bar_width = "activity_bar.width",
        /// Space above the first activity button.
        activity_bar_top = "activity_bar.top",
        activity_bar_gap = "activity_bar.gap",
        bottom_panel_height = "bottom_panel.height",
        /// Header of a page in the editor area (Welcome).
        editor_header_height = "editor_header.height",
        /// Content width of the Welcome page.
        welcome_width = "welcome.width",
        /// Left column of the Welcome page.
        welcome_column = "welcome.column",
        /// Space above the Welcome content.
        welcome_top = "welcome.top",
        welcome_logo = "welcome.logo",
        welcome_button = "welcome.button",
        /// Rows of the Get started list.
        welcome_row = "welcome.row",
        /// Recent projects card when empty.
        welcome_recent_height = "welcome.recent.height",
        /// Fixed-width shortcut badges.
        kbd_width = "kbd.width",
        /// Space above an empty state in a sidebar.
        empty_state_top = "empty_state.top",
        empty_state_icon = "empty_state.icon",
        empty_state_button = "empty_state.button",
        dialog_width = "dialog.width",
        activity_button_width = "activity_button.width",
        activity_button_height = "activity_button.height",
        sidebar_width = "sidebar.width",
        /// Horizontal inset of section header text inside a panel.
        section_header_inset = "section_header.inset",
        /// Indentation of one tree level.
        tree_indent = "tree.indent",
        /// Gap between panels and the window edge.
        panel_gap = "panel.gap",
        tab_strip_height = "tab_strip.height",
        tab_height = "tab.height",
        tab_width = "tab.width",
        /// Accent bar on the active tab / selected row.
        tab_marker_width = "tab.marker.width",
        tab_marker_height = "tab.marker.height",
        panel_tab_underline = "panel_tab.underline",
        list_row_height = "list_row.height",
        /// Rows of the Outline section (code font).
        outline_row_height = "outline.row_height",
        /// Tallest the Outline list grows before it scrolls.
        outline_max_height = "outline.max_height",
        button_small_height = "button.small.height",
        button_medium_height = "button.medium.height",
        input_height = "input.height",
        input_inset_height = "input.inset.height",
        kbd_height = "kbd.height",
        badge_size = "badge.size",
        dirty_dot_size = "dirty_dot.size",
        icon_tiny = "icon.tiny",
        icon_small = "icon.small",
        icon_medium = "icon.medium",
        code_line_height = "code.line_height",
        /// Row with the file path above the editor text.
        breadcrumbs_height = "breadcrumbs.height",
        /// Left inset of the line numbers and the breadcrumbs.
        editor_inset = "editor.inset",
        /// Space above the first line of text.
        editor_top = "editor.top",
        /// Gap between the line numbers and the text (markers go here).
        editor_gutter_gap = "editor.gutter_gap",
        /// Gap between the line numbers and the current line highlight.
        editor_active_line_inset = "editor.active_line.inset",
        /// Height of the current line highlight (slightly taller than a line).
        editor_active_line_height = "editor.active_line.height",
        /// Space right of the text before the panel edge.
        editor_right = "editor.right",
        cursor_width = "cursor.width",
        cursor_height = "cursor.height",
        scrollbar_width = "scrollbar.width",
        /// Gap between the scrollbar and the panel edge.
        scrollbar_inset = "scrollbar.inset",
        minimap_width = "minimap.width",
        /// Space between the minimap and the panel edge.
        minimap_right = "minimap.right",
        minimap_bottom = "minimap.bottom",
        /// Space between the bars and the minimap edge.
        minimap_inset = "minimap.inset",
        minimap_radius = "minimap.radius",
        /// Height of a text line on the minimap.
        minimap_line_height = "minimap.line_height",
        minimap_bar_height = "minimap.bar_height",
        /// Width of a character on the minimap.
        minimap_char_width = "minimap.char_width",
        /// Thickness of a diagnostic underline.
        diagnostic_underline = "diagnostic.underline",
        /// Side and gap of the dots under a hint.
        diagnostic_dot = "diagnostic.dot",
        /// Widest a hover popup grows before it wraps.
        hover_max_width = "hover.max_width",
        /// Tallest a hover popup grows before it scrolls.
        hover_max_height = "hover.max_height",
        /// Width of the completion list.
        completion_width = "completion.width",
        /// Rows of the completion list shown at once.
        completion_rows = "completion.rows",
        /// Gap between the text and an editor popup.
        editor_popup_gap = "editor.popup_gap",
        /// Smallest width of a title bar menu.
        menu_width = "menu.width",
        /// Gap between the end of a line's code and its code lenses.
        code_lens_gap = "code_lens.gap",
        /// Rows of the terminal.
        terminal_line_height = "terminal.line_height",
        /// Bottom panel tabs to the first terminal row.
        terminal_top = "terminal.top",
        /// Width of the terminal tabs list when a split shows several
        /// terminals.
        terminal_tabs_width = "terminal_tabs.width",
        /// The shortcut hints in the corner of the terminal.
        terminal_hint_height = "terminal_hint.height",
        /// Gap between the buttons of the bottom panel header.
        panel_actions_gap = "panel_actions.gap",
        /// Narrowest the sidebar can be dragged to.
        sidebar_min_width = "sidebar.min_width",
        /// Lowest the bottom panel can be dragged to.
        bottom_panel_min_height = "bottom_panel.min_height",
        /// Width the editor area keeps when the sidebar grows.
        editor_min_width = "editor.min_width",
        /// Height the editor area keeps when the bottom panel grows.
        editor_min_height = "editor.min_height",
    }
}

token_struct! {
    /// Spacing scale for paddings and gaps.
    SpaceTokens: f32 {
        xxs = "xxs",
        xs = "xs",
        sm = "sm",
        md = "md",
        lg = "lg",
        xl = "xl",
        xxl = "xxl",
    }
}

token_struct! {
    /// Corner radii.
    RadiusTokens: f32 {
        /// Count badges.
        xs = "xs",
        /// Keyboard badges, current line.
        sm = "sm",
        /// Buttons, tabs, list rows, inputs.
        md = "md",
        /// Cards.
        lg = "lg",
        /// Panels, activity buttons.
        xl = "xl",
        /// Large icon tiles.
        xxl = "xxl",
    }
}

token_struct! {
    /// Font sizes.
    FontSizeTokens: f32 {
        xs = "xs",
        sm = "sm",
        md = "md",
        base = "base",
        lg = "lg",
        xl = "xl",
        brand = "brand",
        h3 = "h3",
        h2 = "h2",
        h1 = "h1",
        display = "display",
        code = "code",
        code_small = "code.small",
    }
}
