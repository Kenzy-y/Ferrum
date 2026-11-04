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
