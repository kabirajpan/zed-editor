use zed_text_editor::{Editor, Point};

#[test]
fn test_empty_editor() {
    let editor = Editor::new();
    assert_eq!(editor.text(), "");
    assert_eq!(editor.cursor(), Point::zero());
}

#[test]
fn test_insert_at_cursor() {
    let mut editor = Editor::new();

    editor.insert("Hello");
    assert_eq!(editor.text(), "Hello");
    assert_eq!(editor.cursor(), Point::new(0, 5));

    editor.insert(" World");
    assert_eq!(editor.text(), "Hello World");
    assert_eq!(editor.cursor(), Point::new(0, 11));
}

#[test]
fn test_insert_newline() {
    let mut editor = Editor::new();

    editor.insert("Line 1");
    editor.insert("\n");
    editor.insert("Line 2");

    assert_eq!(editor.text(), "Line 1\nLine 2");
    assert_eq!(editor.cursor(), Point::new(1, 6));
}

#[test]
fn test_backspace() {
    let mut editor = Editor::from_text("Hello World");
    editor.set_cursor(Point::new(0, 5)); // After "Hello"

    editor.backspace();
    assert_eq!(editor.text(), "Hell World");
    assert_eq!(editor.cursor(), Point::new(0, 4));
}

#[test]
fn test_delete() {
    let mut editor = Editor::from_text("Hello World");
    editor.set_cursor(Point::new(0, 5)); // At space

    editor.delete();
    assert_eq!(editor.text(), "HelloWorld");
    assert_eq!(editor.cursor(), Point::new(0, 5)); // Cursor stays
}

#[test]
fn test_move_left_right() {
    let mut editor = Editor::from_text("Hello");
    editor.set_cursor(Point::new(0, 5));

    editor.move_left();
    assert_eq!(editor.cursor(), Point::new(0, 4));

    editor.move_right();
    assert_eq!(editor.cursor(), Point::new(0, 5));
}

#[test]
fn test_move_up_down() {
    let mut editor = Editor::from_text("Line 1\nLine 2\nLine 3");
    editor.set_cursor(Point::new(1, 3)); // Line 2, column 3

    editor.move_up();
    assert_eq!(editor.cursor(), Point::new(0, 3));

    editor.move_down();
    assert_eq!(editor.cursor(), Point::new(1, 3));

    editor.move_down();
    assert_eq!(editor.cursor(), Point::new(2, 3));
}

#[test]
fn test_move_line_start_end() {
    let mut editor = Editor::from_text("Hello World");
    editor.set_cursor(Point::new(0, 5));

    editor.move_to_line_start();
    assert_eq!(editor.cursor(), Point::new(0, 0));

    editor.move_to_line_end();
    assert_eq!(editor.cursor(), Point::new(0, 11));
}

#[test]
fn test_typing_simulation() {
    let mut editor = Editor::new();

    // Type "Hello"
    editor.insert("H");
    editor.insert("e");
    editor.insert("l");
    editor.insert("l");
    editor.insert("o");

    assert_eq!(editor.text(), "Hello");
    assert_eq!(editor.cursor(), Point::new(0, 5));

    // Press Enter
    editor.insert("\n");

    // Type "World"
    editor.insert("World");

    assert_eq!(editor.text(), "Hello\nWorld");
    assert_eq!(editor.cursor(), Point::new(1, 5));
}

#[test]
fn test_backspace_newline() {
    let mut editor = Editor::from_text("Hello\nWorld");
    editor.set_cursor(Point::new(1, 0)); // Start of "World"

    editor.backspace(); // Delete newline

    assert_eq!(editor.text(), "HelloWorld");
    assert_eq!(editor.cursor(), Point::new(0, 5));
}

#[test]
fn test_undo_insert() {
    let mut editor = Editor::new();

    editor.insert("Hello");
    assert_eq!(editor.text(), "Hello");

    editor.undo();
    assert_eq!(editor.text(), "");
    assert_eq!(editor.cursor(), Point::zero());
}

#[test]
fn test_redo_insert() {
    let mut editor = Editor::new();

    editor.insert("Hello");
    editor.undo();
    editor.redo();

    assert_eq!(editor.text(), "Hello");
    assert_eq!(editor.cursor(), Point::new(0, 5));
}

#[test]
fn test_multiple_undo_redo() {
    let mut editor = Editor::new();

    editor.insert("A");
    editor.insert("B");
    editor.insert("C");

    assert_eq!(editor.text(), "ABC");

    editor.undo();
    assert_eq!(editor.text(), "AB");

    editor.undo();
    assert_eq!(editor.text(), "A");

    editor.redo();
    assert_eq!(editor.text(), "AB");

    editor.redo();
    assert_eq!(editor.text(), "ABC");
}

#[test]
fn test_undo_backspace() {
    let mut editor = Editor::from_text("Hello");
    editor.set_cursor(Point::new(0, 5));

    editor.backspace();
    assert_eq!(editor.text(), "Hell");

    editor.undo();
    assert_eq!(editor.text(), "Hello");
}
