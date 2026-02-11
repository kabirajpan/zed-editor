use super::selection::Selection;
use crate::buffer::{Buffer, Offset, Point};
use crate::history::{History, Transaction};
use crate::syntax::IndentCalculator;
use std::path::Path;
use std::time::Instant;

/// Editor state - buffer + cursor + history
#[derive(Clone)]
pub struct Editor {
    history: History,
    selection: Selection,
    version: u64,
    indent_calculator: IndentCalculator,
    file_path: Option<std::path::PathBuf>,

    // ✅ Batching for word-by-word undo
    pending_insert: String,
    pending_start_cursor: Option<Point>,
    pending_start_buffer: Option<Box<Buffer>>,  // ✅ Save the buffer state BEFORE pending edits
    last_edit_time: Instant,
}

impl Editor {
    /// Create empty editor
    pub fn new() -> Self {
        Self {
            history: History::new(Buffer::new()),
            selection: Selection::cursor(Point::zero()),
            version: 0,
            indent_calculator: IndentCalculator::new(),
            file_path: None,
            pending_insert: String::new(),
            pending_start_cursor: None,
            pending_start_buffer: None,
            last_edit_time: Instant::now(),
        }
    }

    /// Create editor from text
    pub fn from_text(text: &str) -> Self {
        Self {
            history: History::new(Buffer::from_text(text)),
            selection: Selection::cursor(Point::zero()),
            version: 0,
            indent_calculator: IndentCalculator::new(),
            file_path: None,
            pending_insert: String::new(),
            pending_start_cursor: None,
            pending_start_buffer: None,
            last_edit_time: Instant::now(),
        }
    }

    /// Set the file path (needed for language detection)
    pub fn set_file_path(&mut self, path: Option<std::path::PathBuf>) {
        self.file_path = path;
    }

    /// Get file path
    pub fn file_path(&self) -> Option<&Path> {
        self.file_path.as_deref()
    }

    /// Get buffer reference
    pub fn buffer(&self) -> &Buffer {
        self.history.current()
    }

    /// Get cursor position
    pub fn cursor(&self) -> Point {
        self.selection.end
    }

    /// Set cursor position
    pub fn set_cursor(&mut self, point: Point) {
        self.selection = Selection::cursor(point);
    }

    /// Get selection
    pub fn selection(&self) -> Selection {
        self.selection
    }

    /// Get current version (incremented on each edit)
    pub fn version(&self) -> u64 {
        self.version
    }

    /// ✅ Flush pending inserts to history
    fn flush_pending_insert(&mut self) {
        if self.pending_insert.is_empty() {
            return;
        }

        if let Some(start_cursor) = self.pending_start_cursor {
            let transaction =
                Transaction::insert(self.pending_insert.clone(), start_cursor, self.cursor());
            
            // Use the saved buffer state (BEFORE pending edits) and current buffer (AFTER pending edits)
            if let Some(before_buffer) = self.pending_start_buffer.take() {
                let after_buffer = self.buffer().clone();
                self.history.push(*before_buffer, after_buffer, transaction);
            } else {
                // Fallback: shouldn't happen if logic is right
                self.history.push(self.buffer().clone(), self.buffer().clone(), transaction);
            }
        }

        self.pending_insert.clear();
        self.pending_start_cursor = None;
    }

    /// ✅ FIX: Insert with intelligent batching (word-by-word + time-based undo)
    pub fn insert(&mut self, text: &str) {
        let cursor_before = self.cursor();

        // ✅ CRITICAL: Flush pending batch on space/newline OR after 1 second of inactivity
        let is_word_boundary = text == " " || text == "\n";
        let time_based_flush = self.last_edit_time.elapsed().as_millis() > 1000;
        
        // Flush old batch only on timeout, NOT on word boundary (we want space/newline in the batch)
        if time_based_flush && !is_word_boundary && !self.pending_insert.is_empty() {
            self.flush_pending_insert();
        }

        // Start new pending batch if needed and save the buffer state BEFORE editing
        if self.pending_start_cursor.is_none() {
            self.pending_start_cursor = Some(cursor_before);
            self.pending_start_buffer = Some(Box::new(self.buffer().clone()));  // ✅ SAVE BEFORE STATE
        }

        let offset = self.buffer().point_to_offset(cursor_before);

        // Handle auto-indent for newlines
        let text_to_insert = if text == "\n" {
            let rope = self.buffer().rope();
            let indent = self.indent_calculator.calculate_indent_with_rope(
                rope,
                cursor_before.row,
                self.file_path.as_deref(),
            );
            format!("\n{}", indent)
        } else {
            text.to_string()
        };

        // Apply edit directly
        let mut new_buffer = self.buffer().clone();
        new_buffer.insert(offset, &text_to_insert);

        // Move cursor
        let new_offset = offset.value() + text_to_insert.len();
        let cursor_after = new_buffer.offset_to_point(Offset(new_offset));

        // Update current buffer directly
        self.history.update_current(new_buffer);
        self.set_cursor(cursor_after);
        self.version += 1;
        self.last_edit_time = Instant::now();

        // Add to pending batch (includes space/newline as part of the batch)
        self.pending_insert.push_str(&text_to_insert);

        // Flush AFTER adding the space/newline so they're part of the same batch
        if is_word_boundary {
            self.flush_pending_insert();
        }
    }

    /// Backspace with immediate history save
    pub fn backspace(&mut self) {
        self.flush_pending_insert(); // Flush any pending text inserts
        self.pending_start_buffer = None;  // Clear the saved buffer state

        let cursor = self.cursor();

        if cursor.row == 0 && cursor.column == 0 {
            return;
        }

        let cursor_offset = self.buffer().point_to_offset(cursor);

        if cursor_offset.value() > 0 {
            let start = Offset(cursor_offset.value() - 1);

            let deleted_text = self
                .buffer()
                .rope()
                .slice_bytes(start.value(), cursor_offset.value());

            let old_buffer = self.buffer().clone();
            let mut new_buffer = old_buffer.clone();
            new_buffer.delete(start, cursor_offset);

            let cursor_after = new_buffer.offset_to_point(start);

            let transaction = Transaction::delete(deleted_text, cursor, cursor_after);
            self.history.push(old_buffer, new_buffer, transaction);

            self.set_cursor(cursor_after);
            self.version += 1;
            self.last_edit_time = Instant::now();
        }
    }

    /// Delete with immediate history save
    pub fn delete(&mut self) {
        self.flush_pending_insert(); // Flush any pending text inserts

        let cursor = self.cursor();
        let cursor_offset = self.buffer().point_to_offset(cursor);

        if cursor_offset.value() < self.buffer().len() {
            let end = Offset(cursor_offset.value() + 1);

            let deleted_text = self
                .buffer()
                .rope()
                .slice_bytes(cursor_offset.value(), end.value());

            let old_buffer = self.buffer().clone();
            let mut new_buffer = old_buffer.clone();
            new_buffer.delete(cursor_offset, end);

            let transaction = Transaction::delete(deleted_text, cursor, cursor);
            self.history.push(old_buffer, new_buffer, transaction);

            self.version += 1;
            self.last_edit_time = Instant::now();
        }
    }

    /// ✅ Undo - properly handles pending insert without double-click
    pub fn undo(&mut self) {
        // Check if we have a pending insert
        let had_pending = !self.pending_insert.is_empty();
        
        // Clear pending state
        self.pending_insert.clear();
        self.pending_start_cursor = None;
        self.pending_start_buffer = None;
        
        // Flush would have already been called if needed, but clear it anyway
        
        // Now undo twice if we had a pending insert (once for the flush, once for the real action)
        // But if history is empty after the flush, just undo once
        if had_pending && self.history.can_undo() {
            // We just added pending to history, pop it
            let _ = self.history.undo();
            
            // Now pop the REAL previous action
            if let Some(transaction) = self.history.undo() {
                self.set_cursor(transaction.cursor_before);
                self.version += 1;
            }
        } else if !had_pending {
            // No pending, just normal undo
            if let Some(transaction) = self.history.undo() {
                self.set_cursor(transaction.cursor_before);
                self.version += 1;
            }
        }
    }

    /// ✅ Redo - handles pending insert correctly
    pub fn redo(&mut self) {
        // Clear any pending insert before redo
        self.pending_insert.clear();
        self.pending_start_cursor = None;
        self.pending_start_buffer = None;

        if let Some(transaction) = self.history.redo() {
            // Restore cursor to the state AFTER the redone transaction
            self.set_cursor(transaction.cursor_after);
            self.version += 1;
        }
    }

    /// Check if can undo
    pub fn can_undo(&self) -> bool {
        self.history.can_undo()
    }

    /// Check if can redo
    pub fn can_redo(&self) -> bool {
        self.history.can_redo()
    }

    /// Move cursor left
    pub fn move_left(&mut self) {
        self.flush_pending_insert(); // Flush on cursor movement

        let cursor = self.cursor();

        if cursor.column > 0 {
            self.set_cursor(Point::new(cursor.row, cursor.column - 1));
        } else if cursor.row > 0 {
            if let Some(prev_line) = self.buffer().line(cursor.row - 1) {
                self.set_cursor(Point::new(cursor.row - 1, prev_line.len()));
            }
        }
    }

    /// Move cursor right
    pub fn move_right(&mut self) {
        self.flush_pending_insert(); // Flush on cursor movement

        let cursor = self.cursor();

        if let Some(current_line) = self.buffer().line(cursor.row) {
            if cursor.column < current_line.len() {
                self.set_cursor(Point::new(cursor.row, cursor.column + 1));
            } else if cursor.row + 1 < self.buffer().line_count() {
                self.set_cursor(Point::new(cursor.row + 1, 0));
            }
        }
    }

    /// Move cursor up
    pub fn move_up(&mut self) {
        self.flush_pending_insert(); // Flush on cursor movement

        let cursor = self.cursor();

        if cursor.row > 0 {
            let new_row = cursor.row - 1;
            let column = if let Some(line) = self.buffer().line(new_row) {
                cursor.column.min(line.len())
            } else {
                0
            };
            self.set_cursor(Point::new(new_row, column));
        }
    }

    /// Move cursor down
    pub fn move_down(&mut self) {
        self.flush_pending_insert(); // Flush on cursor movement

        let cursor = self.cursor();

        if cursor.row + 1 < self.buffer().line_count() {
            let new_row = cursor.row + 1;
            let column = if let Some(line) = self.buffer().line(new_row) {
                cursor.column.min(line.len())
            } else {
                0
            };
            self.set_cursor(Point::new(new_row, column));
        }
    }

    /// Move cursor to start of line
    pub fn move_to_line_start(&mut self) {
        self.flush_pending_insert();

        let cursor = self.cursor();
        self.set_cursor(Point::new(cursor.row, 0));
    }

    /// Move cursor to end of line
    pub fn move_to_line_end(&mut self) {
        self.flush_pending_insert();

        let cursor = self.cursor();
        if let Some(line) = self.buffer().line(cursor.row) {
            self.set_cursor(Point::new(cursor.row, line.len()));
        }
    }

    /// Get text content
    pub fn text(&self) -> String {
        self.buffer().to_string()
    }

    /// Get line count
    pub fn line_count(&self) -> usize {
        self.buffer().line_count()
    }

    /// Replace entire buffer content (used for formatting)
    pub fn replace_all(&mut self, new_text: &str) {
        self.flush_pending_insert();

        let old_cursor = self.cursor();
        let old_buffer = self.buffer().clone();
        let new_buffer = Buffer::from_text(new_text);

        let new_cursor = if old_cursor.row < new_buffer.line_count() {
            if let Some(line) = new_buffer.line(old_cursor.row) {
                Point::new(old_cursor.row, old_cursor.column.min(line.len()))
            } else {
                Point::zero()
            }
        } else {
            let last_row = new_buffer.line_count().saturating_sub(1);
            if let Some(last_line) = new_buffer.line(last_row) {
                Point::new(last_row, last_line.len())
            } else {
                Point::zero()
            }
        };

        let old_text = self.text();
        let transaction =
            Transaction::replace(old_text, new_text.to_string(), old_cursor, new_cursor);

        self.history.push(old_buffer, new_buffer, transaction);
        self.set_cursor(new_cursor);
        self.version += 1;
    }

    /// Format the buffer using provided formatter
    pub fn format(
        &mut self,
        formatter: &crate::formatter::Formatter,
        file_path: Option<&Path>,
    ) -> Result<(), String> {
        let current_text = self.text();

        match formatter.format_text(&current_text, file_path) {
            Ok(formatted_text) => {
                if formatted_text != current_text {
                    self.replace_all(&formatted_text);
                }
                Ok(())
            }
            Err(e) => Err(format!("Format failed: {:?}", e)),
        }
    }
}

impl Default for Editor {
    fn default() -> Self {
        Self::new()
    }
}
