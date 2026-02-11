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

    // Type character by character
    editor.insert("H");
    editor.insert("e");
    editor.insert("l");
    editor.insert("l");
    editor.insert("o");
    editor.insert(" "); // Space triggers flush
    assert_eq!(editor.text(), "Hello ");

    editor.undo();
    assert_eq!(editor.text(), "", "Undo should remove the entire word batch");
    assert_eq!(editor.cursor(), Point::zero());
}

#[test]
fn test_redo_insert() {
    let mut editor = Editor::new();

    // Type character by character with space to flush
    editor.insert("H");
    editor.insert("e");
    editor.insert("l");
    editor.insert("l");
    editor.insert("o");
    editor.insert(" ");
    assert_eq!(editor.text(), "Hello ");
    let cursor_after = editor.cursor();

    editor.undo();
    assert_eq!(editor.text(), "");

    editor.redo();
    assert_eq!(editor.text(), "Hello ");
    assert_eq!(editor.cursor(), cursor_after);
}

#[test]
fn test_multiple_undo_redo() {
    let mut editor = Editor::new();

    // Type "A " (with space to create first batch)
    editor.insert("A");
    editor.insert(" ");
    assert_eq!(editor.text(), "A ");

    // Type "B " (with space to create second batch)
    editor.insert("B");
    editor.insert(" ");
    assert_eq!(editor.text(), "A B ");

    // Type "C " (with space to create third batch)
    editor.insert("C");
    editor.insert(" ");
    assert_eq!(editor.text(), "A B C ");

    // First undo removes "C "
    editor.undo();
    assert_eq!(editor.text(), "A B ");

    // Second undo removes "B "
    editor.undo();
    assert_eq!(editor.text(), "A ");

    // First redo restores "B "
    editor.redo();
    assert_eq!(editor.text(), "A B ");

    // Second redo restores "C "
    editor.redo();
    assert_eq!(editor.text(), "A B C ");
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
/// ✅ CRITICAL TEST: Single Ctrl+Z should undo immediately
/// This was the main bug - first Ctrl+Z would move cursor, second would change text
#[test]
fn test_undo_single_keypress_word_batching() {
    let mut editor = Editor::new();

    // Type "hello " (6 characters) - space should batch this as one undo unit
    editor.insert("h");
    editor.insert("e");
    editor.insert("l");
    editor.insert("l");
    editor.insert("o");
    editor.insert(" "); // Space triggers flush

    assert_eq!(editor.text(), "hello ");
    assert!(editor.can_undo());

    // ONE press of Ctrl+Z should fully undo the entire "hello " word
    // NOT just move the cursor - the text should actually be removed
    editor.undo();
    assert_eq!(editor.text(), "", "Undo should remove the entire word on first press");
    assert_eq!(editor.cursor(), Point::zero(), "Cursor should be at zero");
    assert!(!editor.can_undo(), "No more undo history");
}

/// ✅ CRITICAL TEST: Single Ctrl+Y (redo) should work correctly after undo
/// The redo stack was reversed in the original code
#[test]
fn test_redo_single_keypress_after_undo() {
    let mut editor = Editor::new();

    editor.insert("h");
    editor.insert("e");
    editor.insert("l");
    editor.insert("l");
    editor.insert("o");
    editor.insert(" ");

    assert_eq!(editor.text(), "hello ");
    let cursor_after_insert = editor.cursor();

    editor.undo();
    assert_eq!(editor.text(), "");

    // ONE press of Ctrl+Y should fully restore "hello "
    editor.redo();
    assert_eq!(editor.text(), "hello ", "Redo should restore the entire word");
    assert_eq!(editor.cursor(), cursor_after_insert, "Cursor should be at the position after the word");
}

/// ✅ Word-by-word undo test - multiple spaces create multiple undo entries
#[test]
fn test_word_by_word_undo_multiple_words() {
    let mut editor = Editor::new();

    // Type "hello " (one batched unit)
    editor.insert("h");
    editor.insert("e");
    editor.insert("l");
    editor.insert("l");
    editor.insert("o");
    editor.insert(" ");
    assert_eq!(editor.text(), "hello ");

    // Type "world " (another batched unit)
    editor.insert("w");
    editor.insert("o");
    editor.insert("r");
    editor.insert("l");
    editor.insert("d");
    editor.insert(" ");
    assert_eq!(editor.text(), "hello world ");

    // First undo removes "world "
    editor.undo();
    assert_eq!(editor.text(), "hello ", "First undo should remove 'world '");

    // Second undo removes "hello "
    editor.undo();
    assert_eq!(editor.text(), "", "Second undo should remove 'hello '");

    // Redo should restore in order
    editor.redo();
    assert_eq!(editor.text(), "hello ", "First redo should restore 'hello '");

    editor.redo();
    assert_eq!(editor.text(), "hello world ", "Second redo should restore 'world '");
}