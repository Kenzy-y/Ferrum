//! Drawing the Settings tab and the page frame.

use gpui::{
    AnyElement, Context, FontWeight, InteractiveElement as _, IntoElement, ParentElement as _,
    Render, StatefulInteractiveElement as _, Styled as _, Window, div, px,
};
use settings::LayerKind;
use theme::ActiveTheme as _;
use ui::{Button, ButtonVariant, IconName, ListRow, SegmentedTab, TextField};

use super::{SettingItem, SettingsPage, SettingsPageEvent, SettingsPageTab, visible_items};

/// Width of the namespace list, in space tokens `xl`.
const NAV_WIDTH_STEPS: f32 = 9.0;
/// Widest readable column of settings, in space tokens `xl`.
const CONTENT_MAX_STEPS: f32 = 40.0;

impl Render for SettingsPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.placeholder_tab != Some(self.tab) {
            self.placeholder_tab = Some(self.tab);
            let placeholder = self.texts.get(match self.tab {
                SettingsPageTab::Settings => "settings-page-search",
                SettingsPageTab::Keybindings => "keybindings-search",
            });
            self.query.update(cx, |query, cx| {
                query.set_placeholder(placeholder, window, cx);
            });
        }
        let body = match self.tab {
            SettingsPageTab::Settings => self.settings_body(window, cx),
            SettingsPageTab::Keybindings => self.keybindings_body(window, cx),
        };
        let header = self.header(cx);
        let theme = cx.theme();
        let colors = &theme.colors;
        let metrics = &theme.metrics;
        div()
            .id("settings-page")
            .track_focus(&self.focus_handle)
            .key_context("settings_page")
            .capture_key_down(cx.listener(Self::on_key_down))
            .flex()
            .flex_col()
            .size_full()
            .bg(colors.bg_editor)
            .text_color(colors.text_primary)
            .child(header)
            .child(
                div()
                    .px(px(metrics.space.xl))
                    .pt(px(metrics.space.lg))
                    .child(TextField::new(&self.query).icon(IconName::Search).raised()),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .pt(px(theme.metrics.space.lg))
                    .child(body),
            )
    }
}

impl SettingsPage {
    fn header(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let metrics = &theme.metrics;
        let page = cx.entity().downgrade();
        let tab = |id: &'static str, label, which: SettingsPageTab| {
            let page = page.clone();
            SegmentedTab::new(id, label)
                .active(self.tab == which)
                .on_activate(move |_, cx| {
                    let _ = page.update(cx, |page, cx| page.set_tab(which, cx));
                })
        };
        let mut right = div().flex().items_center().gap(px(metrics.space.md));
        if self.tab == SettingsPageTab::Settings {
            let layer_tab = |id: &'static str, label, layer: LayerKind, enabled: bool| {
                let page = page.clone();
                let mut tab = SegmentedTab::new(id, label)
                    .active(self.layer == layer)
                    .disabled(!enabled);
                if !enabled {
                    tab = tab.tooltip(self.texts.get("settings-page-no-workspace"));
                }
                tab.on_activate(move |_, cx| {
                    let _ = page.update(cx, |page, cx| page.set_layer(layer, cx));
                })
            };
            right = right
                .child(layer_tab(
                    "settings-layer-user",
                    self.texts.get("settings-page-layer-user"),
                    LayerKind::User,
                    true,
                ))
                .child(layer_tab(
                    "settings-layer-workspace",
                    self.texts.get("settings-page-layer-workspace"),
                    LayerKind::Workspace,
                    self.data.has_workspace,
                ));
        }
        let layer = self.layer;
        let open_label = match self.tab {
            SettingsPageTab::Settings => self.texts.get("settings-page-open-file"),
            SettingsPageTab::Keybindings => self.texts.get("keybindings-open-file"),
        };
        let keymap = self.tab == SettingsPageTab::Keybindings;
        let open_page = page.clone();
        right = right.child(
            Button::new("settings-open-file", open_label)
                .variant(ButtonVariant::Ghost)
                .on_activate(move |_, cx| {
                    let _ = open_page.update(cx, |_, cx| {
                        cx.emit(if keymap {
                            SettingsPageEvent::OpenKeymapFile
                        } else {
                            SettingsPageEvent::OpenFile(layer)
                        });
                    });
                }),
        );
        div()
            .flex()
            .flex_none()
            .items_center()
            .justify_between()
            .h(px(metrics.size.editor_header_height))
            .mx(px(metrics.space.lg))
            .px(px(metrics.space.sm))
            .border_b_1()
            .border_color(theme.colors.border_divider)
            .child(
                div()
                    .flex()
                    .gap(px(metrics.space.lg))
                    .child(tab(
                        "settings-tab-settings",
                        self.texts.get("settings-page-tab-settings"),
                        SettingsPageTab::Settings,
                    ))
                    .child(tab(
                        "settings-tab-keybindings",
                        self.texts.get("settings-page-tab-keybindings"),
                        SettingsPageTab::Keybindings,
                    )),
            )
            .child(right)
            .into_any_element()
    }

    fn settings_body(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let query = self.query_text(cx);
        let groups = self.data.groups.clone();
        let visible = visible_items(&groups, self.group.as_deref(), &query);
        let page = cx.entity().downgrade();

        let theme = cx.theme();
        let metrics = theme.metrics.clone();
        let colors = theme.colors.clone();
        let mut nav = div()
            .id("settings-nav")
            .flex_none()
            .w(px(metrics.space.xl * NAV_WIDTH_STEPS))
            .h_full()
            .overflow_y_scroll()
            .px(px(metrics.space.lg))
            .child({
                let page = page.clone();
                ListRow::new("settings-nav-all", self.texts.get("settings-page-all"))
                    .selected(self.group.is_none())
                    .on_activate(move |_, cx| {
                        let _ = page.update(cx, |page, cx| {
                            page.group = None;
                            cx.notify();
                        });
                    })
            });
        for (index, group) in groups.iter().enumerate() {
            let page = page.clone();
            let namespace = group.namespace.clone();
            nav = nav.child(
                ListRow::new(("settings-nav", index), group.title.clone())
                    .selected(self.group.as_deref() == Some(group.namespace.as_str()))
                    .on_activate(move |_, cx| {
                        let namespace = namespace.clone();
                        let _ = page.update(cx, |page, cx| {
                            page.group = Some(namespace);
                            cx.notify();
                        });
                    }),
            );
        }

        let mut content = div()
            .id("settings-content")
            .flex_1()
            .h_full()
            .overflow_y_scroll()
            .px(px(metrics.space.xl))
            .pb(px(metrics.space.xl));
        if visible.is_empty() {
            content = content.child(
                div()
                    .text_size(px(metrics.font_size.md))
                    .text_color(colors.text_muted)
                    .child(self.texts.get("settings-page-no-results")),
            );
        }
        for (group, items) in visible {
            let mut section = div()
                .flex()
                .flex_col()
                .max_w(px(metrics.space.xl * CONTENT_MAX_STEPS))
                .mb(px(metrics.space.xl))
                .child(
                    div()
                        .text_size(px(metrics.font_size.lg))
                        .font_weight(FontWeight::SEMIBOLD)
                        .mb(px(metrics.space.md))
                        .child(group.title.clone()),
                );
            for item in items {
                section = section.child(self.item(item, window, cx));
            }
            content = content.child(section);
        }
        div()
            .flex()
            .size_full()
            .child(nav)
            .child(content)
            .into_any_element()
    }

    fn item(
        &mut self,
        item: &SettingItem,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let control = self.control(item, window, cx);
        let id = format!("{}.{}", item.namespace, item.key);
        let modified = item.in_layer(self.layer).is_some();
        let invalid = self.invalid.as_deref() == Some(id.as_str());
        let theme = cx.theme();
        let metrics = &theme.metrics;
        let colors = &theme.colors;
        let mut title = div()
            .flex()
            .items_center()
            .gap(px(metrics.space.sm))
            .text_size(px(metrics.font_size.md))
            .font_weight(FontWeight::MEDIUM)
            .child(
                div()
                    .text_color(colors.text_subtle)
                    .child(format!("{}: ", item.namespace)),
            )
            .child(item.title.clone());
        if modified {
            let target = item.clone();
            let page = cx.entity().downgrade();
            title = title
                .child(
                    div()
                        .text_size(px(metrics.font_size.sm))
                        .text_color(colors.text_accent)
                        .child(self.texts.get("settings-page-modified")),
                )
                .child(
                    Button::new(
                        gpui::SharedString::from(format!("reset-{id}")),
                        self.texts.get("settings-page-reset"),
                    )
                    .variant(ButtonVariant::Ghost)
                    .on_activate(move |_, cx| {
                        let _ = page.update(cx, |page, cx| page.change(&target, None, cx));
                    }),
                );
        }
        let mut row = div()
            .flex()
            .flex_col()
            .gap(px(metrics.space.sm))
            .py(px(metrics.space.md))
            .pl(px(metrics.space.md))
            .border_l_2()
            .border_color(if modified {
                colors.accent.into()
            } else {
                gpui::transparent_black()
            })
            .child(title)
            .child(
                div()
                    .text_size(px(metrics.font_size.base))
                    .text_color(colors.text_secondary)
                    .child(item.description.clone()),
            )
            .child(control);
        if invalid {
            row = row.child(
                div()
                    .text_size(px(metrics.font_size.sm))
                    .text_color(colors.diagnostic_error)
                    .child(self.texts.get("settings-page-invalid")),
            );
        }
        row.into_any_element()
    }
}
