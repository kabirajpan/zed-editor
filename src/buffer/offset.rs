/// A byte offset into the buffer
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct Offset(pub usize);

impl Offset {
    /// Create new offset
    pub fn new(offset: usize) -> Self {
        Self(offset)
    }

    /// Zero offset
    pub fn zero() -> Self {
        Self(0)
    }

    /// Get the underlying value
    pub fn value(&self) -> usize {
        self.0
    }
}

impl From<usize> for Offset {
    fn from(value: usize) -> Self {
        Self(value)
    }
}

impl From<Offset> for usize {
    fn from(offset: Offset) -> usize {
        offset.0
    }
}

impl std::fmt::Display for Offset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
