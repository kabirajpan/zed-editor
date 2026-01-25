use super::sum_tree::{Item, Node, SumTree};
use std::sync::Arc;

/// Cursor for navigating through a SumTree
#[allow(dead_code)] // We'll use this later
pub struct Cursor<'a, T: Item> {
    tree: &'a SumTree<T>,
    stack: Vec<(Arc<Node<T>>, usize)>, // (node, child_index)
    position: usize,                   // Current byte position
}

impl<'a, T: Item> Cursor<'a, T> {
    /// Create cursor at the beginning of tree
    pub fn new(tree: &'a SumTree<T>) -> Self {
        Self {
            tree,
            stack: Vec::new(),
            position: 0,
        }
    }

    /// Get current position
    pub fn position(&self) -> usize {
        self.position
    }
}
