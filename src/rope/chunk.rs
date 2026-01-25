use std::sync::Arc;

/// A chunk of text (like Zed's 128-byte chunks)
#[derive(Clone, Debug)]
pub struct Chunk {
    text: Arc<String>,
}

impl Chunk {
    /// Create new chunk from string
    pub fn new(text: String) -> Self {
        Self {
            text: Arc::new(text),
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

    /// Count newlines in this chunk
    pub fn count_lines(&self) -> usize {
        self.text.bytes().filter(|&b| b == b'\n').count()
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
