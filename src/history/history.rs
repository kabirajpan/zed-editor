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
            // Edits are now pre-sorted in Transaction::new
            for edit in transaction.edits.iter().rev() {
                let edit_start = edit.offset.value();
                let edit_len = edit.new_text.len();
                let old_len = edit.old_text.len();

                if offset >= edit_start && offset < edit_start + edit_len {
                    return Some((transaction, edit.author));
                }

                if offset >= edit_start + edit_len {
                    offset = (offset as isize - (edit_len as isize - old_len as isize)) as usize;
                }
            }
        }
        None
    }

    /// 🚀 NEW: Optimized Range Lookup for Layer 2.
    /// Returns a list of (start_offset, end_offset, Author) spans within the given range.
    /// This is significantly faster than querying every character.
    pub fn authorship_spans(&self, start: usize, end: usize) -> Vec<(usize, usize, crate::history::transaction::Author)> {
        let mut spans = Vec::new();
        if start >= end { return spans; }
        
        let mut current_unclaimed = vec![(start, end)];
        
        for (_, transaction) in self.undo_stack.iter().rev() {
            if current_unclaimed.is_empty() { break; }
            
            let mut next_unclaimed = Vec::new();
            
            for (u_start, u_end) in current_unclaimed {
                let mut attributed_in_this_range = false;
                
                // Since transaction.edits are sorted, we can be efficient (though here we just iterate for simplicity)
                for edit in transaction.edits.iter().rev() {
                    let edit_start = edit.offset.value();
                    let edit_len = edit.new_text.len();
                    
                    let e_start = edit_start;
                    let e_end = edit_start + edit_len;
                    
                    // Intersection of unclaimed range [u_start, u_end] and edit range [e_start, e_end]
                    let i_start = u_start.max(e_start);
                    let i_end = u_end.min(e_end);
                    
                    if i_start < i_end {
                        spans.push((i_start, i_end, edit.author));
                        
                        // Split the unclaimed range
                        if u_start < i_start {
                            next_unclaimed.push((u_start, i_start));
                        }
                        if u_end > i_end {
                            next_unclaimed.push((i_end, u_end));
                        }
                        attributed_in_this_range = true;
                        break; 
                    }
                }
                
                if !attributed_in_this_range {
                    // No edit in this transaction touched this range.
                    // Translate entire range back in time.
                    let mut shifted_start = u_start;
                    let mut shifted_end = u_end;
                    
                    for edit in transaction.edits.iter().rev() {
                        let edit_start = edit.offset.value();
                        let edit_len = edit.new_text.len();
                        let old_len = edit.old_text.len();
                        let delta = edit_len as isize - old_len as isize;
                        
                        if shifted_start >= edit_start + edit_len {
                            shifted_start = (shifted_start as isize - delta) as usize;
                        }
                        if shifted_end >= edit_start + edit_len {
                            shifted_end = (shifted_end as isize - delta) as usize;
                        }
                    }
                    next_unclaimed.push((shifted_start, shifted_end));
                }
            }
            current_unclaimed = next_unclaimed;
        }
        
        // Anything remaining is Human
        for (u_start, u_end) in current_unclaimed {
            spans.push((u_start, u_end, crate::history::transaction::Author::Human));
        }
        
        spans.sort_by_key(|s| s.0);
        spans
    }

    /// Expose current Arc<Buffer> so editor can push it to redo stack directly.
    pub fn current_arc(&self) -> Arc<Buffer> {
        self.current.clone()
    }

    pub fn undo_stack_len(&self) -> usize {
        self.undo_stack.len()
    }

    pub fn get_transaction_mut(&mut self, index: usize) -> &mut Transaction {
        &mut self.undo_stack[index].1
    }

    pub fn pop_redo(&mut self) -> Option<(Arc<Buffer>, Transaction)> {
        self.redo_stack.pop()
    }
}
