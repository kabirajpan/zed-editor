// Multi-cursor support - to be implemented later

use super::selection::Selection;

/// Multiple cursors/selections
#[allow(dead_code)] // We'll use this later
#[derive(Debug, Clone)]
pub struct MultiCursor {
    selections: Vec<Selection>,
}

impl MultiCursor {
    pub fn new() -> Self {
        Self {
            selections: Vec::new(),
        }
    }
}

impl Default for MultiCursor {
    fn default() -> Self {
        Self::new()
    }
}
