use super::transaction::Transaction;
use crate::buffer::Buffer;
use std::sync::Arc;

/// History manager - stores buffer snapshots for undo/redo
#[derive(Clone)]
pub struct History {
    /// Stack of previous buffer states
    undo_stack: Vec<(Arc<Buffer>, Transaction)>,
    /// Stack of undone states (for redo)
    redo_stack: Vec<(Arc<Buffer>, Transaction)>,
    /// Current buffer
    current: Arc<Buffer>,
}

impl History {
    /// Create new history with initial buffer
    pub fn new(buffer: Buffer) -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            current: Arc::new(buffer),
        }
    }

    /// Get current buffer
    pub fn current(&self) -> &Buffer {
        &self.current
    }

    /// Push new state (after an edit)
    pub fn push(&mut self, new_buffer: Buffer, transaction: Transaction) {
        // Save current state to undo stack
        self.undo_stack.push((self.current.clone(), transaction));

        // Update current
        self.current = Arc::new(new_buffer);

        // Clear redo stack (new edits invalidate redo)
        self.redo_stack.clear();
    }

    /// Undo last operation
    pub fn undo(&mut self) -> Option<Transaction> {
        if let Some((previous_buffer, transaction)) = self.undo_stack.pop() {
            // Save current to redo stack
            self.redo_stack
                .push((self.current.clone(), transaction.clone()));

            // Restore previous
            self.current = previous_buffer;

            Some(transaction)
        } else {
            None
        }
    }

    /// Redo last undone operation
    pub fn redo(&mut self) -> Option<Transaction> {
        if let Some((next_buffer, transaction)) = self.redo_stack.pop() {
            // Save current to undo stack
            self.undo_stack
                .push((self.current.clone(), transaction.clone()));

            // Restore next
            self.current = next_buffer;

            Some(transaction)
        } else {
            None
        }
    }

    /// Check if can undo
    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    /// Check if can redo
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }
}
