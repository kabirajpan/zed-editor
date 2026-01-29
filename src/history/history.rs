use super::transaction::Transaction;
use crate::buffer::Buffer;
use std::sync::Arc;

/// History manager - uses Arc for cheap cloning
#[derive(Clone)]
pub struct History {
    undo_stack: Vec<(Arc<Buffer>, Transaction)>,
    redo_stack: Vec<(Arc<Buffer>, Transaction)>,
    current: Arc<Buffer>,
}

impl History {
    pub fn new(buffer: Buffer) -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            current: Arc::new(buffer),
        }
    }

    pub fn current(&self) -> &Buffer {
        &self.current
    }

    /// 🚀 NEW: Update current buffer without saving to undo stack
    /// Used for batched edits - we update the buffer live, then save to history later
    pub fn update_current(&mut self, new_buffer: Buffer) {
        self.current = Arc::new(new_buffer);
    }

    pub fn push(&mut self, new_buffer: Buffer, transaction: Transaction) {
        self.undo_stack.push((self.current.clone(), transaction));
        self.current = Arc::new(new_buffer);
        self.redo_stack.clear();
    }

    pub fn undo(&mut self) -> Option<Transaction> {
        if let Some((previous_buffer, transaction)) = self.undo_stack.pop() {
            self.redo_stack
                .push((self.current.clone(), transaction.clone()));
            self.current = previous_buffer;
            Some(transaction)
        } else {
            None
        }
    }

    pub fn redo(&mut self) -> Option<Transaction> {
        if let Some((next_buffer, transaction)) = self.redo_stack.pop() {
            self.undo_stack
                .push((self.current.clone(), transaction.clone()));
            self.current = next_buffer;
            Some(transaction)
        } else {
            None
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }
}
