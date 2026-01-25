use crate::buffer::Point; // Remove Offset

/// Text selection (range)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Selection {
    pub start: Point,
    pub end: Point,
}

impl Selection {
    /// Create new selection
    pub fn new(start: Point, end: Point) -> Self {
        Self { start, end }
    }

    /// Create selection at a single point (cursor)
    pub fn cursor(point: Point) -> Self {
        Self {
            start: point,
            end: point,
        }
    }

    /// Check if selection is empty (just a cursor)
    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }

    /// Get the range, ensuring start <= end
    pub fn range(&self) -> (Point, Point) {
        if self.start <= self.end {
            (self.start, self.end)
        } else {
            (self.end, self.start)
        }
    }
}
