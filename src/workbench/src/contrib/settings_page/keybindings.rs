//! The Keyboard Shortcuts tab: every command with its bindings; Change
//! records the next key press, Remove drops a binding.

use gpui::{
    AnyElement, Context, IntoElement, KeyDownEvent, ParentElement as _, Styled as _, Window, div,
    px, uniform_list,
};
use theme::ActiveTheme as _;
use ui::{Button, ButtonVariant, Kbd};

use super::{KeybindingEntry, SettingsPage, SettingsPageEvent};

/// Share of the row width for the command title.
const TITLE_SHARE: f32 = 0.45;

/// Rows matching `query` (lower case) in title, id, keys or context.
pub(crate) fn visible_bindings<'a>(
    entries: &'a [KeybindingEntry],
    query: &str,
) -> Vec<(usize, &'a KeybindingEntry)> {
    entries
        .iter()
        .enumerate()
        .filter(|(_, entry)| {
            query.is_empty()
                || entry.title.to_lowercase().contains(query)
                || entry.command.to_lowercase().contains(query)
                || entry.context.to_lowercase().contains(query)
                || entry
                    .keys_label
                    .as_ref()
                    .is_some_and(|keys| keys.to_lowercase().contains(query))
        })
        .collect()
}

/// What every row needs.
struct RowContext {
    recording: Option<usize>,
    texts: super::SettingsPageTexts,
    page: gpui::WeakEntity<SettingsPage>,
    metrics: theme::Metrics,
    colors: theme::ColorTokens,
    row_height: f32,
}

/// One binding: title and id, keys, context, actions.
fn binding_row(row: &RowContext, index: usize, entry: &KeybindingEntry) -> gpui::Div {
    let RowContext {
        recording,
        texts,
        page,
        metrics,
        colors,
        row_height,
    } = row;
    let (recording, row_height) = (*recording, *row_height);
    let keys: AnyElement = if recording == Some(index) {
        div()
            .text_color(colors.text_accent)
            .child(texts.get("keybindings-record"))
            .into_any_element()
    } else if let Some(label) = &entry.keys_label {
        Kbd::new(label.clone()).into_any_element()
    } else {
        div().into_any_element()
    };
    let change_page = page.clone();
    let mut actions = div().flex().gap(px(metrics.space.xs)).child(
        Button::new(
            ("keybinding-change", index),
            if entry.keys.is_some() {
                texts.get("keybindings-change")
            } else {
                texts.get("keybindings-add")
            },
        )
        .variant(ButtonVariant::Ghost)
        .on_activate(move |window, cx| {
            let _ = change_page.update(cx, |page, cx| {
                page.recording = Some(index);
                page.focus_handle.focus(window, cx);
                cx.notify();
            });
        }),
    );
    if let Some(keys) = entry.keys.clone() {
        let remove_page = page.clone();
        let command = entry.command.clone();
        let context = entry.context.clone();
        let user = entry.user;
        actions = actions.child(
            Button::new(
                ("keybinding-remove", index),
                texts.get("keybindings-remove"),
            )
            .variant(ButtonVariant::Ghost)
            .on_activate(move |_, cx| {
                let event = SettingsPageEvent::Unbind {
                    command: command.clone(),
                    context: context.clone(),
                    keys: keys.clone(),
                    user,
                };
                let _ = remove_page.update(cx, |_, cx| cx.emit(event));
            }),
        );
    }
    let mut context = div()
        .flex()
        .flex_1()
        .gap(px(metrics.space.sm))
        .text_color(colors.text_subtle)
        .child(entry.context.clone());
    if entry.user {
        context = context.child(
            div()
                .text_color(colors.text_accent)
                .child(texts.get("keybindings-user")),
        );
    }
    div()
        .flex()
        .w_full()
        .items_center()
        .h(px(row_height))
        .px(px(metrics.space.xl))
        .border_b_1()
        .border_color(colors.border_divider)
        .text_size(px(metrics.font_size.base))
        .child(
            div()
                .w(gpui::relative(TITLE_SHARE))
                .flex()
                .flex_col()
                .child(div().truncate().child(entry.title.clone()))
                .child(
                    div()
                        .truncate()
                        .text_size(px(metrics.font_size.sm))
                        .text_color(colors.text_muted)
                        .child(entry.command.clone()),
                ),
        )
        .child(div().flex_1().child(keys))
        .child(context)
        .child(actions)
}

impl SettingsPage {
    pub(crate) fn keybindings_body(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let query = self.query_text(cx);
        let rows: Vec<(usize, KeybindingEntry)> = visible_bindings(&self.data.keybindings, &query)
            .into_iter()
            .map(|(index, entry)| (index, entry.clone()))
            .collect();
        let recording = self.recording;
        let texts = self.texts.clone();
        let page = cx.entity().downgrade();
        let theme = cx.theme();
        let metrics = theme.metrics.clone();
        let colors = theme.colors.clone();
        let row_height = metrics.size.list_row_height + metrics.space.sm * 2.0;
        let header = div()
            .flex()
            .px(px(metrics.space.xl))
            .pb(px(metrics.space.sm))
            .text_size(px(metrics.font_size.sm))
            .text_color(colors.text_subtle)
            .child(
                div()
                    .w(gpui::relative(TITLE_SHARE))
                    .child(texts.get("keybindings-command")),
            )
            .child(div().flex_1().child(texts.get("keybindings-keys")))
            .child(div().flex_1().child(texts.get("keybindings-context")));
        let count = rows.len();
        let context = RowContext {
            recording,
            texts,
            page,
            metrics,
            colors,
            row_height,
        };
        let list = uniform_list("keybindings-list", count, move |range, _, _| {
            range
                .map(|position| {
                    let (index, entry) = &rows[position];
                    binding_row(&context, *index, entry)
                })
                .collect()
        })
        .size_full();
        div()
            .flex()
            .flex_col()
            .size_full()
            .child(header)
            .child(div().flex_1().min_h_0().child(list))
            .into_any_element()
    }

    /// While recording, the next key press becomes the binding; Escape
    /// cancels.
    pub(crate) fn on_key_down(
        &mut self,
        event: &KeyDownEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(index) = self.recording else {
            return;
        };
        cx.stop_propagation();
        let keystroke = &event.keystroke;
        if keystroke.key == "escape" && !keystroke.modifiers.modified() {
            self.recording = None;
            cx.notify();
            return;
        }
        let Some(entry) = self.data.keybindings.get(index) else {
            self.recording = None;
            return;
        };
        cx.emit(SettingsPageEvent::Bind {
            command: entry.command.clone(),
            context: entry.context.clone(),
            previous: entry.keys.clone(),
            keystroke: keystroke.clone(),
        });
        self.recording = None;
        cx.notify();
    }
}
