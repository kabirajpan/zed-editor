use super::summary::Summary;
use std::sync::Arc;

/// Item stored in the tree - must be able to produce a Summary
pub trait Item: Clone {
    type Summary: Summary;

    fn summary(&self) -> Self::Summary;
}

/// SumTree node - either leaf or internal
#[derive(Clone)]
pub enum Node<T: Item> {
    Leaf {
        items: Vec<T>,
        summary: T::Summary,
    },
    Internal {
        children: Vec<Arc<Node<T>>>,
        summary: T::Summary,
    },
}

/// The main SumTree structure
#[derive(Clone)]
pub struct SumTree<T: Item> {
    root: Option<Arc<Node<T>>>,
}

impl<T: Item> SumTree<T> {
    /// Create empty tree
    pub fn new() -> Self {
        Self { root: None }
    }

    /// Get total summary of entire tree
    pub fn summary(&self) -> T::Summary {
        match &self.root {
            Some(node) => node.summary().clone(),
            None => T::Summary::default(),
        }
    }

    /// Check if tree is empty
    pub fn is_empty(&self) -> bool {
        self.root.is_none()
    }

    /// Push item to end (simple append for now)
    pub fn push(&mut self, item: T) {
        let new_summary = item.summary();
        let new_leaf = Arc::new(Node::Leaf {
            items: vec![item],
            summary: new_summary.clone(),
        });

        match self.root.take() {
            None => {
                // Empty tree - new leaf becomes root
                self.root = Some(new_leaf);
            }
            Some(old_root) => {
                // Combine old root + new leaf under internal node
                let combined_summary = old_root.summary().add_summary(&new_summary);
                self.root = Some(Arc::new(Node::Internal {
                    children: vec![old_root, new_leaf],
                    summary: combined_summary,
                }));
            }
        }
    }

    /// Iterate over all items in the tree (in-order)
    pub fn iter(&self) -> SumTreeIter<T> {
        SumTreeIter {
            stack: match &self.root {
                Some(root) => vec![root.clone()],
                None => vec![],
            },
            current_items: vec![],
            current_index: 0,
        }
    }
}

impl<T: Item> Node<T> {
    fn summary(&self) -> &T::Summary {
        match self {
            Node::Leaf { summary, .. } => summary,
            Node::Internal { summary, .. } => summary,
        }
    }
}

impl<T: Item> Default for SumTree<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// Iterator for SumTree
pub struct SumTreeIter<T: Item> {
    stack: Vec<Arc<Node<T>>>,
    current_items: Vec<T>,
    current_index: usize,
}

impl<T: Item> Iterator for SumTreeIter<T> {
    type Item = T;

    fn next(&mut self) -> Option<T> {
        loop {
            // If we have items in current leaf, return next one
            if self.current_index < self.current_items.len() {
                let item = self.current_items[self.current_index].clone();
                self.current_index += 1;
                return Some(item);
            }

            // No more items in current leaf, get next node from stack
            let node = self.stack.pop()?;

            match node.as_ref() {
                Node::Leaf { items, .. } => {
                    // Found a leaf - prepare to iterate its items
                    self.current_items = items.clone();
                    self.current_index = 0;
                }
                Node::Internal { children, .. } => {
                    // Internal node - push children to stack in reverse order
                    // (so we process left-to-right)
                    for child in children.iter().rev() {
                        self.stack.push(child.clone());
                    }
                }
            }
        }
    }
}
