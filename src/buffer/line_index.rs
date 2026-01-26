/// Lazy line index - doesn't scan entire file upfront
/// Similar to how Zed handles line indexing
#[derive(Clone, Debug)]
pub struct LineIndex {
    /// Byte offsets for each line start
    /// Only populated for lines we've accessed
    offsets: Vec<usize>,
    /// Total file size in bytes
    file_size: usize,
    /// Whether we've scanned the entire file
    fully_indexed: bool,
}

impl LineIndex {
    /// Create new line index
    pub fn new(file_size: usize) -> Self {
        Self {
            offsets: vec![0], // Line 0 always starts at 0
            file_size,
            fully_indexed: false,
        }
    }

    /// Get number of lines we know about so far
    pub fn known_line_count(&self) -> usize {
        self.offsets.len()
    }

    /// Check if we've indexed the entire file
    pub fn is_fully_indexed(&self) -> bool {
        self.fully_indexed
    }

    /// Add a line offset (when we discover a newline)
    pub fn add_line(&mut self, byte_offset: usize) {
        self.offsets.push(byte_offset);
    }

    /// Mark indexing as complete
    pub fn mark_complete(&mut self) {
        self.fully_indexed = true;
    }

    /// Get byte offset for a specific line
    /// Returns None if line hasn't been indexed yet
    pub fn line_offset(&self, line_idx: usize) -> Option<usize> {
        self.offsets.get(line_idx).copied()
    }

    /// Get range (start, end) for a specific line
    /// Returns None if line hasn't been indexed yet
    pub fn line_range(&self, line_idx: usize) -> Option<(usize, usize)> {
        let start = self.offsets.get(line_idx).copied()?;
        let end = self
            .offsets
            .get(line_idx + 1)
            .copied()
            .unwrap_or(self.file_size);
        Some((start, end))
    }

    /// Scan text and build index for a range
    /// This is called lazily as we scroll through the file
    pub fn index_range(&mut self, text: &str, start_offset: usize) {
        let mut current_offset = start_offset;

        for ch in text.chars() {
            if ch == '\n' {
                self.add_line(current_offset + ch.len_utf8());
            }
            current_offset += ch.len_utf8();
        }

        // If we reached the end of the file, mark complete
        if current_offset >= self.file_size {
            self.mark_complete();
        }
    }
}

/// Progressive line indexer - scans file in chunks
pub struct ProgressiveIndexer {
    index: LineIndex,
    bytes_indexed: usize,
}

impl ProgressiveIndexer {
    pub fn new(file_size: usize) -> Self {
        Self {
            index: LineIndex::new(file_size),
            bytes_indexed: 0,
        }
    }

    /// Index the next chunk of text
    /// Returns true if indexing is complete
    pub fn index_chunk(&mut self, chunk: &str) -> bool {
        let chunk_start = self.bytes_indexed;
        self.index.index_range(chunk, chunk_start);
        self.bytes_indexed += chunk.len();

        self.index.is_fully_indexed()
    }

    /// Get the current line index
    pub fn index(&self) -> &LineIndex {
        &self.index
    }

    /// Get progress (0.0 to 1.0)
    pub fn progress(&self) -> f32 {
        if self.index.file_size == 0 {
            1.0
        } else {
            (self.bytes_indexed as f32 / self.index.file_size as f32).min(1.0)
        }
    }

    /// Check if complete
    pub fn is_complete(&self) -> bool {
        self.index.is_fully_indexed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_line_index_basic() {
        let mut index = LineIndex::new(20);

        // Add some lines
        index.add_line(5);
        index.add_line(10);
        index.add_line(15);

        assert_eq!(index.line_offset(0), Some(0));
        assert_eq!(index.line_offset(1), Some(5));
        assert_eq!(index.line_offset(2), Some(10));
        assert_eq!(index.line_offset(3), Some(15));
    }

    #[test]
    fn test_line_index_range() {
        let mut index = LineIndex::new(20);
        index.add_line(5);
        index.add_line(10);

        assert_eq!(index.line_range(0), Some((0, 5)));
        assert_eq!(index.line_range(1), Some((5, 10)));
        assert_eq!(index.line_range(2), Some((10, 20)));
    }

    #[test]
    fn test_progressive_indexer() {
        let text = "line1\nline2\nline3\n";
        let mut indexer = ProgressiveIndexer::new(text.len());

        // Index in chunks
        assert!(!indexer.index_chunk("line1\n"));
        assert_eq!(indexer.progress(), 6.0 / 18.0);

        assert!(!indexer.index_chunk("line2\n"));
        assert!(!indexer.index_chunk("line3\n"));
        assert!(indexer.is_complete());
    }
}
