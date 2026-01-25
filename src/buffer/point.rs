use std::cmp::Ordering;

/// A position in the buffer as (row, column)
/// Both are 0-indexed
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Point {
    pub row: usize,
    pub column: usize,
}

impl Point {
    /// Create new point
    pub fn new(row: usize, column: usize) -> Self {
        Self { row, column }
    }

    /// Origin point (0, 0)
    pub fn zero() -> Self {
        Self { row: 0, column: 0 }
    }
}

impl PartialOrd for Point {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Point {
    fn cmp(&self, other: &Self) -> Ordering {
        self.row
            .cmp(&other.row)
            .then(self.column.cmp(&other.column))
    }
}

impl std::fmt::Display for Point {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.row + 1, self.column + 1) // 1-indexed for display
    }
}
