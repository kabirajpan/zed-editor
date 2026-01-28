use crate::buffer::Point;

/// A single edit operation
#[derive(Debug, Clone)]
pub enum EditKind {
    Insert { text: String },
    Delete { text: String },
    Replace { old_text: String, new_text: String },
}

/// A transaction represents a group of edits
#[derive(Debug, Clone)]
pub struct Transaction {
    pub cursor_before: Point,
    pub cursor_after: Point,
    pub edit: EditKind,
}

impl Transaction {
    pub fn insert(text: String, cursor_before: Point, cursor_after: Point) -> Self {
        Self {
            cursor_before,
            cursor_after,
            edit: EditKind::Insert { text },
        }
    }

    pub fn delete(text: String, cursor_before: Point, cursor_after: Point) -> Self {
        Self {
            cursor_before,
            cursor_after,
            edit: EditKind::Delete { text },
        }
    }

    pub fn replace(
        old_text: String,
        new_text: String,
        cursor_before: Point,
        cursor_after: Point,
    ) -> Self {
        Self {
            cursor_before,
            cursor_after,
            edit: EditKind::Replace { old_text, new_text },
        }
    }
}
