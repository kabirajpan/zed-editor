use std::sync::Arc;

/// A chunk of text with cached newline positions for fast lookups
#[derive(Clone, Debug)]
pub struct Chunk {
    text: Arc<String>,
    /// 🚀 CACHED newline positions for O(1) line lookups
    newline_positions: Arc<Vec<usize>>,
}

impl Chunk {
    /// Create new chunk from string
    pub fn new(text: String) -> Self {
        // 🚀 Build newline cache immediately
        let newline_positions: Vec<usize> = text
            .bytes()
            .enumerate()
            .filter(|(_, b)| *b == b'\n')
            .map(|(i, _)| i)
            .collect();

        Self {
            text: Arc::new(text),
            newline_positions: Arc::new(newline_positions),
        }
    }

    /// Get the text as a string slice
    pub fn as_str(&self) -> &str {
        &self.text
    }

    /// Length in bytes
    pub fn len(&self) -> usize {
        self.text.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// 🚀 OPTIMIZED: Count newlines using cached data (O(1) instead of O(n))
    pub fn count_lines(&self) -> usize {
        self.newline_positions.len()
    }

    /// 🚀 NEW: Get newline position by index (for fast line_to_byte)
    pub fn get_newline_position(&self, line_idx: usize) -> Option<usize> {
        self.newline_positions.get(line_idx).copied()
    }

    /// 🚀 NEW: Get all newline positions
    pub fn newline_positions(&self) -> &[usize] {
        &self.newline_positions
    }

    /// Split chunk at position
    pub fn split_at(&self, pos: usize) -> (Chunk, Chunk) {
        let (left, right) = self.text.split_at(pos);
        (Chunk::from(left), Chunk::from(right))
    }

    /// Get substring as new chunk
    pub fn slice(&self, start: usize, end: usize) -> Chunk {
        Chunk::from(&self.text[start..end])
    }
}

impl From<String> for Chunk {
    fn from(text: String) -> Self {
        Self::new(text)
    }
}

impl From<&str> for Chunk {
    fn from(text: &str) -> Self {
        Self::new(text.to_string())
    }
}
