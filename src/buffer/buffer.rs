use super::line_cache::{LineOffsetCache, PredictiveCache, ReusableBuffer};
use super::offset::Offset;
use super::point::Point;
use crate::rope::Rope;
use std::sync::Arc;

/// Buffer with advanced line offset caching for performance
/// Uses Arc for cheap cloning (copy-on-write)
#[derive(Clone)]
pub struct Buffer {
    rope: Arc<Rope>,
    line_cache: LineOffsetCache,
    reusable_buffer: ReusableBuffer,
    predictive_cache: PredictiveCache,
}

impl Buffer {
    pub fn new() -> Self {
        Self {
            rope: Arc::new(Rope::new()),
            line_cache: LineOffsetCache::new(0),
            reusable_buffer: ReusableBuffer::new(),
            predictive_cache: PredictiveCache::new(),
        }
    }

    pub fn from_text(text: &str) -> Self {
        let rope = Rope::from_text(text);
        let line_count = rope.line_count();

        Self {
            rope: Arc::new(rope),
            line_cache: LineOffsetCache::new(line_count),
            reusable_buffer: ReusableBuffer::new(),
            predictive_cache: PredictiveCache::new(),
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
            let affected_lines: Vec<usize> = (line..self.line_count()).collect();
            self.line_cache.invalidate_lines(&affected_lines);
        } else {
            // Single line - just invalidate this line
            self.line_cache.invalidate_lines(&[line]);
        }

        // Copy-on-write: only clone rope if there are other Arc references
        let rope = Arc::make_mut(&mut self.rope);
        rope.insert(pos, text);

        // Update line count in cache
        let new_line_count = rope.line_count();
        self.line_cache.update_line_count(new_line_count);
    }

    pub fn delete(&mut self, start: Offset, end: Offset) {
        let start_pos = start.value();
        let end_pos = end.value();

        // Invalidate cache for affected lines
        let (start_line, _) = self.rope.byte_to_line_col(start_pos);
        let (end_line, _) = self.rope.byte_to_line_col(end_pos);

        if start_line != end_line {
            // Multi-line delete - invalidate from start line onwards
            let affected_lines: Vec<usize> = (start_line..self.line_count()).collect();
            self.line_cache.invalidate_lines(&affected_lines);
        } else {
            // Single line - just invalidate this line
            self.line_cache.invalidate_lines(&[start_line]);
        }

        // Copy-on-write: only clone rope if there are other Arc references
        let rope = Arc::make_mut(&mut self.rope);
        rope.delete(start_pos, end_pos);

        // Update line count in cache
        let new_line_count = rope.line_count();
        self.line_cache.update_line_count(new_line_count);
    }

    /// Get line by index - uses rope directly (already optimized)
    pub fn line(&self, line_idx: usize) -> Option<String> {
        self.rope.line(line_idx)
    }

    /// 🚀 NEW: Batch get line offsets (FAST!)
    /// Use this when you need multiple line offsets at once
    pub fn get_line_offsets_batch(&mut self, lines: &[usize]) -> Vec<usize> {
        let offsets =
            self.line_cache
                .get_offsets_zero_alloc(lines, &self.rope, &mut self.reusable_buffer);
        offsets.to_vec()
    }

    /// 🚀 NEW: Ensure a range of lines is cached (for predictive scrolling)
    pub fn ensure_range_cached(&mut self, range: std::ops::Range<usize>) {
        self.line_cache.ensure_range_cached(&self.rope, range);
    }

    /// 🚀 NEW: Update scroll prediction for smart pre-caching
    pub fn update_scroll_prediction(
        &mut self,
        visible_range: std::ops::Range<usize>,
        scroll_delta: f32,
        frame_time: f32,
    ) {
        self.predictive_cache
            .update_scroll_prediction(visible_range, scroll_delta, frame_time);

        let total_lines = self.line_count();
        self.line_cache
            .predictive_ensure_cached(&self.rope, &self.predictive_cache, total_lines);

        // Memory optimization: Clean up cache if too large
        if self.line_cache.cache_stats().cached_lines > 10_000 {
            self.line_cache.smart_eviction(5_000);
        }
    }

    /// Point to offset conversion
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

    /// Offset to point conversion
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

    /// 🚀 Get byte range for a line (efficient - for syntax highlighting)
    pub fn line_byte_range(&self, line_idx: usize) -> Option<(usize, usize)> {
        self.rope.line_byte_range(line_idx)
    }

    /// 🚀 Extract text from byte range (efficient - no full rope conversion)
    pub fn slice_bytes(&self, start: usize, end: usize) -> String {
        self.rope.slice_bytes(start, end)
    }

    /// 🚀 Get the underlying rope reference (for advanced operations)
    pub fn rope(&self) -> &Rope {
        &self.rope
    }

    /// 🚀 Get cache statistics for performance monitoring
    pub fn cache_stats(&self) -> String {
        let stats = self.line_cache.cache_stats();
        format!(
            "Cache: {} hits, {} misses, {:.1}% hit rate, {} lines cached",
            stats.hits,
            stats.misses,
            stats.hit_rate * 100.0,
            stats.cached_lines
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
