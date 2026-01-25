#!/bin/bash
cat >> tests/newline_tests.rs << 'RUST'
use zed_text_editor::Rope;

#[test]
fn test_insert_newline() {
    let mut rope = Rope::from_text("Hello World");
    rope.insert(5, "\n");
    assert_eq!(rope.to_string(), "Hello\n World");
    assert_eq!(rope.line_count(), 1); // One newline = 1 line break
}

#[test]
fn test_delete_newline() {
    let mut rope = Rope::from_text("Hello\nWorld");
    rope.delete(5, 6); // Delete the newline
    assert_eq!(rope.to_string(), "HelloWorld");
    assert_eq!(rope.line_count(), 0); // No newlines
}

#[test]
fn test_insert_multiple_newlines() {
    let mut rope = Rope::from_text("A");
    rope.insert(1, "\n\n\n");
    rope.insert(4, "B");
    assert_eq!(rope.to_string(), "A\n\n\nB");
    assert_eq!(rope.line_count(), 3);
}

#[test]
fn test_press_enter_simulation() {
    // Simulating user pressing Enter at cursor
    let mut rope = Rope::from_text("Hello World");
    let cursor_pos = 5; // After "Hello"
    
    rope.insert(cursor_pos, "\n");
    
    assert_eq!(rope.to_string(), "Hello\n World");
    assert_eq!(rope.len(), 12); // Original 11 + 1 newline
    assert_eq!(rope.line_count(), 1);
}

#[test]
fn test_type_paragraph() {
    let mut rope = Rope::new();
    
    // Simulate typing a paragraph with Enter key
    rope.insert(0, "First line");
    rope.insert(rope.len(), "\n");
    rope.insert(rope.len(), "Second line");
    rope.insert(rope.len(), "\n");
    rope.insert(rope.len(), "Third line");
    
    let expected = "First line\nSecond line\nThird line";
    assert_eq!(rope.to_string(), expected);
    assert_eq!(rope.line_count(), 2); // Two newline characters
}

#[test]
fn test_backspace_newline() {
    // Simulating backspace that deletes a newline
    let mut rope = Rope::from_text("Hello\nWorld");
    
    // Cursor at position 6 (after newline), backspace deletes newline
    rope.delete(5, 6);
    
    assert_eq!(rope.to_string(), "HelloWorld");
    assert_eq!(rope.line_count(), 0);
}

#[test]
fn test_newline_in_chunked_text() {
    // Text with newlines that spans multiple chunks
    let text = "Line 1\n".repeat(50); // 350 bytes, will be chunked
    let rope = Rope::from_text(&text);
    
    assert_eq!(rope.line_count(), 50);
    assert_eq!(rope.to_string(), text);
}
RUST
