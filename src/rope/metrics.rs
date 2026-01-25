use super::chunk::Chunk;
use crate::tree::{Item, Summary};
use std::ops::Add;

/// Text metrics - what we track at each node
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TextMetrics {
    pub len: usize,   // Byte length
    pub lines: usize, // Number of newlines
}

impl Add for TextMetrics {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        TextMetrics {
            len: self.len + other.len,
            lines: self.lines + other.lines,
        }
    }
}

impl Summary for TextMetrics {
    fn add_summary(&self, other: &Self) -> Self {
        TextMetrics {
            len: self.len + other.len,
            lines: self.lines + other.lines,
        }
    }
}

/// Make Chunk work with SumTree
impl Item for Chunk {
    type Summary = TextMetrics;

    fn summary(&self) -> TextMetrics {
        TextMetrics {
            len: self.len(),
            lines: self.count_lines(),
        }
    }
}
