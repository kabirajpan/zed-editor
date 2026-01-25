use zed_text_editor::{Buffer, Offset, Point};

#[test]
fn test_empty_buffer() {
    let buffer = Buffer::new();
    assert!(buffer.is_empty());
    assert_eq!(buffer.len(), 0);
    assert_eq!(buffer.line_count(), 1); // Empty buffer has 1 line
}

#[test]
fn test_buffer_from_text() {
    let buffer = Buffer::from_text("Hello, World!");
    assert_eq!(buffer.len(), 13);
    assert_eq!(buffer.line_count(), 1);
}

#[test]
fn test_point_to_offset() {
    let buffer = Buffer::from_text("Hello\nWorld\n!");

    // Point (0, 0) = 'H'
    assert_eq!(buffer.point_to_offset(Point::new(0, 0)), Offset(0));

    // Point (0, 5) = '\n'
    assert_eq!(buffer.point_to_offset(Point::new(0, 5)), Offset(5));

    // Point (1, 0) = 'W'
    assert_eq!(buffer.point_to_offset(Point::new(1, 0)), Offset(6));

    // Point (1, 5) = '\n'
    assert_eq!(buffer.point_to_offset(Point::new(1, 5)), Offset(11));

    // Point (2, 0) = '!'
    assert_eq!(buffer.point_to_offset(Point::new(2, 0)), Offset(12));
}

#[test]
fn test_offset_to_point() {
    let buffer = Buffer::from_text("Hello\nWorld\n!");

    assert_eq!(buffer.offset_to_point(Offset(0)), Point::new(0, 0));
    assert_eq!(buffer.offset_to_point(Offset(5)), Point::new(0, 5));
    assert_eq!(buffer.offset_to_point(Offset(6)), Point::new(1, 0));
    assert_eq!(buffer.offset_to_point(Offset(11)), Point::new(1, 5));
    assert_eq!(buffer.offset_to_point(Offset(12)), Point::new(2, 0));
}

#[test]
fn test_roundtrip_conversion() {
    let buffer = Buffer::from_text("Line 1\nLine 2\nLine 3\n");

    // Test various points
    let points = vec![
        Point::new(0, 0),
        Point::new(0, 3),
        Point::new(1, 0),
        Point::new(2, 6),
    ];

    for point in points {
        let offset = buffer.point_to_offset(point);
        let back = buffer.offset_to_point(offset);
        assert_eq!(point, back, "Roundtrip failed for {:?}", point);
    }
}

#[test]
fn test_get_line() {
    let buffer = Buffer::from_text("Line 1\nLine 2\nLine 3");

    assert_eq!(buffer.line(0), Some("Line 1".to_string()));
    assert_eq!(buffer.line(1), Some("Line 2".to_string()));
    assert_eq!(buffer.line(2), Some("Line 3".to_string()));
    assert_eq!(buffer.line(3), None);
}

#[test]
fn test_get_all_lines() {
    let buffer = Buffer::from_text("A\nB\nC");
    let lines = buffer.lines();

    assert_eq!(lines, vec!["A", "B", "C"]);
}

#[test]
fn test_line_count() {
    let b1 = Buffer::from_text("Single line");
    assert_eq!(b1.line_count(), 1);

    let b2 = Buffer::from_text("Line 1\nLine 2");
    assert_eq!(b2.line_count(), 2);

    let b3 = Buffer::from_text("A\nB\nC\n");
    assert_eq!(b3.line_count(), 4); // 3 lines + 1 empty
}

#[test]
fn test_insert_at_point() {
    let mut buffer = Buffer::from_text("Hello World");

    // Insert newline at position 5
    let offset = buffer.point_to_offset(Point::new(0, 5));
    buffer.insert(offset, "\n");

    assert_eq!(buffer.to_string(), "Hello\n World");
    assert_eq!(buffer.line_count(), 2);
}

#[test]
fn test_delete_at_point() {
    let mut buffer = Buffer::from_text("Hello\nWorld");

    // Delete the newline
    let start = buffer.point_to_offset(Point::new(0, 5));
    let end = buffer.point_to_offset(Point::new(1, 0));
    buffer.delete(start, end);

    assert_eq!(buffer.to_string(), "HelloWorld");
}
