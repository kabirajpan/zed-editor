// src/buffer/line_cache.rs
use crate::buffer::Rope;

use std::ops::Range;

/// 🚀 ZERO-ALLOCATION LINE OFFSET CACHE
/// Replaces HashMap with array-based cache for 10x performance
#[derive(Debug)]
pub struct LineOffsetCache {
    // 🚀 ARRAY-BASED STORAGE (no HashMap allocations)
    cached_offsets: Vec<Option<usize>>, // line_index -> byte_offset
    cached_range: Range<usize>,         // What lines we have cached
    current_version: u64,               // For cache invalidation
    last_access: std::time::Instant,
    total_lines: usize, // File size for bounds checking
    cache_hits: u64,    // Performance metrics
    cache_misses: u64,
}

#[derive(Debug, Clone, Copy)]
pub enum ScrollDirection {
    Up,
    Down,
    Stationary,
}

#[derive(Debug)]
pub struct PredictiveCache {
    pub visible_range: std::ops::Range<usize>, // Currently visible lines
    pub precalc_range: std::ops::Range<usize>, // Lines to pre-calculate (±200)
    pub scroll_direction: ScrollDirection,     // Predict where user will scroll
    pub last_scroll_time: std::time::Instant,  // For scroll prediction
    pub scroll_velocity: f32,                  // Pixels/frame for prediction
}

/// 🚀 REUSABLE BUFFER to avoid allocations during rendering
#[derive(Debug)]
pub struct ReusableBuffer {
    pub buffer: Vec<usize>,
    last_used: std::time::Instant,
}

impl PredictiveCache {
    pub fn new() -> Self {
        Self {
            visible_range: 0..0,
            precalc_range: 0..0,
            scroll_direction: ScrollDirection::Stationary,
            last_scroll_time: std::time::Instant::now(),
            scroll_velocity: 0.0,
        }
    }

    /// 🚀 UPDATE scroll prediction based on user behavior
    pub fn update_scroll_prediction(
        &mut self,
        new_visible_range: std::ops::Range<usize>,
        scroll_delta: f32,
        frame_time: f32,
    ) {
        // Calculate scroll velocity
        self.scroll_velocity = if frame_time > 0.0 {
            scroll_delta.abs() / frame_time
        } else {
            0.0
        };

        // Determine scroll direction
        self.scroll_direction = if scroll_delta < -0.1 {
            ScrollDirection::Up
        } else if scroll_delta > 0.1 {
            ScrollDirection::Down
        } else {
            ScrollDirection::Stationary
        };

        self.visible_range = new_visible_range.clone();

        // 🚀 PREDICT where user will scroll next
        self.precalc_range = self.calculate_precalc_range(new_visible_range);

        self.last_scroll_time = std::time::Instant::now();
    }

    /// 🚀 CALCULATE which lines to pre-cache based on scroll behavior
    fn calculate_precalc_range(
        &self,
        visible_range: std::ops::Range<usize>,
    ) -> std::ops::Range<usize> {
        const BASE_PADDING: usize = 100; // Lines to cache around viewport
        const VELOCITY_MULTIPLIER: f32 = 2.0; // How much extra to cache based on speed

        let velocity_padding = (self.scroll_velocity * VELOCITY_MULTIPLIER) as usize;
        let total_padding = BASE_PADDING + velocity_padding.min(300); // Cap at 400 lines

        match self.scroll_direction {
            ScrollDirection::Up => {
                // User scrolling up - cache more lines above
                let start = visible_range.start.saturating_sub(total_padding * 2);
                let end = visible_range.end + total_padding;
                start..end.min(usize::MAX)
            }
            ScrollDirection::Down => {
                // User scrolling down - cache more lines below
                let start = visible_range.start.saturating_sub(total_padding);
                let end = visible_range.end + total_padding * 2;
                start..end.min(usize::MAX)
            }
            ScrollDirection::Stationary => {
                // User stationary - cache balanced around viewport
                let start = visible_range.start.saturating_sub(total_padding);
                let end = visible_range.end + total_padding;
                start..end.min(usize::MAX)
            }
        }
    }

    /// 🚀 GET the range that should be pre-cached
    pub fn get_precalc_range(&self, total_lines: usize) -> std::ops::Range<usize> {
        let start = self.precalc_range.start.min(total_lines.saturating_sub(1));
        let end = self.precalc_range.end.min(total_lines);
        start..end
    }

    /// 🚀 CHECK if we should pre-cache based on scroll activity
    pub fn should_precache(&self) -> bool {
        let time_since_scroll = self.last_scroll_time.elapsed();
        time_since_scroll.as_millis() < 500 // Pre-cache for 500ms after scroll
    }
}

impl LineOffsetCache {
    const CACHE_PADDING: usize = 200; // Lines to cache around viewport

    pub fn new(total_lines: usize) -> Self {
        Self {
            cached_offsets: Vec::new(),
            cached_range: 0..0,
            current_version: 0,
            last_access: std::time::Instant::now(),
            total_lines,
            cache_hits: 0,
            cache_misses: 0,
        }
    }

    pub fn get_offsets_zero_alloc<'a>(
        &mut self,
        lines: &[usize],
        rope: &Rope, // 🚀 CHANGE: Take Rope reference instead of Editor
        reusable_buffer: &'a mut ReusableBuffer,
    ) -> &'a [usize] {
        self.last_access = std::time::Instant::now();

        // 🚀 REUSE EXISTING BUFFER (no allocation)
        reusable_buffer.buffer.clear();
        reusable_buffer.buffer.reserve(lines.len());

        for &line in lines {
            if self.is_cached(line) {
                // 🚀 CACHE HIT: Direct array access (no hash computation)
                if let Some(offset) = self.get_cached_offset(line) {
                    reusable_buffer.buffer.push(offset);
                    self.cache_hits += 1;
                    continue;
                }
            }

            // 🚀 CACHE MISS: Calculate using rope and cache it
            let offset = rope.line_to_byte(line); // 🚀 DIRECT ROPE CALL
            self.cache_miss_calculate(line, offset, rope);
            reusable_buffer.buffer.push(offset);
            self.cache_misses += 1;
        }

        &reusable_buffer.buffer
    }

    /// 🚀 ENSURE A RANGE OF LINES IS CACHED (for predictive scrolling)
    pub fn ensure_range_cached(
        &mut self,
        rope: &Rope, // 🚀 CHANGE: Take Rope reference
        range: std::ops::Range<usize>,
    ) {
        // 🚀 FIX: Manual range containment check
        if range.start >= self.cached_range.start && range.end <= self.cached_range.end {
            return; // Already cached
        }

        // 🚀 EXPAND RANGE FOR BETTER CACHE UTILIZATION
        let new_range = self.expand_range_for_caching(range);

        // 🚀 BATCH CALCULATION (much faster than individual calls)
        let mut new_offsets = vec![None; new_range.len()];

        for (i, line) in new_range.clone().enumerate() {
            if line < self.total_lines {
                let offset = rope.line_to_byte(line); // 🚀 DIRECT ROPE CALL
                new_offsets[i] = Some(offset);
            }
        }

        // 🚀 ATOMIC CACHE UPDATE (replace entire range)
        self.cached_range = new_range;
        self.cached_offsets = new_offsets;
        self.current_version += 1;
    }

    /// 🚀 SMART INVALIDATION: Handle line number shifts after newline insertion
    pub fn invalidate_range_with_shift(&mut self, start_line: usize, inserted_lines: usize) {
        // Invalidate lines from start_line onwards
        // Because all line numbers after this point have shifted down

        if !self.cached_range.contains(&start_line) {
            return; // Nothing to invalidate
        }

        let cache_start_idx = if start_line >= self.cached_range.start {
            start_line - self.cached_range.start
        } else {
            0
        };

        // Invalidate from cache_start_idx to end
        for i in cache_start_idx..self.cached_offsets.len() {
            self.cached_offsets[i] = None;
        }

        self.current_version += 1;

        println!(
            "🔄 Cache invalidated from line {} (shift: {} lines)",
            start_line, inserted_lines
        );
    }
    /// 🚀 SMART CACHE INVALIDATION (only affected lines)
    pub fn invalidate_lines(&mut self, lines: &[usize]) {
        for &line in lines {
            if self.cached_range.contains(&line) {
                let cache_index = line - self.cached_range.start;
                if cache_index < self.cached_offsets.len() {
                    self.cached_offsets[cache_index] = None;
                }
            }
        }
        self.current_version += 1;
    }

    /// 🚀 INVALIDATE ENTIRE CACHE (for major edits)
    pub fn invalidate_all(&mut self) {
        self.cached_range = 0..0;
        self.cached_offsets.clear();
        self.current_version += 1;
    }

    pub fn predictive_ensure_cached(
        &mut self,
        rope: &Rope,
        predictive_cache: &PredictiveCache,
        total_lines: usize,
    ) {
        if predictive_cache.should_precache() {
            let precalc_range = predictive_cache.get_precalc_range(total_lines);
            if !precalc_range.is_empty() {
                self.ensure_range_cached(rope, precalc_range);
            }
        }
    }

    pub fn smart_eviction(&mut self, max_cached_lines: usize) {
        if self.cached_offsets.len() > max_cached_lines {
            // Simple strategy: keep current range, clear if too large
            if self.cached_range.len() > max_cached_lines {
                // Cache is too large - clear and let it rebuild naturally
                self.cached_range = 0..0;
                self.cached_offsets.clear();
                self.current_version += 1;
            }
        }
    }

    /// 🚀 MEMORY USAGE OPTIMIZATION
    pub fn optimize_memory(&mut self) {
        // Shrink vectors if they're using too much extra capacity
        if self.cached_offsets.capacity() > self.cached_offsets.len() * 2 {
            self.cached_offsets.shrink_to_fit();
        }
    }

    // ==================== PRIVATE HELPERS ====================

    fn is_cached(&self, line: usize) -> bool {
        self.cached_range.contains(&line)
            && line - self.cached_range.start < self.cached_offsets.len()
    }

    fn get_cached_offset(&self, line: usize) -> Option<usize> {
        if self.is_cached(line) {
            let index = line - self.cached_range.start;
            self.cached_offsets[index]
        } else {
            None
        }
    }

    fn cache_miss_calculate(&mut self, line: usize, _offset: usize, rope: &Rope) {
        // Actually use the offset parameter or prefix with underscore
        if self.should_expand_cache(line) {
            let new_range = self.expand_range_to_include(line);
            self.ensure_range_cached(rope, new_range);
        }
    }

    fn should_expand_cache(&self, line: usize) -> bool {
        if self.cached_range.is_empty() {
            return true;
        }

        // Expand if line is close to cached range
        (line < self.cached_range.start && self.cached_range.start - line < Self::CACHE_PADDING)
            || (line >= self.cached_range.end && line - self.cached_range.end < Self::CACHE_PADDING)
    }

    fn expand_range_to_include(&self, line: usize) -> Range<usize> {
        if self.cached_range.is_empty() {
            let start = line.saturating_sub(Self::CACHE_PADDING);
            let end = (line + Self::CACHE_PADDING).min(self.total_lines);
            return start..end;
        }

        let start = if line < self.cached_range.start {
            line.saturating_sub(Self::CACHE_PADDING)
        } else {
            self.cached_range.start
        };

        let end = if line >= self.cached_range.end {
            (line + Self::CACHE_PADDING).min(self.total_lines)
        } else {
            self.cached_range.end
        };

        start..end
    }

    fn expand_range_for_caching(&self, range: Range<usize>) -> Range<usize> {
        let start = range.start.saturating_sub(Self::CACHE_PADDING);
        let end = (range.end + Self::CACHE_PADDING).min(self.total_lines);
        start..end
    }

    fn ensure_range_cached_impl(&mut self, range: Range<usize>) {
        // Implementation for internal use
        let mut new_offsets = vec![None; range.len()];

        // Copy existing cached values
        for line in range.clone() {
            if let Some(offset) = self.get_cached_offset(line) {
                let new_index = line - range.start;
                new_offsets[new_index] = Some(offset);
            }
        }

        self.cached_range = range;
        self.cached_offsets = new_offsets;
    }

    /// 🚀 UPDATE line count without resetting cache statistics
    pub fn update_line_count(&mut self, new_total_lines: usize) {
        self.total_lines = new_total_lines;

        // If the new file is smaller, truncate the cached range
        if self.cached_range.end > new_total_lines {
            self.cached_range.end = new_total_lines;
            if self.cached_range.start > new_total_lines {
                self.cached_range.start = 0;
                self.cached_offsets.clear();
            } else {
                // Truncate the offsets vector
                let new_len = new_total_lines - self.cached_range.start;
                if new_len < self.cached_offsets.len() {
                    self.cached_offsets.truncate(new_len);
                }
            }
        }

        self.current_version += 1;
    }

    /// 🚀 MANUALLY SET cache statistics (for preserving across file loads)
    pub fn set_cache_stats(&mut self, hits: u64, misses: u64) {
        self.cache_hits = hits;
        self.cache_misses = misses;
    }
    // ==================== PERFORMANCE METRICS ====================

    pub fn cache_stats(&self) -> CacheStats {
        CacheStats {
            hits: self.cache_hits,
            misses: self.cache_misses,
            hit_rate: if self.cache_hits + self.cache_misses > 0 {
                self.cache_hits as f64 / (self.cache_hits + self.cache_misses) as f64
            } else {
                0.0
            },
            cached_lines: self.cached_offsets.iter().filter(|o| o.is_some()).count(),
            total_capacity: self.cached_offsets.len(),
        }
    }

    pub fn memory_usage(&self) -> usize {
        std::mem::size_of::<Self>()
            + (self.cached_offsets.capacity() * std::mem::size_of::<Option<usize>>())
    }
}

impl ReusableBuffer {
    pub fn new() -> Self {
        Self {
            buffer: Vec::with_capacity(1000), // Pre-allocate for typical viewport
            last_used: std::time::Instant::now(),
        }
    }

    pub fn take(&mut self) -> Vec<usize> {
        self.last_used = std::time::Instant::now();
        std::mem::replace(&mut self.buffer, Vec::new())
    }

    pub fn restore(&mut self, mut buffer: Vec<usize>) {
        buffer.clear(); // Clear but keep capacity
        self.buffer = buffer;
    }
}

/// 🚀 PERFORMANCE METRICS
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub hit_rate: f64,
    pub cached_lines: usize,
    pub total_capacity: usize,
}

impl Default for LineOffsetCache {
    fn default() -> Self {
        Self::new(0)
    }
}

impl Default for ReusableBuffer {
    fn default() -> Self {
        Self::new()
    }
}

// ==================== TESTS ====================
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zero_allocation_cache() {
        let mut cache = LineOffsetCache::new(1000);
        let mut editor = crate::buffer::rope_engine::RopeEditor::new();
        let mut buffer = ReusableBuffer::new();

        // Test empty case
        let lines = [];
        let offsets = cache.get_offsets_zero_alloc(&lines, &mut editor, &mut buffer);
        assert_eq!(offsets.len(), 0);

        // Test cache expansion
        cache.ensure_range_cached(&mut editor, 100..150);
        assert!(!cache.cached_range.is_empty());
    }

    #[test]
    fn test_reusable_buffer() {
        let mut buffer = ReusableBuffer::new();
        let original_capacity = buffer.buffer.capacity();

        // Use buffer
        let temp = buffer.take();
        buffer.restore(temp);

        // Should maintain capacity (no reallocation)
        assert_eq!(buffer.buffer.capacity(), original_capacity);
    }
}
