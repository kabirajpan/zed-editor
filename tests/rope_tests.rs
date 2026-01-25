use zed_text_editor::Rope;

#[test]
fn test_empty_rope() {
    let rope = Rope::new();
    assert!(rope.is_empty());
    assert_eq!(rope.len(), 0);
    assert_eq!(rope.line_count(), 0);
}

#[test]
fn test_rope_from_text() {
    let rope = Rope::from_text("Hello, World!");
    assert!(!rope.is_empty());
    assert_eq!(rope.len(), 13);
    assert_eq!(rope.line_count(), 0); // No newlines
}

#[test]
fn test_rope_with_newlines() {
    let rope = Rope::from_text("Line 1\nLine 2\nLine 3\n");
    assert_eq!(rope.line_count(), 3);
}

#[test]
fn test_rope_push() {
    let mut rope = Rope::from_text("Hello");
    rope.push_str(" World");
    assert_eq!(rope.len(), 11);
}

#[test]
fn test_rope_multi_push() {
    let mut rope = Rope::new();
    rope.push_str("First\n"); // 6 bytes, 1 newline
    rope.push_str("Second\n"); // 7 bytes, 1 newline
    rope.push_str("Third\n"); // 6 bytes, 1 newline

    assert_eq!(rope.len(), 19);
    assert_eq!(rope.line_count(), 3);
}

#[test]
fn test_rope_to_string() {
    let rope = Rope::from_text("Hello, World!");
    assert_eq!(rope.to_string(), "Hello, World!");
}

#[test]
fn test_rope_to_string_multiline() {
    let text = "Line 1\nLine 2\nLine 3\n";
    let rope = Rope::from_text(text);
    assert_eq!(rope.to_string(), text);
}

#[test]
fn test_rope_large_text() {
    // Create text larger than chunk size (128 bytes)
    let text = "a".repeat(500);
    let rope = Rope::from_text(&text);

    assert_eq!(rope.len(), 500);
    assert_eq!(rope.to_string(), text);
}

#[test]
fn test_rope_chunking() {
    // Test that chunking works correctly
    let text = "This is a test that should be split into multiple chunks because it's longer than 128 bytes. Let's make sure the chunking logic works correctly and preserves the text.";
    let rope = Rope::from_text(text);

    assert_eq!(rope.to_string(), text);
}

#[test]
fn test_insert_at_start() {
    let mut rope = Rope::from_text("World");
    rope.insert(0, "Hello ");
    assert_eq!(rope.to_string(), "Hello World");
}

#[test]
fn test_insert_at_end() {
    let mut rope = Rope::from_text("Hello");
    rope.insert(5, " World");
    assert_eq!(rope.to_string(), "Hello World");
}

#[test]
fn test_insert_in_middle() {
    let mut rope = Rope::from_text("HelloWorld");
    rope.insert(5, " ");
    assert_eq!(rope.to_string(), "Hello World");
}

#[test]
fn test_delete_at_start() {
    let mut rope = Rope::from_text("Hello World");
    rope.delete(0, 6); // Remove "Hello "
    assert_eq!(rope.to_string(), "World");
}

#[test]
fn test_delete_at_end() {
    let mut rope = Rope::from_text("Hello World");
    rope.delete(5, 11); // Remove " World"
    assert_eq!(rope.to_string(), "Hello");
}

#[test]
fn test_delete_in_middle() {
    let mut rope = Rope::from_text("Hello Beautiful World");
    rope.delete(6, 16); // Remove "Beautiful "
    assert_eq!(rope.to_string(), "Hello World");
}

#[test]
fn test_insert_and_delete() {
    let mut rope = Rope::from_text("The quick fox");
    assert_eq!(rope.to_string(), "The quick fox");

    // Insert "brown " before "fox"
    rope.insert(10, "brown ");
    assert_eq!(rope.to_string(), "The quick brown fox");

    // Append " jumped"
    rope.insert(rope.len(), " jumped");
    assert_eq!(rope.to_string(), "The quick brown fox jumped");
}

#[test]
fn test_multiple_edits() {
    let mut rope = Rope::new();
    rope.insert(0, "Hello");
    rope.insert(5, " ");
    rope.insert(6, "World");
    rope.insert(11, "!");

    assert_eq!(rope.to_string(), "Hello World!");

    rope.delete(5, 6); // Remove space
    assert_eq!(rope.to_string(), "HelloWorld!");
}
