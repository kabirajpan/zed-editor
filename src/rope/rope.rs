use super::chunk::Chunk;
use crate::tree::SumTree;

/// Rope - optimized text storage using SumTree
#[derive(Clone)]
pub struct Rope {
    tree: SumTree<Chunk>,
}

impl Rope {
    const CHUNK_SIZE: usize = 1024;

    pub fn new() -> Self {
        Self {
            tree: SumTree::new(),
        }
    }

    /// 🚀 FIXED: Build tree efficiently from text (NO STACK OVERFLOW!)
    pub fn from_text(text: &str) -> Self {
        if text.is_empty() {
            return Self::new();
        }

        // Chunk the text
        let mut chunks = Vec::new();
        let mut start = 0;

        while start < text.len() {
            let mut end = (start + Self::CHUNK_SIZE).min(text.len());

            // Align to character boundary
            while end < text.len() && !text.is_char_boundary(end) {
                end += 1;
            }

            chunks.push(Chunk::from(&text[start..end]));
            start = end;
        }

        // 🚀 Build tree in one go (balanced, iterative)
        Self {
            tree: SumTree::from_items(chunks),
        }
    }

    pub fn len(&self) -> usize {
        self.tree.summary().len
    }

    pub fn is_empty(&self) -> bool {
        self.tree.is_empty()
    }

    pub fn line_count(&self) -> usize {
        self.tree.summary().lines
    }

    /// Get a specific line by index
    pub fn line(&self, line_idx: usize) -> Option<String> {
        let mut current_line = 0;
        let mut line_content = String::new();
        let mut found_line = false;

        for chunk in self.tree.iter() {
            let chunk_text = chunk.as_str();

            for ch in chunk_text.chars() {
                if current_line == line_idx {
                    found_line = true;
                    if ch == '\n' {
                        return Some(line_content);
                    }
                    line_content.push(ch);
                } else if ch == '\n' {
                    current_line += 1;
                    if current_line > line_idx {
                        break;
                    }
                }
            }

            if current_line > line_idx {
                break;
            }
        }

        if found_line {
            Some(line_content)
        } else {
            None
        }
    }

    /// 🚀 SUPER OPTIMIZED: Get byte offset for a line using chunk newline cache
    pub fn line_to_byte(&self, target_line: usize) -> usize {
        if target_line == 0 {
            return 0;
        }

        let mut current_line = 0;
        let mut byte_offset = 0;

        for chunk in self.tree.iter() {
            let newlines_in_chunk = chunk.count_lines();

            if current_line + newlines_in_chunk >= target_line {
                // Target line is in this chunk
                let line_in_chunk = target_line - current_line;

                if line_in_chunk == 0 {
                    return byte_offset;
                }

                // 🚀 Use cached newline position (O(1) lookup!)
                if let Some(newline_pos) = chunk.get_newline_position(line_in_chunk - 1) {
                    return byte_offset + newline_pos + 1;
                }

                return byte_offset + chunk.len();
            }

            current_line += newlines_in_chunk;
            byte_offset += chunk.len();
        }

        byte_offset
    }

    /// 🚀 FIXED: Get line and column from byte offset
    /// Returns (line, column) where column is CHARACTER count, not bytes
    pub fn byte_to_line_col(&self, target_byte: usize) -> (usize, usize) {
        if target_byte == 0 {
            return (0, 0);
        }

        let mut byte_offset = 0;
        let mut line = 0;
        let mut column = 0;

        for chunk in self.tree.iter() {
            let chunk_len = chunk.len();

            if byte_offset + chunk_len > target_byte {
                // Found the chunk containing target_byte
                let offset_in_chunk = target_byte - byte_offset;
                let chunk_text = chunk.as_str();

                // Count characters up to this byte offset
                let mut bytes_counted = 0;

                for ch in chunk_text.chars() {
                    if bytes_counted >= offset_in_chunk {
                        break;
                    }
                    bytes_counted += ch.len_utf8();

                    if ch == '\n' {
                        line += 1;
                        column = 0;
                    } else {
                        column += 1;
                    }
                }

                return (line, column);
            }

            // Count newlines in this chunk
            line += chunk.count_lines();

            // Set column to 0 if chunk ends with newline, otherwise count chars after last newline
            let chunk_text = chunk.as_str();
            if let Some(last_newline_pos) = chunk.newline_positions().last() {
                // Count characters after last newline
                column = chunk_text[last_newline_pos + 1..].chars().count();
            } else {
                // No newline in chunk - add all characters
                column += chunk_text.chars().count();
            }

            byte_offset += chunk_len;
        }

        (line, column)
    }

    /// 🚀 NEW: Extract a substring by byte range (EFFICIENT - no full conversion!)
    /// This is critical for syntax highlighting performance
    pub fn slice_bytes(&self, start: usize, end: usize) -> String {
        if start >= end || start >= self.len() {
            return String::new();
        }

        let end = end.min(self.len());
        let mut result = String::new();
        let mut current_pos = 0;

        for chunk in self.tree.iter() {
            let chunk_len = chunk.len();
            let chunk_end = current_pos + chunk_len;

            // Skip chunks before our range
            if chunk_end <= start {
                current_pos = chunk_end;
                continue;
            }

            // Stop if we're past our range
            if current_pos >= end {
                break;
            }

            let chunk_text = chunk.as_str();

            // Determine what part of this chunk to include
            let slice_start = if current_pos < start {
                start - current_pos
            } else {
                0
            };

            let slice_end = if chunk_end > end {
                end - current_pos
            } else {
                chunk_len
            };

            result.push_str(&chunk_text[slice_start..slice_end]);
            current_pos = chunk_end;
        }

        result
    }

    /// 🚀 NEW: Get byte range for a specific line (returns start, end)
    /// Used by syntax highlighter for efficient line extraction
    pub fn line_byte_range(&self, line_idx: usize) -> Option<(usize, usize)> {
        let start = self.line_to_byte(line_idx);

        // Find end of line
        let mut current_line = 0;
        let mut byte_offset = 0;
        let mut found_start = false;

        for chunk in self.tree.iter() {
            let chunk_text = chunk.as_str();

            for ch in chunk_text.chars() {
                if current_line == line_idx {
                    found_start = true;
                }

                if found_start && ch == '\n' {
                    return Some((start, byte_offset));
                }

                byte_offset += ch.len_utf8();

                if ch == '\n' {
                    current_line += 1;
                    if current_line > line_idx {
                        break;
                    }
                }
            }

            if current_line > line_idx {
                break;
            }
        }

        // Line doesn't end with newline (last line)
        if found_start {
            Some((start, byte_offset))
        } else {
            None
        }
    }

    /// Convert to string (avoid on large files!)
    pub fn to_string(&self) -> String {
        let mut result = String::with_capacity(self.len());
        for chunk in self.tree.iter() {
            result.push_str(chunk.as_str());
        }
        result
    }

    /// 🚀 OPTIMIZED INSERT
    pub fn insert(&mut self, pos: usize, text: &str) {
        if text.is_empty() {
            return;
        }

        // For small files (<1MB), simple rebuild is acceptable
        if self.len() < 1_000_000 {
            let mut content = self.to_string();
            content.insert_str(pos, text);
            *self = Self::from_text(&content);
            return;
        }

        // For large files: Use optimized chunked insertion
        self.insert_optimized(pos, text);
    }

    /// 🚀 Optimized insertion for large files
    fn insert_optimized(&mut self, pos: usize, text: &str) {
        let chunks: Vec<Chunk> = self.tree.iter().collect();

        let mut current_pos = 0;
        let mut insert_chunk_idx = 0;
        let mut insert_offset = 0;

        for (idx, chunk) in chunks.iter().enumerate() {
            let chunk_len = chunk.as_str().len();
            let chunk_end = current_pos + chunk_len;

            if pos >= current_pos && pos < chunk_end {
                insert_chunk_idx = idx;
                insert_offset = pos - current_pos;
                break;
            }

            if pos >= chunk_end {
                insert_chunk_idx = idx + 1;
                insert_offset = 0;
            }

            current_pos = chunk_end;
        }

        let mut new_chunks = Vec::new();

        if insert_chunk_idx < chunks.len() && insert_offset > 0 {
            for (idx, chunk) in chunks.iter().enumerate() {
                if idx < insert_chunk_idx {
                    new_chunks.push(chunk.clone());
                } else if idx == insert_chunk_idx {
                    let chunk_text = chunk.as_str();
                    let before = &chunk_text[..insert_offset];
                    let after = &chunk_text[insert_offset..];

                    new_chunks.push(Chunk::from(before));

                    let mut start = 0;
                    while start < text.len() {
                        let mut end = (start + Self::CHUNK_SIZE).min(text.len());
                        while end < text.len() && !text.is_char_boundary(end) {
                            end += 1;
                        }
                        new_chunks.push(Chunk::from(&text[start..end]));
                        start = end;
                    }

                    new_chunks.push(Chunk::from(after));
                } else {
                    new_chunks.push(chunk.clone());
                }
            }
        } else {
            for (idx, chunk) in chunks.iter().enumerate() {
                if idx == insert_chunk_idx {
                    let mut start = 0;
                    while start < text.len() {
                        let mut end = (start + Self::CHUNK_SIZE).min(text.len());
                        while end < text.len() && !text.is_char_boundary(end) {
                            end += 1;
                        }
                        new_chunks.push(Chunk::from(&text[start..end]));
                        start = end;
                    }
                }
                new_chunks.push(chunk.clone());
            }
        }

        self.tree = SumTree::from_items(new_chunks);
    }

    /// 🚀 OPTIMIZED DELETE
    pub fn delete(&mut self, start: usize, end: usize) {
        if start >= end {
            return;
        }

        if self.len() < 1_000_000 {
            let mut content = self.to_string();
            content.drain(start..end);
            *self = Self::from_text(&content);
            return;
        }

        self.delete_optimized(start, end);
    }

    fn delete_optimized(&mut self, start: usize, end: usize) {
        let mut new_chunks = Vec::new();
        let mut current_pos = 0;

        for chunk in self.tree.iter() {
            let chunk_text = chunk.as_str();
            let chunk_len = chunk_text.len();
            let chunk_end = current_pos + chunk_len;

            if chunk_end <= start {
                new_chunks.push(chunk);
            } else if current_pos >= end {
                new_chunks.push(chunk);
            } else {
                let keep_start = if current_pos < start {
                    start - current_pos
                } else {
                    0
                };

                let keep_end = if chunk_end > end {
                    end - current_pos
                } else {
                    chunk_len
                };

                if keep_start > 0 {
                    new_chunks.push(Chunk::from(&chunk_text[..keep_start]));
                }

                if keep_end < chunk_len {
                    new_chunks.push(Chunk::from(&chunk_text[keep_end..]));
                }
            }

            current_pos = chunk_end;
        }

        self.tree = SumTree::from_items(new_chunks);
    }

    pub fn chunk_count(&self) -> usize {
        self.tree.iter().count()
    }

    pub fn memory_usage(&self) -> usize {
        self.len() + self.chunk_count() * 64
    }
}

impl Default for Rope {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for Rope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_string())
    }
}
