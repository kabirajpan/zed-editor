use super::selection::Selection;
use crate::buffer::{Buffer, Offset, Point};
use crate::history::{History, Transaction};
use crate::syntax::IndentCalculator;
use std::path::Path;
use std::time::Instant;

/// Byte-level record of a single rope edit, used to incrementally update the
/// tree-sitter parse tree without keeping the highlighter in the Editor itself.
#[derive(Debug, Clone)]
pub struct EditEvent {
    pub start_byte: usize,
    pub old_end_byte: usize,
    pub new_end_byte: usize,
}

/// Editor state - buffer + cursor + history
#[derive(Clone)]
pub struct Editor {
    history: History,
    selection: Selection,
    version: u64,
    indent_calculator: IndentCalculator,
    file_path: Option<std::path::PathBuf>,

    // ── Pending INSERT batch (word-level undo) ────────────────────────────────
    // Non-whitespace characters accumulate here. When a space is typed, it is
    // appended to pending_insert and then the whole "word " is committed as one
    // undo unit. When any other boundary occurs (newline, backspace, cursor
    // move, undo) the pending text is committed first.
    pending_insert: String,
    pending_start_cursor: Option<Point>,
    pending_start_buffer: Option<Box<Buffer>>,

    // ── Pending DELETE batch (word-level undo) ────────────────────────────────
    // Consecutive backspace presses accumulate here. The batch is committed as
    // one undo unit when the user stops deleting (starts typing, moves cursor,
    // presses undo, etc.).
    pending_delete: String,
    pending_delete_cursor_before: Option<Point>,
    pending_delete_start_buffer: Option<Box<Buffer>>,
    last_delete_time: Instant, // tracks pause between backspaces for time-based boundary

    last_edit_time: Instant,

    // Tree-sitter sync: every rope mutation pushes one event here.
    // The renderer drains this each frame and forwards to the highlighter.
    pending_edit_events: Vec<EditEvent>,
}

impl Editor {
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
            pending_delete: String::new(),
            pending_delete_cursor_before: None,
            pending_delete_start_buffer: None,
            last_delete_time: Instant::now(),
            last_edit_time: Instant::now(),
            pending_edit_events: Vec::new(),
        }
    }

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
            pending_delete: String::new(),
            pending_delete_cursor_before: None,
            pending_delete_start_buffer: None,
            last_delete_time: Instant::now(),
            last_edit_time: Instant::now(),
            pending_edit_events: Vec::new(),
        }
    }

    pub fn set_file_path(&mut self, path: Option<std::path::PathBuf>) {
        self.file_path = path;
    }

    pub fn file_path(&self) -> Option<&Path> {
        self.file_path.as_deref()
    }

    pub fn buffer(&self) -> &Buffer {
        self.history.current()
    }

    pub fn cursor(&self) -> Point {
        self.selection.end
    }

    pub fn set_cursor(&mut self, point: Point) {
        self.selection = Selection::cursor(point);
    }

    pub fn selection(&self) -> Selection {
        self.selection
    }

    pub fn version(&self) -> u64 {
        self.version
    }

    /// Drain all pending edit events. Call once per frame before highlighting.
    pub fn drain_edit_events(&mut self) -> Vec<EditEvent> {
        std::mem::take(&mut self.pending_edit_events)
    }

    // -------------------------------------------------------------------------
    // Flush helpers — commit pending batches to the undo stack
    // -------------------------------------------------------------------------

    /// Commit the accumulated pending insert ("word " or "word") to undo stack.
    fn flush_pending_insert(&mut self) {
        if self.pending_insert.is_empty() {
            return;
        }

        if let Some(start_cursor) = self.pending_start_cursor.take() {
            let transaction =
                Transaction::insert(self.pending_insert.clone(), start_cursor, self.cursor());

            let before_buffer = self
                .pending_start_buffer
                .take()
                .map(|b| *b)
                .unwrap_or_else(|| self.buffer().clone());

            self.history
                .push(before_buffer, self.buffer().clone(), transaction);
        }

        self.pending_insert.clear();
    }

    /// Commit the accumulated pending delete run to undo stack.
    fn flush_pending_delete(&mut self) {
        if self.pending_delete.is_empty() {
            return;
        }

        if let (Some(cursor_before), Some(before_buffer)) = (
            self.pending_delete_cursor_before.take(),
            self.pending_delete_start_buffer.take(),
        ) {
            let cursor_after = self.cursor();
            let transaction =
                Transaction::delete(self.pending_delete.clone(), cursor_before, cursor_after);
            self.history
                .push(*before_buffer, self.buffer().clone(), transaction);
        }

        self.pending_delete.clear();
    }

    // -------------------------------------------------------------------------
    // Editing operations
    // -------------------------------------------------------------------------

    pub fn insert(&mut self, text: &str) {
        // Any insert ends a backspace run.
        self.flush_pending_delete();

        let cursor_before = self.cursor();

        // ── Newline: flush pending word, then commit newline as its own unit ──
        if text == "\n" {
            self.flush_pending_insert();

            let offset = self.buffer().point_to_offset(cursor_before);
            let rope = self.buffer().rope();
            let indent = self.indent_calculator.calculate_indent_with_rope(
                rope,
                cursor_before.row,
                self.file_path.as_deref(),
            );
            let text_to_insert = format!("\n{}", indent);

            let old_buffer = self.buffer().clone();
            let mut new_buffer = old_buffer.clone();
            new_buffer.insert(offset, &text_to_insert);

            let new_offset = offset.value() + text_to_insert.len();
            let cursor_after = new_buffer.offset_to_point(Offset(new_offset));

            self.pending_edit_events.push(EditEvent {
                start_byte: offset.value(),
                old_end_byte: offset.value(),
                new_end_byte: offset.value() + text_to_insert.len(),
            });

            let transaction = Transaction::insert(text_to_insert, cursor_before, cursor_after);
            self.history.push(old_buffer, new_buffer, transaction);

            self.set_cursor(cursor_after);
            self.version += 1;
            self.last_edit_time = Instant::now();
            return;
        }

        // ── Space: append to pending word, then flush "word " as one unit ─────
        if text == " " {
            // If nothing is pending yet this is a standalone space — still
            // treat it as its own unit by starting a fresh pending batch.
            if self.pending_start_buffer.is_none() {
                self.pending_start_cursor = Some(cursor_before);
                self.pending_start_buffer = Some(Box::new(self.buffer().clone()));
            }

            let offset = self.buffer().point_to_offset(cursor_before);
            let mut new_buffer = self.buffer().clone();
            new_buffer.insert(offset, text);

            let new_offset = offset.value() + text.len();
            let cursor_after = new_buffer.offset_to_point(Offset(new_offset));

            self.pending_edit_events.push(EditEvent {
                start_byte: offset.value(),
                old_end_byte: offset.value(),
                new_end_byte: offset.value() + text.len(),
            });

            // Live-update the buffer so the user sees the space immediately.
            self.history.update_current(new_buffer);
            self.set_cursor(cursor_after);
            self.version += 1;
            self.last_edit_time = Instant::now();

            // Append space to the pending text and flush — "word " is now one
            // complete undo unit.
            self.pending_insert.push_str(text);
            self.flush_pending_insert();
            return;
        }

        // ── Non-whitespace: accumulate into pending word ───────────────────────
        if self.pending_start_cursor.is_none() {
            self.pending_start_cursor = Some(cursor_before);
            self.pending_start_buffer = Some(Box::new(self.buffer().clone()));
        }

        let offset = self.buffer().point_to_offset(cursor_before);
        let mut new_buffer = self.buffer().clone();
        new_buffer.insert(offset, text);

        let new_offset = offset.value() + text.len();
        let cursor_after = new_buffer.offset_to_point(Offset(new_offset));

        self.pending_edit_events.push(EditEvent {
            start_byte: offset.value(),
            old_end_byte: offset.value(),
            new_end_byte: offset.value() + text.len(),
        });

        self.history.update_current(new_buffer);
        self.set_cursor(cursor_after);
        self.version += 1;
        self.last_edit_time = Instant::now();

        self.pending_insert.push_str(text);
    }

    pub fn backspace(&mut self) {
        // Any delete ends a word insert batch.
        self.flush_pending_insert();

        let cursor = self.cursor();
        if cursor.row == 0 && cursor.column == 0 {
            return;
        }

        let cursor_offset = self.buffer().point_to_offset(cursor);
        if cursor_offset.value() == 0 {
            return;
        }

        let start = Offset(cursor_offset.value() - 1);
        let deleted_char = self
            .buffer()
            .rope()
            .slice_bytes(start.value(), cursor_offset.value());

        // Boundary 1: crossed a newline — seal the current line's batch.
        // Each line gets its own undo unit so undo restores line by line.
        if deleted_char == "\n" && !self.pending_delete.is_empty() {
            self.flush_pending_delete();
        }

        // Boundary 2: pause >= 2s — seal the batch, start a fresh one.
        // Lets user undo back to wherever they paused mid-deletion.
        if self.last_delete_time.elapsed().as_secs() >= 2 && !self.pending_delete.is_empty() {
            self.flush_pending_delete();
        }

        // Save the buffer state BEFORE the first delete in this run.
        if self.pending_delete_start_buffer.is_none() {
            self.pending_delete_start_buffer = Some(Box::new(self.buffer().clone()));
            self.pending_delete_cursor_before = Some(cursor);
        }

        let mut new_buffer = self.buffer().clone();
        new_buffer.delete(start, cursor_offset);
        let cursor_after = new_buffer.offset_to_point(start);

        self.pending_edit_events.push(EditEvent {
            start_byte: start.value(),
            old_end_byte: cursor_offset.value(),
            new_end_byte: start.value(),
        });

        // Deleted chars are prepended because we're going backwards.
        self.pending_delete.insert_str(0, &deleted_char);

        // Live-update so the user sees each character disappear immediately.
        self.history.update_current(new_buffer);
        self.set_cursor(cursor_after);
        self.version += 1;
        self.last_edit_time = Instant::now();
        self.last_delete_time = Instant::now();
    }

    pub fn delete(&mut self) {
        self.flush_pending_insert();
        self.flush_pending_delete();

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

            self.pending_edit_events.push(EditEvent {
                start_byte: cursor_offset.value(),
                old_end_byte: end.value(),
                new_end_byte: cursor_offset.value(),
            });

            let transaction = Transaction::delete(deleted_text, cursor, cursor);
            self.history.push(old_buffer, new_buffer, transaction);

            self.version += 1;
            self.last_edit_time = Instant::now();
        }
    }

    // -------------------------------------------------------------------------
    // Undo / Redo
    // -------------------------------------------------------------------------

    pub fn undo(&mut self) {
        // First flush any pending delete run as one unit, then undo it.
        if !self.pending_delete.is_empty() {
            self.flush_pending_delete();
            if let Some(transaction) = self.history.undo() {
                self.set_cursor(transaction.cursor_before);
                self.version += 1;
            }
            return;
        }

        // Pending insert: revert the live buffer to before the word started,
        // and push the word onto the redo stack so Ctrl+Y can bring it back.
        if !self.pending_insert.is_empty() {
            if let Some(before_buffer) = self.pending_start_buffer.take() {
                let current_arc = self.history.current_arc();
                let transaction = Transaction::insert(
                    self.pending_insert.clone(),
                    self.pending_start_cursor.unwrap_or_else(Point::zero),
                    self.cursor(),
                );
                // Push current state to redo so Ctrl+Y works.
                self.history.push_redo(current_arc, transaction);

                self.history.update_current((*before_buffer).clone());
                if let Some(start_cursor) = self.pending_start_cursor {
                    self.set_cursor(start_cursor);
                }
            }
            self.pending_insert.clear();
            self.pending_start_cursor = None;
            self.version += 1;
            return;
        }

        // Normal undo — pop from undo stack.
        if let Some(transaction) = self.history.undo() {
            self.set_cursor(transaction.cursor_before);
            self.version += 1;
        }
        // No EditEvent pushed — caller must full_reset the renderer.
    }

    pub fn redo(&mut self) {
        // Discard any in-progress pending state before jumping forward.
        self.pending_insert.clear();
        self.pending_start_cursor = None;
        self.pending_start_buffer = None;
        self.pending_delete.clear();
        self.pending_delete_cursor_before = None;
        self.pending_delete_start_buffer = None;

        if let Some(transaction) = self.history.redo() {
            self.set_cursor(transaction.cursor_after);
            self.version += 1;
        }
        // No EditEvent pushed — caller must full_reset the renderer.
    }

    pub fn can_undo(&self) -> bool {
        !self.pending_insert.is_empty()
            || !self.pending_delete.is_empty()
            || self.history.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.history.can_redo()
    }

    // -------------------------------------------------------------------------
    // Cursor movement — flush both pending batches before moving
    // -------------------------------------------------------------------------

    pub fn move_left(&mut self) {
        self.flush_pending_insert();
        self.flush_pending_delete();
        let cursor = self.cursor();
        if cursor.column > 0 {
            self.set_cursor(Point::new(cursor.row, cursor.column - 1));
        } else if cursor.row > 0 {
            if let Some(prev_line) = self.buffer().line(cursor.row - 1) {
                self.set_cursor(Point::new(cursor.row - 1, prev_line.len()));
            }
        }
    }

    pub fn move_right(&mut self) {
        self.flush_pending_insert();
        self.flush_pending_delete();
        let cursor = self.cursor();
        if let Some(current_line) = self.buffer().line(cursor.row) {
            if cursor.column < current_line.len() {
                self.set_cursor(Point::new(cursor.row, cursor.column + 1));
            } else if cursor.row + 1 < self.buffer().line_count() {
                self.set_cursor(Point::new(cursor.row + 1, 0));
            }
        }
    }

    pub fn move_up(&mut self) {
        self.flush_pending_insert();
        self.flush_pending_delete();
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

    pub fn move_down(&mut self) {
        self.flush_pending_insert();
        self.flush_pending_delete();
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

    pub fn move_to_line_start(&mut self) {
        self.flush_pending_insert();
        self.flush_pending_delete();
        let cursor = self.cursor();
        self.set_cursor(Point::new(cursor.row, 0));
    }

    pub fn move_to_line_end(&mut self) {
        self.flush_pending_insert();
        self.flush_pending_delete();
        let cursor = self.cursor();
        if let Some(line) = self.buffer().line(cursor.row) {
            self.set_cursor(Point::new(cursor.row, line.len()));
        }
    }

    // -------------------------------------------------------------------------
    // Misc
    // -------------------------------------------------------------------------

    pub fn text(&self) -> String {
        self.buffer().to_string()
    }

    pub fn line_count(&self) -> usize {
        self.buffer().line_count()
    }

    /// Replace entire buffer content (used for formatting).
    /// Caller must call ViewportRenderer::full_reset() after this.
    pub fn replace_all(&mut self, new_text: &str) {
        self.flush_pending_insert();
        self.flush_pending_delete();

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

    pub fn format(
        &mut self,
        formatter: &crate::formatter::Formatter,
        file_path: Option<&Path>,
    ) -> Result<(), String> {
        let current_text = self.text();
        let had_trailing_newline = current_text.ends_with('\n');

        match formatter.format_text(&current_text, file_path) {
            Ok(mut formatted_text) => {
                if !had_trailing_newline && formatted_text.ends_with('\n') {
                    formatted_text.pop();
                }
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
