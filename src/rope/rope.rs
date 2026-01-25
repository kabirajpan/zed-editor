use super::chunk::Chunk;
use crate::tree::SumTree;

/// Rope - text storage using SumTree
#[derive(Clone)]
pub struct Rope {
    tree: SumTree<Chunk>,
}

impl Rope {
    /// Create empty rope
    pub fn new() -> Self {
        Self {
            tree: SumTree::new(),
        }
    }

    /// Create rope from text
    pub fn from_text(text: &str) -> Self {
        let mut rope = Self::new();
        if !text.is_empty() {
            // Split into chunks of ~128 bytes (like Zed)
            const CHUNK_SIZE: usize = 128;

            for chunk_text in text.as_bytes().chunks(CHUNK_SIZE) {
                let chunk_str = std::str::from_utf8(chunk_text).expect("Invalid UTF-8 in text");
                rope.tree.push(Chunk::from(chunk_str));
            }
        }
        rope
    }

    /// Get length in bytes
    pub fn len(&self) -> usize {
        self.tree.summary().len
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.tree.is_empty()
    }

    /// Get number of lines
    pub fn line_count(&self) -> usize {
        self.tree.summary().lines
    }

    /// Convert to string - now with proper tree traversal!
    pub fn to_string(&self) -> String {
        let mut result = String::with_capacity(self.len());

        for chunk in self.tree.iter() {
            result.push_str(chunk.as_str());
        }

        result
    }

    /// Append text to end
    pub fn push_str(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }

        // Split into chunks
        const CHUNK_SIZE: usize = 128;

        for chunk_text in text.as_bytes().chunks(CHUNK_SIZE) {
            let chunk_str = std::str::from_utf8(chunk_text).expect("Invalid UTF-8 in text");
            self.tree.push(Chunk::from(chunk_str));
        }
    }

    /// Insert text at position (SIMPLE VERSION - rebuild tree)
    /// TODO: Later we'll make this O(log n) with proper tree manipulation
    pub fn insert(&mut self, pos: usize, text: &str) {
        assert!(pos <= self.len(), "Insert position out of bounds");

        if text.is_empty() {
            return;
        }

        // Simple approach: convert to string, insert, rebuild
        // This is O(n) but works for now
        let mut content = self.to_string();
        content.insert_str(pos, text);
        *self = Self::from_text(&content);
    }

    /// Delete range [start, end) (SIMPLE VERSION - rebuild tree)
    /// TODO: Later we'll make this O(log n) with proper tree manipulation
    pub fn delete(&mut self, start: usize, end: usize) {
        assert!(start <= end, "Start must be <= end");
        assert!(end <= self.len(), "Delete range out of bounds");

        if start == end {
            return;
        }

        // Simple approach: convert to string, delete, rebuild
        // This is O(n) but works for now
        let mut content = self.to_string();
        content.drain(start..end);
        *self = Self::from_text(&content);
    }
}

impl Default for Rope {
    fn default() -> Self {
        Self::new()
    }
}

// Make Rope printable
impl std::fmt::Display for Rope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_string())
    }
}
