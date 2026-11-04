//! Interaction states shared by interactive components.

use std::rc::Rc;

use gpui::{
    App, Div, ElementId, Hsla, InteractiveElement as _, KeyDownEvent, SharedString, Stateful,
    StatefulInteractiveElement as _, Styled as _, Window, div, transparent_black,
};
use theme::ActiveTheme as _;

/// A visual state of an interactive component.
///
/// Normally the state follows the pointer and the keyboard. Passing a state
/// to a component's `force_state` renders it statically, which the component
/// gallery and tests use to show every state side by side.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InteractionState {
    /// At rest.
    Normal,
    /// The pointer is over the component.
    Hover,
    /// The component is being pressed.
    Pressed,
    /// The component has keyboard focus.
    Focused,
    /// The component cannot be used.
    Disabled,
}

impl InteractionState {
    /// Every state, in the order the gallery shows them.
    pub const ALL: [Self; 5] = [
        Self::Normal,
        Self::Hover,
        Self::Pressed,
        Self::Focused,
        Self::Disabled,
    ];
}

/// Colors of one component in each pointer state.
#[derive(Debug, Clone, Copy)]
pub(crate) struct StateColors {
    pub(crate) normal: Hsla,
    pub(crate) hover: Hsla,
    pub(crate) pressed: Hsla,
}

impl StateColors {
    /// The same color in every state.
    pub(crate) fn fixed(color: Hsla) -> Self {
        Self {
            normal: color,
            hover: color,
            pressed: color,
        }
    }

    pub(crate) fn for_state(self, state: InteractionState) -> Hsla {
        match state {
            InteractionState::Hover => self.hover,
            InteractionState::Pressed => self.pressed,
            InteractionState::Normal | InteractionState::Focused | InteractionState::Disabled => {
                self.normal
            }
        }
    }
}

/// Handler run when an interactive component is activated by a click or by
/// Enter / Space while focused.
pub(crate) type ActivateHandler = Rc<dyn Fn(&mut Window, &mut App)>;

/// What every interactive component has in common.
pub(crate) struct Interactivity {
    pub(crate) id: ElementId,
    pub(crate) disabled: bool,
    pub(crate) forced: Option<InteractionState>,
    pub(crate) on_activate: Option<ActivateHandler>,
    /// Text color while hovered, for components whose hover changes text.
    pub(crate) hover_text: Option<Hsla>,
    /// Focus ring color when the default one does not contrast with the
    /// background (accent buttons).
    pub(crate) focus_ring: Option<Hsla>,
    /// Text shown on hover, also when disabled (it can explain why).
    pub(crate) tooltip: Option<SharedString>,
}

impl Interactivity {
    pub(crate) fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            disabled: false,
            forced: None,
            on_activate: None,
            hover_text: None,
            focus_ring: None,
            tooltip: None,
        }
    }

    /// The state to draw statically: the forced one, or disabled / normal.
    pub(crate) fn static_state(&self) -> InteractionState {
        self.forced.unwrap_or(if self.disabled {
            InteractionState::Disabled
        } else {
            InteractionState::Normal
        })
    }

    /// Builds the root element: background per state, a focus ring (a 1px
    /// border that is transparent unless focused), keyboard focus with Tab,
    /// activation by click, Enter or Space, and reduced opacity when
    /// disabled.
    pub(crate) fn root(
        self,
        background: StateColors,
        window: &mut Window,
        cx: &mut App,
    ) -> Stateful<Div> {
        self.root_with_border(background, transparent_black(), window, cx)
    }

    /// Like [`Self::root`] with a visible border at rest; focus replaces its
    /// color with the focus ring.
    pub(crate) fn root_with_border(
        self,
        background: StateColors,
        border: Hsla,
        window: &mut Window,
        cx: &mut App,
    ) -> Stateful<Div> {
        let focus_handle = window
            .use_keyed_state(self.id.clone(), cx, |_, cx| cx.focus_handle())
            .read(cx)
            .clone()
            .tab_stop(!self.disabled && self.forced.is_none());
        let theme = cx.theme();
        let ring: Hsla = self
            .focus_ring
            .unwrap_or_else(|| theme.colors.border_focus.into());
        let disabled_opacity = theme.opacity.disabled;
        let state = self.static_state();
        let hover_text = self.hover_text;

        let mut element = div()
            .id(self.id)
            .flex()
            .items_center()
            .flex_none()
            .border_1()
            .border_color(if state == InteractionState::Focused {
                ring
            } else {
                border
            })
            .bg(background.for_state(state));

        if let Some(text) = self.tooltip {
            element = element.tooltip(crate::tooltip::Tooltip::builder(text));
        }
        if state == InteractionState::Disabled {
            return element.opacity(disabled_opacity);
        }
        if self.forced.is_some() {
            return element;
        }

        element = element
            .track_focus(&focus_handle)
            .hover(move |style| {
                let style = style.bg(background.hover);
                match hover_text {
                    Some(color) => style.text_color(color),
                    None => style,
                }
            })
            .active(move |style| style.bg(background.pressed))
            .focus_visible(move |style| style.border_color(ring));
        if let Some(handler) = self.on_activate {
            let on_key = handler.clone();
            element = element
                .cursor_pointer()
                .on_click(move |_, window, cx| handler(window, cx))
                .on_key_down(move |event: &KeyDownEvent, window, cx| {
                    if is_activation_key(event) {
                        cx.stop_propagation();
                        on_key(window, cx);
                    }
                });
        }
        element
    }
}

/// Enter or Space without modifiers.
fn is_activation_key(event: &KeyDownEvent) -> bool {
    let keystroke = &event.keystroke;
    let modifiers = keystroke.modifiers;
    !modifiers.control
        && !modifiers.alt
        && !modifiers.platform
        && matches!(keystroke.key.as_str(), "enter" | "space")
}

/// Declares the builder methods every interactive component shares. The
/// type must have an `interactivity: Interactivity` field.
macro_rules! interactive_builders {
    ($type:ty) => {
        impl $type {
            /// Disables the component.
            #[must_use]
            pub fn disabled(mut self, disabled: bool) -> Self {
                self.interactivity.disabled = disabled;
                self
            }

            /// Renders the component statically in `state` (component
            /// gallery, tests). Pointer and keyboard are ignored.
            #[must_use]
            pub fn force_state(mut self, state: $crate::InteractionState) -> Self {
                self.interactivity.forced = Some(state);
                self
            }

            /// Shows `text` when hovered, also when disabled.
            #[must_use]
            pub fn tooltip(mut self, text: impl Into<gpui::SharedString>) -> Self {
                self.interactivity.tooltip = Some(text.into());
                self
            }

            /// Runs `handler` on click, or on Enter / Space when focused.
            #[must_use]
            pub fn on_activate(
                mut self,
                handler: impl Fn(&mut gpui::Window, &mut gpui::App) + 'static,
            ) -> Self {
                self.interactivity.on_activate = Some(std::rc::Rc::new(handler));
                self
            }
        }
    };
}

pub(crate) use interactive_builders;
