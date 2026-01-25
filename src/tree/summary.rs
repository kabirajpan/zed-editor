use std::ops::Add;

/// Summary trait - allows different types of metadata aggregation
/// Examples: text length, line count, character count, etc.
pub trait Summary: Default + Clone + Add<Output = Self> {
    /// Combine two summaries
    fn add_summary(&self, other: &Self) -> Self;
}

/// Simple example: just track total count/length
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Count {
    pub value: usize,
}

impl Add for Count {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Count {
            value: self.value + other.value,
        }
    }
}

impl Summary for Count {
    fn add_summary(&self, other: &Self) -> Self {
        Count {
            value: self.value + other.value,
        }
    }
}

/// Text summary - tracks length AND line count
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TextSummary {
    pub len: usize,   // Byte length
    pub lines: usize, // Number of lines
}

impl Add for TextSummary {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        TextSummary {
            len: self.len + other.len,
            lines: self.lines + other.lines,
        }
    }
}

impl Summary for TextSummary {
    fn add_summary(&self, other: &Self) -> Self {
        TextSummary {
            len: self.len + other.len,
            lines: self.lines + other.lines,
        }
    }
}
