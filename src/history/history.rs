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

    pub fn current_mut(&mut self) -> &mut Buffer {
        Arc::make_mut(&mut self.current)
    }

    /// Update current buffer without saving to undo stack.
    /// Used for live batched edits — we update the buffer incrementally,
    /// then commit the whole batch to history via push() when the word is done.
    pub fn update_current(&mut self, new_buffer: Buffer) {
        self.current = Arc::new(new_buffer);
    }

    /// Commit a completed edit to the undo stack.
    /// Clears the redo stack — any new edit breaks the redo chain.
    pub fn push(&mut self, old_buffer: Buffer, new_buffer: Buffer, transaction: Transaction) {
        self.undo_stack.push((Arc::new(old_buffer), transaction));
        self.current = Arc::new(new_buffer);
        self.redo_stack.clear();
    }

    /// Manually push a state onto the redo stack.
    /// Used when undoing a pending (un-committed) insert, so redo can
    /// bring it back correctly.
    pub fn push_redo(&mut self, buffer: Arc<Buffer>, transaction: Transaction) {
        self.redo_stack.push((buffer, transaction));
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

    pub fn last_transaction(&self) -> Option<&Transaction> {
        self.undo_stack.last().map(|(_, txn)| txn)
    }

    pub fn is_empty(&self) -> bool {
        self.undo_stack.is_empty()
    }

    pub fn last_transaction_mut(&mut self) -> Option<&mut Transaction> {
        self.undo_stack.last_mut().map(|(_, txn)| txn)
    }

    /// Returns the author and transaction information for the character at the specified byte offset.
    /// Walks backwards through history and translates coordinates to find the origin.
    pub fn author_at(&self, mut offset: usize) -> Option<(&Transaction, crate::history::transaction::Author)> {
        for (_, transaction) in self.undo_stack.iter().rev() {
            // We need to see if this transaction was the "birth" of the byte at 'offset'
            // Edits within a transaction are simultaneous, but we can treat them as ordered reverse for coordinate shift.
            let mut sorted_edits = transaction.edits.clone();
            sorted_edits.sort_unstable_by_key(|e| e.offset.value());

            for edit in sorted_edits.iter().rev() {
                let edit_start = edit.offset.value();
                let edit_len = edit.new_text.len();
                let old_len = edit.old_text.len();

                if offset >= edit_start && offset < edit_start + edit_len {
                    // Match found: This specific edit in this transaction created this byte!
                    return Some((transaction, edit.author));
                }

                // Coordinate translation: where was this offset BEFORE this edit?
                if offset >= edit_start + edit_len {
                    offset = (offset as isize - (edit_len as isize - old_len as isize)) as usize;
                }
            }
        }
        None
    }

    /// Expose current Arc<Buffer> so editor can push it to redo stack directly.
    pub fn current_arc(&self) -> Arc<Buffer> {
        self.current.clone()
    }
}
