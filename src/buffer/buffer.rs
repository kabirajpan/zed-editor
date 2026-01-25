use super::offset::Offset;
use super::point::Point;
use crate::rope::Rope;

/// Buffer wraps Rope and provides Point/Offset operations
#[derive(Clone)]
pub struct Buffer {
    rope: Rope,
}

impl Buffer {
    /// Create empty buffer
    pub fn new() -> Self {
        Self { rope: Rope::new() }
    }

    /// Create buffer from text
    pub fn from_text(text: &str) -> Self {
        Self {
            rope: Rope::from_text(text),
        }
    }

    /// Get length in bytes
    pub fn len(&self) -> usize {
        self.rope.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.rope.is_empty()
    }

    /// Get number of lines
    pub fn line_count(&self) -> usize {
        // In editors, line count is newlines + 1
        // "hello" = 1 line
        // "hello\n" = 2 lines (one empty line at end)
        if self.rope.is_empty() {
            1
        } else {
            self.rope.line_count() + 1
        }
    }

    /// Convert to string
    pub fn to_string(&self) -> String {
        self.rope.to_string()
    }

    /// Insert text at offset
    pub fn insert(&mut self, offset: Offset, text: &str) {
        self.rope.insert(offset.value(), text);
    }

    /// Delete range [start, end)
    pub fn delete(&mut self, start: Offset, end: Offset) {
        self.rope.delete(start.value(), end.value());
    }

    /// Convert Point to Offset
    pub fn point_to_offset(&self, point: Point) -> Offset {
        let text = self.to_string();
        let mut current_row = 0;
        let mut offset = 0;

        for (i, ch) in text.char_indices() {
            if current_row == point.row {
                // We're on the target row
                if offset == point.column {
                    return Offset(i);
                }
                if ch == '\n' {
                    // End of line, return this position
                    return Offset(i);
                }
                offset += 1;
            } else if ch == '\n' {
                current_row += 1;
                offset = 0;
            }
        }

        // If we get here, return end of buffer
        Offset(text.len())
    }

    /// Convert Offset to Point
    pub fn offset_to_point(&self, offset: Offset) -> Point {
        let text = self.to_string();
        let mut row = 0;
        let mut column = 0;

        for (i, ch) in text.char_indices() {
            if i >= offset.value() {
                break;
            }

            if ch == '\n' {
                row += 1;
                column = 0;
            } else {
                column += 1;
            }
        }

        Point::new(row, column)
    }

    /// Get line by index (0-based)
    pub fn line(&self, line_idx: usize) -> Option<String> {
        let text = self.to_string();
        text.lines().nth(line_idx).map(|s| s.to_string())
    }

    /// Get all lines
    pub fn lines(&self) -> Vec<String> {
        self.to_string().lines().map(|s| s.to_string()).collect()
    }
}

impl Default for Buffer {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for Buffer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_string())
    }
}
