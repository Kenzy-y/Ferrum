use super::*;

/// Places cursors at every `|` in `marked` and returns the document.
fn document(marked: &str) -> Document {
    let text = marked.replace('|', "");
    let cursors = marked
        .match_indices('|')
        .enumerate()
        .map(|(removed, (index, _))| Selection::cursor(index - removed))
        .collect();
    let mut document = Document::from_text(&text);
    document.set_selections(&Selections::from_vec(cursors));
    document
}

/// The text with `|` at every cursor head.
fn marked(document: &Document) -> String {
    let mut text = document.buffer().rope().to_string();
    let mut heads: Vec<usize> = document
        .selections()
        .iter()
        .map(|selection| selection.head)
        .collect();
    heads.sort_unstable();
    for head in heads.into_iter().rev() {
        text.insert(head, '|');
    }
    text
}

const FAMILY: &str = "👨\u{200d}👩\u{200d}👧";

#[test]
fn types_at_every_cursor() {
    let mut document = document("a|b\nc|d");
    document.insert("x");
    assert_eq!(marked(&document), "ax|b\ncx|d");
}

#[test]
fn backspace_removes_a_whole_emoji() {
    let mut document = document(&format!("a{FAMILY}|b"));
    document.delete_backward();
    assert_eq!(marked(&document), "a|b");
    document.move_cursors(Movement::Left, false);
    document.delete_forward();
    assert_eq!(marked(&document), "|b");
}

#[test]
fn cursor_moves_over_an_emoji_in_one_step() {
    let mut document = document(&format!("|{FAMILY}x"));
    document.move_cursors(Movement::Right, false);
    assert_eq!(marked(&document), format!("{FAMILY}|x"));
    document.move_cursors(Movement::Left, false);
    assert_eq!(marked(&document), format!("|{FAMILY}x"));
}

#[test]
fn undo_and_redo_restore_cursors_and_selections() {
    let mut document = document("one two");
    document.set_selections(&Selections::from_vec(vec![
        Selection::new(0, 3),
        Selection::new(4, 7),
    ]));
    let before = document.selections().clone();
    document.insert("x");
    assert_eq!(marked(&document), "x| x|");
    let after = document.selections().clone();

    assert!(document.undo());
    assert_eq!(document.buffer().rope().to_string(), "one two");
    assert_eq!(document.selections(), &before, "both selections are back");

    assert!(document.redo());
    assert_eq!(document.selections(), &after);
    assert!(!document.redo());
}

#[test]
fn typing_groups_into_one_undo_step_until_the_cursor_moves() {
    let mut document = document("|");
    for character in ["a", "b", "c"] {
        document.insert(character);
    }
    document.move_cursors(Movement::Left, false);
    document.insert("X");
    assert_eq!(marked(&document), "abX|c");

    document.undo();
    assert_eq!(
        marked(&document),
        "ab|c",
        "the cursor is where it was before X"
    );
    document.undo();
    assert_eq!(marked(&document), "|", "abc was one step");
    assert!(!document.undo());
}

#[test]
fn deleting_with_two_cursors_on_one_character_deletes_it_once() {
    let mut document = document("ab|c|d");
    document.move_cursors(Movement::Left, true);
    document.set_selections(&Selections::from_vec(vec![
        Selection::cursor(3),
        Selection::cursor(3),
    ]));
    document.delete_backward();
    assert_eq!(marked(&document), "ab|d");
}

#[test]
fn vertical_movement_keeps_the_goal_column() {
    let mut document = document("long line|\nab\nlong line");
    document.move_cursors(Movement::Down, false);
    assert_eq!(marked(&document), "long line\nab|\nlong line");
    document.move_cursors(Movement::Down, false);
    assert_eq!(marked(&document), "long line\nab\nlong line|");
    document.move_cursors(Movement::Down, false);
    assert_eq!(
        marked(&document),
        "long line\nab\nlong line|",
        "down on the last line goes to the end"
    );
    document.move_cursors(Movement::DocumentStart, false);
    document.move_cursors(Movement::Up, false);
    assert_eq!(marked(&document), "|long line\nab\nlong line");
}

#[test]
fn smart_home_toggles_between_indentation_and_line_start() {
    let mut document = document("    let x|");
    document.move_cursors(Movement::LineStart, false);
    assert_eq!(marked(&document), "    |let x");
    document.move_cursors(Movement::LineStart, false);
    assert_eq!(marked(&document), "|    let x");
}

#[test]
fn newline_keeps_indentation() {
    let mut document = document("    let x = 1;|");
    document.newline();
    assert_eq!(marked(&document), "    let x = 1;\n    |");
}

#[test]
fn extending_and_collapsing_selections() {
    let mut document = document("hello |world");
    document.move_cursors(Movement::WordRight, true);
    assert_eq!(document.selected_text(), "world");
    document.move_cursors(Movement::Left, false);
    assert_eq!(marked(&document), "hello |world", "collapses to the start");
}

#[test]
fn paste_distributes_lines_over_cursors() {
    let mut document = document("|a\n|b");
    document.paste("1\n2");
    assert_eq!(marked(&document), "1|a\n2|b");
    let mut single = document_with_one_cursor();
    single.paste("x\ny");
    assert_eq!(marked(&single), "x\ny|");
}

fn document_with_one_cursor() -> Document {
    document("|")
}

#[test]
fn add_cursor_below_keeps_column() {
    let mut document = document("ab|cd\nefgh");
    document.add_cursor_vertically(false);
    assert_eq!(marked(&document), "ab|cd\nef|gh");
    document.insert("X");
    assert_eq!(marked(&document), "abX|cd\nefX|gh");
}

#[test]
fn version_changes_only_with_the_text() {
    let mut document = document("a|");
    let version = document.version();
    document.move_cursors(Movement::Left, false);
    assert_eq!(document.version(), version);
    document.insert("b");
    assert!(document.version() > version);
}

/// A clock the test moves by hand.
fn manual_clock() -> (Clock, std::sync::Arc<std::sync::Mutex<std::time::Instant>>) {
    let now = std::sync::Arc::new(std::sync::Mutex::new(std::time::Instant::now()));
    let reader = now.clone();
    (Clock::new(move || *reader.lock().unwrap()), now)
}

#[test]
fn a_pause_in_typing_starts_a_new_undo_step() {
    let (clock, now) = manual_clock();
    let mut document = Document::from_text("").with_clock(clock);
    document.insert("a");
    document.insert("b");
    *now.lock().unwrap() += crate::UNDO_GROUP_PAUSE * 2;
    document.insert("c");

    document.undo();
    assert_eq!(document.buffer().rope().to_string(), "ab");
    document.undo();
    assert_eq!(document.buffer().rope().to_string(), "");
}

#[test]
fn the_end_of_a_word_starts_a_new_undo_step() {
    let mut document = document("|");
    for character in "hello world".chars() {
        document.insert(&character.to_string());
    }
    document.undo();
    assert_eq!(marked(&document), "hello |");
    document.undo();
    assert_eq!(marked(&document), "|");
}

#[test]
fn new_lines_get_the_dominant_ending_and_others_stay() {
    // Two `\n` against one `\r\n`: new lines use `\n`.
    let mut document = document("a\r\nb\nc|\n");
    document.newline();
    document.insert("d");
    assert_eq!(document.buffer().to_text(), "a\r\nb\nc\nd\n");

    let mut windows = Document::from_text("x\r\n");
    windows.paste("1\n2");
    assert_eq!(
        windows.buffer().to_text(),
        "1\r\n2x\r\n",
        "pasted lines get the file's ending"
    );
}

#[test]
fn line_endings_can_be_made_uniform_and_undone() {
    let mut document = document("a\r\nb|\nc\n");
    document.set_line_endings(crate::LineEnding::CrLf);
    assert_eq!(document.buffer().to_text(), "a\r\nb\r\nc\r\n");
    assert_eq!(
        marked(&document).replace("\r\n", "\n"),
        "a\nb|\nc\n",
        "the cursor stays after b"
    );
    assert_eq!(
        document.buffer().line_endings(),
        crate::LineEndings::Uniform(crate::LineEnding::CrLf)
    );
    document.undo();
    assert_eq!(document.buffer().to_text(), "a\r\nb\nc\n");
}

#[test]
fn crlf_is_one_step_for_the_cursor_and_backspace() {
    let mut document = document("a|\r\nb");
    document.move_cursors(Movement::Right, false);
    assert_eq!(marked(&document), "a\r\n|b");
    document.delete_backward();
    assert_eq!(marked(&document), "a|b");
}

#[test]
fn tab_inserts_spaces_to_the_next_stop() {
    let mut document = document("ab|c");
    document.indent(crate::Indent::default());
    assert_eq!(marked(&document), "ab  |c");
}

#[test]
fn tab_and_shift_tab_on_several_lines() {
    let mut document = document("a\nb\nc");
    document.set_selections(&Selections::single(Selection::new(0, 4)));
    document.indent(crate::Indent::default());
    assert_eq!(
        document.buffer().to_text(),
        "    a\n    b\nc",
        "c is not touched"
    );
    document.outdent(crate::Indent::default());
    assert_eq!(document.buffer().to_text(), "a\nb\nc");
    let mut tabs = document_from("\t\tx|");
    tabs.outdent(crate::Indent::default());
    assert_eq!(marked(&tabs), "\tx|");
}

fn document_from(marked_text: &str) -> Document {
    document(marked_text)
}

#[test]
fn copy_and_cut_whole_lines_without_a_selection() {
    let mut document = document("one\ntw|o\nthree");
    let copied = document.copy_text();
    assert_eq!(
        copied,
        crate::CopiedText {
            text: "two\n".to_owned(),
            whole_lines: true
        }
    );
    document.cut();
    assert_eq!(marked(&document), "one\n|three");
    document.paste_lines(&copied.text);
    assert_eq!(
        marked(&document),
        "one\ntwo\n|three",
        "the cursor moves down with its line"
    );
}

#[test]
fn replace_and_primary_selection() {
    let mut document = document("hello|");
    let range = document.replace(0..5, "привет");
    assert_eq!(range, 0.."привет".len());
    assert_eq!(marked(&document), "привет|");
    document.set_primary_selection(Selection::new(0, 4));
    assert_eq!(document.selected_text(), "пр");
    assert_eq!(document.buffer().offset_to_utf16(4), 2);
    assert_eq!(document.buffer().utf16_to_offset(2), 4);
}

#[test]
fn tracked_changes_list_edits_in_the_order_applied() {
    let mut document = document("a|b\nc|d");
    document.insert("x");
    assert!(
        document.take_changes().is_empty(),
        "nothing is logged until tracking starts"
    );

    document.track_changes();
    document.insert("y");
    let changes = document.take_changes();
    // The later cursor is edited first, so earlier offsets stay valid.
    let starts: Vec<usize> = changes.edits.iter().map(|edit| edit.start).collect();
    assert_eq!(starts, vec![6, 2]);
    assert_eq!(changes.edits[0].start_point, crate::Point::new(1, 2));
    let inserted: Vec<&str> = changes.edits.iter().map(|edit| &*edit.inserted).collect();
    assert_eq!(inserted, vec!["y", "y"], "every edit carries its text");

    // Replaying lengths through every edit, undo and redo included, ends at
    // the length of the text.
    let mut length = document.buffer().len();
    document.undo();
    document.redo();
    document.delete_backward();
    for edit in document.take_changes().edits {
        assert!(edit.start <= edit.old_end && edit.old_end <= length);
        length = length - (edit.old_end - edit.start) + (edit.new_end - edit.start);
    }
    assert_eq!(length, document.buffer().len());

    document.reload(crate::Buffer::from_text("new"));
    let changes = document.take_changes();
    assert!(changes.replaced && changes.edits.is_empty());
}

#[test]
fn undo_back_to_the_saved_text_restores_its_state_id() {
    let mut document = document("|");
    let saved = document.mark_saved();
    document.insert("a");
    assert_ne!(document.state_id(), saved);
    document.undo();
    assert_eq!(
        document.state_id(),
        saved,
        "undo brought the saved text back"
    );
    document.redo();
    let typed = document.mark_saved();
    // Typing right after a save does not join the saved step.
    document.insert("b");
    assert_ne!(document.state_id(), typed);
    document.undo();
    assert_eq!(document.state_id(), typed);

    document.reload(crate::Buffer::from_text("new"));
    assert_ne!(document.state_id(), saved);
    assert_ne!(document.state_id(), typed);
}

#[test]
fn replacing_the_whole_text_can_be_undone() {
    let mut document = document("mine|");
    document.replace_all(&crate::Buffer::from_text("\u{feff}disk\r\n"));
    assert_eq!(document.buffer().to_text(), "\u{feff}disk\r\n");
    assert_eq!(
        document.selections().primary().head,
        4,
        "the cursor stays where it fits"
    );
    document.undo();
    assert_eq!(document.buffer().rope().to_string(), "mine");
}
