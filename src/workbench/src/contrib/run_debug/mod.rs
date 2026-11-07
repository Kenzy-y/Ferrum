//! The Run and Debug view of the sidebar, as in `debug.svg`: the launch
//! configuration and the Variables, Watch, Call stack and Breakpoints
//! sections. Until the debugger arrives (stage 8) every section shows its
//! empty state and the controls are disabled.

use gpui::{
    AnyElement, App, Context, FocusHandle, InteractiveElement as _, IntoElement, KeyDownEvent,
    ParentElement as _, SharedString, StatefulInteractiveElement as _, Styled as _, div, px,
};
use theme::ActiveTheme as _;
use ui::{Icon, IconButton, IconName, ListRow, SectionHeader};

use crate::Workbench;

/// A collapsible section of the view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DebugSection {
    Variables,
    Watch,
    CallStack,
    Breakpoints,
}

impl DebugSection {
    /// Every section, top to bottom.
    pub(crate) const ALL: [Self; 4] = [
        Self::Variables,
        Self::Watch,
        Self::CallStack,
        Self::Breakpoints,
    ];

    fn index(self) -> usize {
        match self {
            Self::Variables => 0,
            Self::Watch => 1,
            Self::CallStack => 2,
            Self::Breakpoints => 3,
        }
    }

    fn title(self) -> &'static str {
        match self {
            Self::Variables => "debug-variables",
            Self::Watch => "debug-watch",
            Self::CallStack => "debug-call-stack",
            Self::Breakpoints => "debug-breakpoints",
        }
    }

    /// Shown in the section while there is no debug session.
    fn empty(self) -> &'static str {
        match self {
            Self::Variables => "debug-variables-empty",
            Self::Watch => "debug-watch-empty",
            Self::CallStack => "debug-call-stack-empty",
            Self::Breakpoints => "debug-breakpoints-empty",
        }
    }
}

/// Which sections are collapsed, and their headers' focus.
#[derive(Default)]
pub(crate) struct DebugSections {
    collapsed: [bool; 4],
    headers: Option<[FocusHandle; 4]>,
}

impl DebugSections {
    pub(crate) fn is_expanded(&self, section: DebugSection) -> bool {
        !self.collapsed[section.index()]
    }

    pub(crate) fn toggle(&mut self, section: DebugSection) {
        let collapsed = &mut self.collapsed[section.index()];
        *collapsed = !*collapsed;
    }
}

impl Workbench {
    /// Creates the focus handles of the section headers.
    pub(crate) fn ensure_debug_sections(&mut self, cx: &mut Context<Self>) {
        if self.debug_sections.headers.is_none() {
            self.debug_sections.headers = Some(std::array::from_fn(|_| cx.focus_handle()));
        }
    }

    fn toggle_debug_section(&mut self, section: DebugSection, cx: &mut Context<Self>) {
        self.debug_sections.toggle(section);
        cx.notify();
    }
}

/// A muted line of text inside a section.
fn muted(text: SharedString, cx: &App) -> AnyElement {
    let theme = cx.theme();
    div()
        .px(px(theme.metrics.size.section_header_inset))
        .text_size(px(theme.metrics.font_size.md))
        .text_color(theme.colors.text_muted)
        .child(text)
        .into_any_element()
}

/// The launch configuration picker; disabled until stage 8.
fn configuration(workbench: &Workbench, cx: &App) -> AnyElement {
    let theme = cx.theme();
    let colors = &theme.colors;
    let metrics = &theme.metrics;
    div()
        .id("debug-configuration")
        .flex()
        .flex_none()
        .items_center()
        .mx(px(metrics.space.lg))
        .h(px(metrics.size.input_inset_height + 2.0))
        .px(px(metrics.space.md))
        .gap(px(metrics.space.sm))
        .rounded(px(metrics.radius.md))
        .bg(colors.bg_input)
        .opacity(theme.opacity.disabled)
        .text_size(px(metrics.font_size.base))
        .text_color(colors.text_secondary)
        .tooltip(ui::tooltip(workbench.text("welcome-later")))
        .child(Icon::new(IconName::Play).color(colors.icon))
        .child(
            div()
                .flex_1()
                .truncate()
                .child(workbench.text("debug-no-configuration")),
        )
        .child(Icon::new(IconName::ChevronDown).color(colors.icon))
        .into_any_element()
}

/// Body of `section` while there is no debug session.
fn section_body(workbench: &Workbench, section: DebugSection, cx: &App) -> AnyElement {
    let theme = cx.theme();
    let empty = muted(workbench.text(section.empty()), cx);
    if section != DebugSection::Breakpoints {
        return empty;
    }
    let exception = |id: &'static str, label: &str| {
        ListRow::new(id, workbench.text(label))
            .flat()
            .disabled(true)
            .tooltip(workbench.text("welcome-later"))
    };
    div()
        .flex()
        .flex_col()
        .gap(px(theme.metrics.space.xs))
        .child(empty)
        .child(
            div()
                .px(px(theme.metrics.space.sm))
                .child(exception("debug-all-exceptions", "debug-all-exceptions"))
                .child(exception(
                    "debug-uncaught-exceptions",
                    "debug-uncaught-exceptions",
                )),
        )
        .into_any_element()
}

fn section(
    workbench: &Workbench,
    section: DebugSection,
    focus: Option<&FocusHandle>,
    cx: &mut Context<Workbench>,
) -> AnyElement {
    let theme = cx.theme();
    let metrics = &theme.metrics;
    let ring = theme.colors.border_focus;
    let expanded = workbench.debug_sections.is_expanded(section);
    let mut title = SectionHeader::group(workbench.text(section.title()), Some(expanded));
    if section == DebugSection::Watch {
        title = title.action(
            IconButton::new("debug-watch-add", IconName::Plus)
                .disabled(true)
                .tooltip(workbench.text("welcome-later")),
        );
    }
    let mut header = div()
        .id(("debug-section", section.index()))
        .cursor_pointer()
        .border_1()
        .border_color(gpui::transparent_black())
        .rounded(px(metrics.radius.md))
        .focus_visible(move |style| style.border_color(ring))
        .on_click(cx.listener(move |workbench, _, _, cx| {
            workbench.toggle_debug_section(section, cx);
        }))
        .on_key_down(cx.listener(move |workbench, event: &KeyDownEvent, _, cx| {
            let modifiers = event.keystroke.modifiers;
            let plain = !(modifiers.control || modifiers.alt || modifiers.platform);
            if plain && matches!(event.keystroke.key.as_str(), "enter" | "space") {
                workbench.toggle_debug_section(section, cx);
                cx.stop_propagation();
            }
        }))
        .child(title);
    if let Some(focus) = focus {
        header = header.track_focus(&focus.clone().tab_stop(true));
    }
    let gap = metrics.space.md;
    let divider = (section != DebugSection::Variables).then(|| {
        div()
            .mx(px(metrics.space.lg))
            .mb(px(gap))
            .border_t_1()
            .border_color(theme.colors.border_panel)
    });
    div()
        .flex()
        .flex_col()
        .flex_none()
        .gap(px(gap))
        .children(divider)
        .child(header)
        .children(expanded.then(|| section_body(workbench, section, cx)))
        .into_any_element()
}

pub(crate) fn render(
    workbench: &Workbench,
    cx: &mut Context<Workbench>,
) -> impl IntoElement + use<> {
    let headers = workbench.debug_sections.headers.clone();
    let sections: Vec<AnyElement> = DebugSection::ALL
        .into_iter()
        .map(|kind| {
            let focus = headers.as_ref().map(|headers| &headers[kind.index()]);
            section(workbench, kind, focus, cx)
        })
        .collect();
    let configuration = configuration(workbench, cx);
    let gap = cx.theme().metrics.space.lg;
    div()
        .id("run-and-debug")
        .flex()
        .flex_col()
        .flex_1()
        .min_h_0()
        .overflow_y_scroll()
        .gap(px(gap))
        .child(configuration)
        .children(sections)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sections_start_expanded_and_toggle_alone() {
        let mut sections = DebugSections::default();
        assert!(
            DebugSection::ALL
                .iter()
                .all(|&kind| sections.is_expanded(kind))
        );
        sections.toggle(DebugSection::Watch);
        assert!(!sections.is_expanded(DebugSection::Watch));
        assert!(sections.is_expanded(DebugSection::Variables));
        sections.toggle(DebugSection::Watch);
        assert!(sections.is_expanded(DebugSection::Watch));
    }

    #[test]
    fn every_section_has_its_own_index() {
        let indices: std::collections::HashSet<_> =
            DebugSection::ALL.iter().map(|kind| kind.index()).collect();
        assert_eq!(indices.len(), DebugSection::ALL.len());
    }
}
