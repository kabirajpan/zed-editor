use std::collections::HashMap;

/// Simple line offset cache - NO complex predictive scrolling
/// Just caches line_to_byte conversions to avoid repeated scans
#[derive(Debug, Clone)]
pub struct LineCache {
    /// Maps line number -> byte offset
    cache: HashMap<usize, usize>,
    /// Track cache hits/misses for performance monitoring
    hits: u64,
    misses: u64,
}

impl LineCache {
    pub fn new() -> Self {
        Self {
            cache: HashMap::with_capacity(1000),
            hits: 0,
            misses: 0,
        }
    }

    /// Try to get cached offset
    pub fn get(&mut self, line: usize) -> Option<usize> {
        if let Some(&offset) = self.cache.get(&line) {
            self.hits += 1;
            Some(offset)
        } else {
            self.misses += 1;
            None
        }
    }

    /// Store offset in cache
    pub fn insert(&mut self, line: usize, offset: usize) {
        self.cache.insert(line, offset);
    }

    /// Invalidate a single line
    pub fn invalidate_line(&mut self, line: usize) {
        self.cache.remove(&line);
    }

    /// Invalidate from a line onwards (for newline insertions)
    pub fn invalidate_from(&mut self, start_line: usize) {
        self.cache.retain(|&line, _| line < start_line);
    }

    /// Invalidate range of lines
    pub fn invalidate_range(&mut self, start: usize, end: usize) {
        for line in start..=end {
            self.cache.remove(&line);
        }
    }

    /// Clear entire cache (for major edits)
    pub fn clear(&mut self) {
        self.cache.clear();
    }

    /// Get cache statistics
    pub fn stats(&self) -> (u64, u64, f64) {
        let total = self.hits + self.misses;
        let hit_rate = if total > 0 {
            self.hits as f64 / total as f64
        } else {
            0.0
        };
        (self.hits, self.misses, hit_rate)
    }

    /// Get number of cached lines
    pub fn size(&self) -> usize {
        self.cache.len()
    }

    /// Limit cache size (keep most recent entries)
    pub fn limit_size(&mut self, max_entries: usize) {
        if self.cache.len() > max_entries {
            // Simple strategy: clear and rebuild naturally
            self.cache.clear();
        }
    }
}

impl Default for LineCache {
    fn default() -> Self {
        Self::new()
    }
}
