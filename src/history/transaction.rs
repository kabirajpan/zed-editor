use crate::buffer::{Point, Offset};

/// A single atomic edit in the document
#[derive(Debug, Clone)]
pub struct RawEdit {
    pub offset: Offset,
    pub old_text: String,
    pub new_text: String,
    pub cursor_offset: Option<usize>, // Absolute byte offset after all edits
}

/// A transaction represents a group of edits across multiple cursors
#[derive(Debug, Clone)]
pub struct Transaction {
    pub edits: Vec<RawEdit>,
    pub cursor_offsets_before: Vec<usize>,
    pub cursor_offsets_after: Vec<usize>,
}

impl Transaction {
    pub fn new(
        edits: Vec<RawEdit>,
        cursor_offsets_before: Vec<usize>,
        cursor_offsets_after: Vec<usize>,
    ) -> Self {
        Self {
            edits,
            cursor_offsets_before,
            cursor_offsets_after,
        }
    }
}
