//! The control of each setting kind, and turning typed text into values.

use gpui::{
    AnyElement, AppContext as _, Context, Entity, Focusable as _, IntoElement, ParentElement as _,
    Styled as _, Window, div, px,
};
use gpui_component::input::{InputEvent, InputState};
use settings::FieldKind;
use theme::ActiveTheme as _;
use ui::{Button, ButtonVariant, SegmentedTab, Switch, TextField};

use super::{SettingItem, SettingsPage, SettingsPageEvent};

/// Width of text fields of numbers, in space tokens `xl`.
const NUMBER_FIELD_STEPS: f32 = 6.0;
/// Width of text fields of strings, in space tokens `xl`.
const TEXT_FIELD_STEPS: f32 = 18.0;

impl SettingsPage {
    /// The control that edits `item` in the current layer.
    pub(crate) fn control(
        &mut self,
        item: &SettingItem,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let id = format!("{}.{}", item.namespace, item.key);
        let value = item.shown_value(self.layer).cloned();
        match &item.kind {
            FieldKind::Bool => {
                let on = value.and_then(|value| value.as_bool()).unwrap_or(false);
                let target = item.clone();
                let page = cx.entity().downgrade();
                Switch::new(gpui::SharedString::from(format!("switch-{id}")))
                    .on(on)
                    .on_activate(move |_, cx| {
                        let _ = page.update(cx, |page, cx| {
                            page.change(&target, Some(toml::Value::Boolean(!on)), cx);
                        });
                    })
                    .into_any_element()
            }
            FieldKind::Choice(choices) => {
                let current = value
                    .as_ref()
                    .and_then(toml::Value::as_str)
                    .unwrap_or_default()
                    .to_owned();
                let page = cx.entity().downgrade();
                div()
                    .flex()
                    .gap(px(cx.theme().metrics.space.md))
                    .children(choices.iter().map(|choice| {
                        let target = item.clone();
                        let page = page.clone();
                        let value = choice.value.clone();
                        let mut tab = SegmentedTab::new(
                            gpui::SharedString::from(format!("choice-{id}-{value}")),
                            choice.value.clone(),
                        )
                        .active(current == choice.value);
                        if !choice.description.is_empty() {
                            tab = tab.tooltip(choice.description.clone());
                        }
                        tab.on_activate(move |_, cx| {
                            let _ = page.update(cx, |page, cx| {
                                page.change(&target, Some(toml::Value::String(value.clone())), cx);
                            });
                        })
                    }))
                    .into_any_element()
            }
            FieldKind::Integer { .. }
            | FieldKind::Number
            | FieldKind::String
            | FieldKind::StringList => {
                let text = value.as_ref().map(value_text).unwrap_or_default();
                let field = self.text_field(&id, item, &text, window, cx);
                let metrics = &cx.theme().metrics;
                let steps = if matches!(item.kind, FieldKind::Integer { .. } | FieldKind::Number) {
                    NUMBER_FIELD_STEPS
                } else {
                    TEXT_FIELD_STEPS
                };
                div()
                    .w(px(metrics.space.xl * steps))
                    .child(TextField::new(&field))
                    .into_any_element()
            }
            FieldKind::Other => {
                let layer = self.layer;
                let page = cx.entity().downgrade();
                div()
                    .flex()
                    .child(
                        Button::new(
                            gpui::SharedString::from(format!("toml-{id}")),
                            self.texts.get("settings-page-toml-only"),
                        )
                        .variant(ButtonVariant::Secondary)
                        .on_activate(move |_, cx| {
                            let _ = page
                                .update(cx, |_, cx| cx.emit(SettingsPageEvent::OpenFile(layer)));
                        }),
                    )
                    .into_any_element()
            }
        }
    }

    /// The input of `id`, created on first use and refilled when the value
    /// changed elsewhere (and the field is not being edited).
    fn text_field(
        &mut self,
        id: &str,
        item: &SettingItem,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<InputState> {
        if let Some((input, shown, _)) = self.fields.get_mut(id) {
            let input = input.clone();
            let editing = input.read(cx).focus_handle(cx).is_focused(window);
            if shown != text && !editing {
                text.clone_into(shown);
                let text = text.to_owned();
                input.update(cx, |input, cx| input.set_value(text, window, cx));
            }
            return input;
        }
        let initial = text.to_owned();
        let input = cx.new(|cx| InputState::new(window, cx).default_value(initial));
        let target = item.clone();
        let subscription = cx.subscribe(&input, move |page, input, event: &InputEvent, cx| {
            if matches!(event, InputEvent::PressEnter { .. } | InputEvent::Blur) {
                let typed = input.read(cx).value().to_string();
                page.commit_text(&target, &typed, cx);
            }
        });
        self.fields.insert(
            id.to_owned(),
            (input.clone(), text.to_owned(), subscription),
        );
        input
    }

    fn commit_text(&mut self, item: &SettingItem, typed: &str, cx: &mut Context<Self>) {
        let id = format!("{}.{}", item.namespace, item.key);
        let shown = item
            .shown_value(self.layer)
            .map(value_text)
            .unwrap_or_default();
        if typed.trim() == shown.trim() {
            return;
        }
        if let Some(value) = parse_text(&item.kind, typed) {
            self.invalid = None;
            self.change(item, Some(value), cx);
        } else {
            self.invalid = Some(id);
            cx.notify();
        }
    }
}

/// How a value reads in a text field: strings without quotes, lists as
/// comma-separated items.
pub(crate) fn value_text(value: &toml::Value) -> String {
    match value {
        toml::Value::String(text) => text.clone(),
        toml::Value::Array(items) => items.iter().map(value_text).collect::<Vec<_>>().join(", "),
        other => other.to_string(),
    }
}

/// The value typed into the field of `kind`, or `None` if it does not fit.
pub(crate) fn parse_text(kind: &FieldKind, typed: &str) -> Option<toml::Value> {
    let trimmed = typed.trim();
    match kind {
        FieldKind::Integer { minimum, maximum } => {
            let number: i64 = trimmed.parse().ok()?;
            let fits = minimum.is_none_or(|minimum| number >= minimum)
                && maximum.is_none_or(|maximum| number <= maximum);
            fits.then_some(toml::Value::Integer(number))
        }
        FieldKind::Number => {
            let number: f64 = trimmed.parse().ok()?;
            number.is_finite().then_some(toml::Value::Float(number))
        }
        FieldKind::String => Some(toml::Value::String(typed.to_owned())),
        FieldKind::StringList => Some(toml::Value::Array(
            trimmed
                .split(',')
                .map(str::trim)
                .filter(|part| !part.is_empty())
                .map(|part| toml::Value::String(part.to_owned()))
                .collect(),
        )),
        FieldKind::Bool | FieldKind::Choice(_) | FieldKind::Other => None,
    }
}
