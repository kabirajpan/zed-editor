use super::line_cache_simple::LineCache;
use super::offset::Offset;
use super::point::Point;
use crate::rope::Rope;

/// Buffer with line offset caching for performance
#[derive(Clone)]
pub struct Buffer {
    rope: Rope,
    line_cache: LineCache,
}

impl Buffer {
    pub fn new() -> Self {
        Self {
            rope: Rope::new(),
            line_cache: LineCache::new(),
        }
    }

    pub fn from_text(text: &str) -> Self {
        Self {
            rope: Rope::from_text(text),
            line_cache: LineCache::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.rope.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rope.is_empty()
    }

    pub fn line_count(&self) -> usize {
        if self.rope.is_empty() {
            1
        } else {
            self.rope.line_count() + 1
        }
    }

    pub fn to_string(&self) -> String {
        self.rope.to_string()
    }

    pub fn insert(&mut self, offset: Offset, text: &str) {
        let pos = offset.value();

        // Invalidate cache for affected lines
        let (line, _col) = self.rope.byte_to_line_col(pos);

        if text.contains('\n') {
            // Multi-line insert - invalidate from this line onwards
            self.line_cache.invalidate_from(line);
        } else {
            // Single line - just invalidate this line
            self.line_cache.invalidate_line(line);
        }

        self.rope.insert(pos, text);
    }

    pub fn delete(&mut self, start: Offset, end: Offset) {
        let start_pos = start.value();
        let end_pos = end.value();

        // Invalidate cache for affected lines
        let (start_line, _) = self.rope.byte_to_line_col(start_pos);
        let (end_line, _) = self.rope.byte_to_line_col(end_pos);

        if start_line != end_line {
            // Multi-line delete - invalidate from start line onwards
            self.line_cache.invalidate_from(start_line);
        } else {
            // Single line - just invalidate this line
            self.line_cache.invalidate_line(start_line);
        }

        self.rope.delete(start_pos, end_pos);
    }

    /// Get line by index - uses rope directly (already optimized)
    pub fn line(&self, line_idx: usize) -> Option<String> {
        self.rope.line(line_idx)
    }

    /// Get line offset with caching (FAST!)
    fn get_line_offset_cached(&mut self, line: usize) -> usize {
        // Try cache first
        if let Some(offset) = self.line_cache.get(line) {
            return offset;
        }

        // Cache miss - calculate and store
        let offset = self.rope.line_to_byte(line);
        self.line_cache.insert(line, offset);
        offset
    }

    /// Point to offset conversion - Calculates on demand (no cache mutation)
    pub fn point_to_offset(&self, point: Point) -> Offset {
        let line_start = self.rope.line_to_byte(point.row);

        // Get column offset within line
        if let Some(line_text) = self.line(point.row) {
            let column_bytes: usize = line_text
                .chars()
                .take(point.column)
                .map(|c| c.len_utf8())
                .sum();

            Offset(line_start + column_bytes)
        } else {
            Offset(line_start)
        }
    }

    /// Cached version - use this when you have &mut self
    pub fn point_to_offset_cached(&mut self, point: Point) -> Offset {
        let line_start = self.get_line_offset_cached(point.row);

        if let Some(line_text) = self.line(point.row) {
            let column_bytes: usize = line_text
                .chars()
                .take(point.column)
                .map(|c| c.len_utf8())
                .sum();

            Offset(line_start + column_bytes)
        } else {
            Offset(line_start)
        }
    }

    /// Offset to point conversion - NOW USES ROPE METHOD!
    pub fn offset_to_point(&self, offset: Offset) -> Point {
        let (line, col) = self.rope.byte_to_line_col(offset.value());
        Point::new(line, col)
    }

    pub fn lines(&self) -> Vec<String> {
        let mut result = Vec::new();
        for i in 0..self.line_count() {
            if let Some(line) = self.line(i) {
                result.push(line);
            }
        }
        result
    }

    /// Print cache statistics (for debugging)
    pub fn cache_stats(&self) -> String {
        let (hits, misses, hit_rate) = self.line_cache.stats();
        format!(
            "Cache: {} hits, {} misses, {:.1}% hit rate, {} lines cached",
            hits,
            misses,
            hit_rate * 100.0,
            self.line_cache.size()
        )
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
