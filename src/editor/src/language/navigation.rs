//! Go to definition (`editor.goToDefinition`, F12, or a click with the
//! go-to modifier): a definition in the same file moves the cursor there,
//! one in another file is reported with [`EditorEvent::OpenLocation`] for
//! the owner to open.

use gpui::Context;

use crate::editor::{Editor, EditorEvent};

impl Editor {
    /// Goes to the definition of the symbol at `offset`.
    pub(crate) fn go_to_definition(&mut self, offset: usize, cx: &mut Context<Self>) {
        let point = self.document.buffer().offset_to_point(offset);
        let Some(language) = &mut self.language else {
            return;
        };
        if !language.capabilities.definition {
            return;
        }
        let request = language.service.definition(&language.path, point);
        language.definition_task = Some(cx.spawn(async move |editor, cx| {
            let Ok(locations) = request.await else {
                return;
            };
            let Some(location) = locations.into_iter().next() else {
                return;
            };
            let _ = editor.update(cx, |editor, cx| {
                if editor.path() == Some(location.path.as_path()) {
                    let offset = editor
                        .document
                        .buffer()
                        .point_to_offset(location.range.start);
                    editor.go_to(offset, cx);
                } else {
                    cx.emit(EditorEvent::OpenLocation(location));
                }
            });
        }));
    }
}
