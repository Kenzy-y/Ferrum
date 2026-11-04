//! Keys the editor handles itself are not taken by root actions.

#[cfg(test)]
mod tests {
    use gpui::{KeyBinding, KeyContext, Keystroke, TestAppContext, actions};

    actions!(test, [MoveFocus]);

    #[gpui::test]
    fn tab_inside_the_editor_is_not_a_root_action(cx: &mut TestAppContext) {
        cx.update(|cx| {
            // What the component library binds at the window root.
            cx.bind_keys([KeyBinding::new("tab", MoveFocus, Some("Root"))]);
            editor::init(cx);
            let contexts = [
                KeyContext::parse("Root").unwrap(),
                KeyContext::parse(editor::CONTEXT).unwrap(),
            ];
            let keymap = cx.key_bindings();
            let keymap = keymap.borrow();
            for key in ["tab", "shift-tab"] {
                let (bindings, _) =
                    keymap.bindings_for_input(&[Keystroke::parse(key).unwrap()], &contexts);
                assert!(bindings.is_empty(), "{key} reaches the editor keymap");
            }
            // Outside the editor the root still moves the focus.
            let (bindings, _) =
                keymap.bindings_for_input(&[Keystroke::parse("tab").unwrap()], &contexts[..1]);
            assert_eq!(bindings.len(), 1);
        });
    }
}
