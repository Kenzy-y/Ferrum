//! Metric tokens. Keys match `density/*.toml`.

use crate::macros::token_struct;

token_struct! {
    /// Fixed sizes of interface elements.
    SizeTokens: f32 {
        titlebar_height = "titlebar.height",
        statusbar_height = "statusbar.height",
        activity_bar_width = "activity_bar.width",
        activity_button_width = "activity_button.width",
        activity_button_height = "activity_button.height",
        sidebar_width = "sidebar.width",
        /// Gap between panels and the window edge.
        panel_gap = "panel.gap",
        tab_strip_height = "tab_strip.height",
        tab_height = "tab.height",
        /// Accent bar on the active tab / selected row.
        tab_marker_width = "tab.marker.width",
        tab_marker_height = "tab.marker.height",
        panel_tab_underline = "panel_tab.underline",
        list_row_height = "list_row.height",
        button_small_height = "button.small.height",
        button_medium_height = "button.medium.height",
        input_height = "input.height",
        input_inset_height = "input.inset.height",
        kbd_height = "kbd.height",
        badge_size = "badge.size",
        dirty_dot_size = "dirty_dot.size",
        icon_small = "icon.small",
        icon_medium = "icon.medium",
        code_line_height = "code.line_height",
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
