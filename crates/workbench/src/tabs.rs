//! Tabs of the editor area: open editors and other views (the component
//! gallery, messages about files that cannot be opened).

use std::path::{Path, PathBuf};

use editor::{Editor, EditorEvent};
use gpui::{
    AnyView, AppContext as _, Context, Entity, Focusable as _, PromptLevel, SharedString,
    Subscription, Window,
};
use i18n::FluentArgs;

use crate::Workbench;
use crate::message_view::MessageView;

/// What a tab shows.
#[derive(Clone)]
pub(crate) enum TabContent {
    Editor(Entity<Editor>),
    /// Any other view; `path` is set for messages about a file, so opening
    /// that file again reuses the tab.
    View {
        view: AnyView,
        path: Option<PathBuf>,
    },
}

/// An open tab.
pub(crate) struct Tab {
    pub(crate) title: SharedString,
    pub(crate) content: TabContent,
    _subscription: Option<Subscription>,
}

impl Tab {
    pub(crate) fn path(&self, cx: &gpui::App) -> Option<PathBuf> {
        match &self.content {
            TabContent::Editor(editor) => editor.read(cx).path().map(Path::to_path_buf),
            TabContent::View { path, .. } => path.clone(),
        }
    }

    pub(crate) fn is_dirty(&self, cx: &gpui::App) -> bool {
        match &self.content {
            TabContent::Editor(editor) => editor.read(cx).is_dirty(),
            TabContent::View { .. } => false,
        }
    }
}

/// A tab title: the file name of `path`.
pub(crate) fn file_title(path: &Path) -> SharedString {
    path.file_name()
        .map_or_else(
            || path.display().to_string(),
            |name| name.to_string_lossy().into_owned(),
        )
        .into()
}

impl Workbench {
    /// The editor of `path` if it is open.
    pub fn editor_for(&self, path: &Path, cx: &gpui::App) -> Option<Entity<Editor>> {
        self.tabs.iter().find_map(|tab| match &tab.content {
            TabContent::Editor(editor) if editor.read(cx).path() == Some(path) => {
                Some(editor.clone())
            }
            _ => None,
        })
    }

    /// The editor of the active tab.
    pub fn active_editor(&self) -> Option<Entity<Editor>> {
        match &self.tabs.get(self.active_tab?)?.content {
            TabContent::Editor(editor) => Some(editor.clone()),
            TabContent::View { .. } => None,
        }
    }

    /// Every open editor.
    pub fn editors(&self) -> impl Iterator<Item = &Entity<Editor>> {
        self.tabs.iter().filter_map(|tab| match &tab.content {
            TabContent::Editor(editor) => Some(editor),
            TabContent::View { .. } => None,
        })
    }

    /// Shows `editor` in a new tab, or the tab already showing its file
    /// (replacing a message about that file), and focuses it.
    pub fn open_editor(
        &mut self,
        editor: Entity<Editor>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let path = editor.read(cx).path().map(Path::to_path_buf);
        if let Some(path) = &path
            && let Some(index) = self.tab_index_for(path, cx)
        {
            if matches!(self.tabs[index].content, TabContent::Editor(_)) {
                self.activate_tab(index, window, cx);
                return;
            }
            self.tabs.remove(index);
        }
        let title = path
            .as_deref()
            .map_or_else(|| self.text("editor-untitled"), file_title);
        let subscription = cx.subscribe(&editor, |workbench, editor, event: &EditorEvent, cx| {
            match event {
                EditorEvent::SaveFailed(message) => tracing::error!(%message, "saving failed"),
                EditorEvent::OutlineChanged
                    if workbench.active_editor().as_ref() == Some(&editor) =>
                {
                    workbench.sync_outline(cx);
                }
                EditorEvent::Renamed => {
                    let path = editor.read(cx).path().map(Path::to_path_buf);
                    if let Some(path) = path
                        && let Some(tab) = workbench.tabs.iter_mut().find(|tab| {
                            matches!(&tab.content, TabContent::Editor(open) if open == &editor)
                        })
                    {
                        tab.title = file_title(&path);
                    }
                }
                _ => {}
            }
            cx.notify();
        });
        self.tabs.push(Tab {
            title,
            content: TabContent::Editor(editor),
            _subscription: Some(subscription),
        });
        self.activate_tab(self.tabs.len() - 1, window, cx);
    }

    /// Shows a message about `path` (it cannot be opened) in its tab.
    pub fn show_file_message(
        &mut self,
        path: PathBuf,
        message: SharedString,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let index = self.tab_index_for(&path, cx);
        let view = cx.new(|_| MessageView::new(message)).into();
        let tab = Tab {
            title: file_title(&path),
            content: TabContent::View {
                view,
                path: Some(path),
            },
            _subscription: None,
        };
        let index = if let Some(index) = index {
            self.tabs[index] = tab;
            index
        } else {
            self.tabs.push(tab);
            self.tabs.len() - 1
        };
        self.activate_tab(index, window, cx);
    }

    fn tab_index_for(&self, path: &Path, cx: &gpui::App) -> Option<usize> {
        self.tabs
            .iter()
            .position(|tab| tab.path(cx).as_deref() == Some(path))
    }

    /// Makes tab `index` active and focuses its editor.
    pub(crate) fn activate_tab(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if index >= self.tabs.len() {
            return;
        }
        self.active_tab = Some(index);
        if let TabContent::Editor(editor) = &self.tabs[index].content {
            let focus = editor.read(cx).focus_handle(cx);
            focus.focus(window, cx);
        }
        self.sync_outline(cx);
        cx.notify();
    }

    /// Activates the next (`step` 1) or previous (`step` -1) tab, wrapping.
    pub(crate) fn cycle_tab(&mut self, forward: bool, window: &mut Window, cx: &mut Context<Self>) {
        let count = self.tabs.len();
        if let Some(active) = self.active_tab
            && count > 1
        {
            let next = if forward {
                (active + 1) % count
            } else {
                (active + count - 1) % count
            };
            self.activate_tab(next, window, cx);
        }
    }

    /// Closes tab `index`; an editor with unsaved edits asks first whether
    /// to save them.
    pub(crate) fn close_tab(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(tab) = self.tabs.get(index) else {
            return;
        };
        if !tab.is_dirty(cx) {
            self.remove_tab(index, window, cx);
            return;
        }
        let mut args = FluentArgs::new();
        args.set("name", tab.title.to_string());
        let message = self.i18n.text_with("editor-close-unsaved", &args);
        let detail = self.i18n.text("editor-close-unsaved-detail");
        let answers = [
            self.i18n.text("editor-close-save"),
            self.i18n.text("editor-close-discard"),
            self.i18n.text("editor-close-cancel"),
        ];
        let answer = window.prompt(
            PromptLevel::Warning,
            &message,
            Some(&detail),
            &answers.iter().map(String::as_str).collect::<Vec<_>>(),
            cx,
        );
        let content = tab.content.clone();
        cx.spawn_in(window, async move |workbench, cx| {
            let Ok(answer) = answer.await else {
                return;
            };
            let _ = workbench.update_in(cx, |workbench, window, cx| {
                // The tab may have moved while the dialog was open.
                let Some(index) = workbench
                    .tabs
                    .iter()
                    .position(|tab| same_content(&tab.content, &content))
                else {
                    return;
                };
                match answer {
                    0 => {
                        if let TabContent::Editor(editor) = &content {
                            // The write keeps running after the editor is gone.
                            editor.update(cx, Editor::save);
                        }
                        workbench.remove_tab(index, window, cx);
                    }
                    1 => workbench.remove_tab(index, window, cx),
                    _ => {}
                }
            });
        })
        .detach();
    }

    /// Closes the active tab.
    pub(crate) fn close_active_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(index) = self.active_tab {
            self.close_tab(index, window, cx);
        }
    }

    fn remove_tab(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.tabs.remove(index);
        self.active_tab = match self.active_tab {
            _ if self.tabs.is_empty() => None,
            Some(active) if active > index => Some(active - 1),
            Some(active) if active == index => Some(index.min(self.tabs.len() - 1)),
            other => other,
        };
        if let Some(active) = self.active_tab {
            self.activate_tab(active, window, cx);
        } else {
            self.sync_outline(cx);
        }
        cx.notify();
    }

    /// Shows `view` in a tab titled `title`, or removes that tab (`None`).
    /// For views that are not editors (the component gallery).
    pub fn set_view_tab(
        &mut self,
        title: SharedString,
        view: Option<AnyView>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let existing = self.tabs.iter().position(|tab| {
            matches!(&tab.content, TabContent::View { path: None, .. }) && tab.title == title
        });
        match (existing, view) {
            (Some(index), None) => self.remove_tab(index, window, cx),
            (Some(index), Some(_)) => self.activate_tab(index, window, cx),
            (None, Some(view)) => {
                self.tabs.push(Tab {
                    title,
                    content: TabContent::View { view, path: None },
                    _subscription: None,
                });
                self.activate_tab(self.tabs.len() - 1, window, cx);
            }
            (None, None) => {}
        }
    }

    /// Whether a view tab titled `title` is open.
    pub fn has_view_tab(&self, title: &str) -> bool {
        self.tabs.iter().any(|tab| {
            matches!(&tab.content, TabContent::View { path: None, .. }) && tab.title == title
        })
    }
}

fn same_content(left: &TabContent, right: &TabContent) -> bool {
    match (left, right) {
        (TabContent::Editor(left), TabContent::Editor(right)) => left == right,
        (TabContent::View { view: left, .. }, TabContent::View { view: right, .. }) => {
            left == right
        }
        _ => false,
    }
}
