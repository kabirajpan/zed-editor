use super::selection::Selection;
use crate::buffer::{Buffer, Offset, Point};
use crate::history::{History, Transaction};
use crate::syntax::IndentCalculator;
use std::path::Path;
use std::time::Instant;

#[derive(Debug, Clone)]
pub struct EditEvent {
    pub start_byte: usize,
    pub old_end_byte: usize,
    pub new_end_byte: usize,
    pub start_position: Point,
    pub old_end_position: Point,
    pub new_end_position: Point,
}

#[derive(Clone)]
pub struct Editor {
    history: History,
    selection: Selection,
    version: u64,
    indent_calculator: IndentCalculator,
    file_path: Option<std::path::PathBuf>,

    // ── Pending INSERT batch (word-level undo) ────────────────────────────────
    pending_insert: String,
    pending_start_cursor: Option<Point>,
    pending_start_buffer: Option<Box<Buffer>>,

    // ── Pending DELETE batch (word-level undo) ────────────────────────────────
    pending_delete: String,
    pending_delete_cursor_before: Option<Point>,
    pending_delete_start_buffer: Option<Box<Buffer>>,
    last_delete_time: Instant,

    last_edit_time: Instant,
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

    pub fn set_selection(&mut self, selection: Selection) {
        self.selection = selection;
    }

    pub fn version(&self) -> u64 {
        self.version
    }

    pub fn drain_edit_events(&mut self) -> Vec<EditEvent> {
        std::mem::take(&mut self.pending_edit_events)
    }

    /// Get the character at a specific point in the buffer.
    pub fn char_at(&self, point: Point) -> Option<char> {
        let offset = self.buffer().point_to_offset(point);
        if offset.value() >= self.buffer().len() {
            return None;
        }
        // Slice 4 bytes to ensure we capture at least one full UTF-8 character
        let end = (offset.value() + 4).min(self.buffer().len());
        let s = self.buffer().slice_bytes(offset.value(), end);
        s.chars().next()
    }

    // -------------------------------------------------------------------------
    // Word boundary
    // -------------------------------------------------------------------------

    fn is_word_char(c: char) -> bool {
        c.is_alphanumeric() || c == '_'
    }

    // -------------------------------------------------------------------------
    // Flush helpers
    // -------------------------------------------------------------------------

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
    // Editing
    // -------------------------------------------------------------------------

    pub fn insert(&mut self, text: &str) {
        // Selection → delete it first, then insert (replaces selected text)
        if !self.selection.is_empty() {
            self.delete_selection();
        }

        self.flush_pending_delete();
        let cursor_before = self.cursor();

        // Newline: flush word, commit newline as own unit
        if text == "\n" {
            self.flush_pending_insert();
            let offset = self.buffer().point_to_offset(cursor_before);
            let rope = self.buffer().rope();

            // ── CHECK FOR ELECTRIC BRACES ────────────────────────────────────
            // Detect if we are between { and }
            let mut is_braces = false;
            if cursor_before.column > 0 {
                let prev = self.char_at(Point::new(cursor_before.row, cursor_before.column - 1));
                let next = self.char_at(cursor_before);
                if prev == Some('{') && next == Some('}') {
                    is_braces = true;
                }
            }

            let indent = self.indent_calculator.calculate_indent_with_rope(
                rope,
                cursor_before.row,
                self.file_path.as_deref(),
            );

            if is_braces {
                // Determine the base indentation
                let line_text = rope.line(cursor_before.row).unwrap_or_default();
                let base_indent: String = line_text
                    .chars()
                    .take_while(|c| c.is_whitespace())
                    .collect();

                // For Electric Braces, we ALWAYS want one extra level of indentation 
                // regardless of whether the brackets on the current line are "balanced"
                let indent = format!("{}{}", base_indent, " ".repeat(4));

                // Multi-line expansion: { \n    | \n }
                let text_to_insert = format!("\n{}\n{}", indent, base_indent);
                let old_buffer = self.buffer().clone();
                let mut new_buffer = old_buffer.clone();
                new_buffer.insert(offset, &text_to_insert);

                // End position is end of inserted text
                let new_offset = offset.value() + text_to_insert.len();
                let cursor_after_final = new_buffer.offset_to_point(Offset(new_offset));

                self.pending_edit_events.push(EditEvent {
                    start_byte: offset.value(),
                    old_end_byte: offset.value(),
                    new_end_byte: offset.value() + text_to_insert.len(),
                    start_position: cursor_before,
                    old_end_position: cursor_before,
                    new_end_position: cursor_after_final,
                });

                let transaction = Transaction::insert(text_to_insert, cursor_before, cursor_after_final);

                // Set cursor to the middle line (indented). Calculate BEFORE moving new_buffer.
                let middle_line_offset = offset.value() + 1 + indent.len();
                let cursor_middle = new_buffer.offset_to_point(Offset(middle_line_offset));

                self.history.push(old_buffer, new_buffer, transaction);

                self.set_cursor(cursor_middle);
                self.version += 1;
                self.last_edit_time = Instant::now();
                return;
            }

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
                start_position: cursor_before,
                old_end_position: cursor_before,
                new_end_position: cursor_after,
            });
            let transaction = Transaction::insert(text_to_insert, cursor_before, cursor_after);
            self.history.push(old_buffer, new_buffer, transaction);
            self.set_cursor(cursor_after);
            self.version += 1;
            self.last_edit_time = Instant::now();
            return;
        }

        // Space: append to pending word then flush "word " as one unit
        if text == " " {
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
                start_position: cursor_before,
                old_end_position: cursor_before,
                new_end_position: cursor_after,
            });
            self.history.update_current(new_buffer);
            self.set_cursor(cursor_after);
            self.version += 1;
            self.last_edit_time = Instant::now();
            self.pending_insert.push_str(text);
            self.flush_pending_insert();
            return;
        }

        // Non-whitespace: accumulate
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
            start_position: cursor_before,
            old_end_position: cursor_before,
            new_end_position: cursor_after,
        });
        self.history.update_current(new_buffer);
        self.set_cursor(cursor_after);
        self.version += 1;
        self.last_edit_time = Instant::now();
        self.pending_insert.push_str(text);

        // If this is a block insert (e.g. from a past or multi-char macro),
        // flush it immediately so it becomes its own undo step.
        if text.len() > 1 {
            self.flush_pending_insert();
        }
    }

    /// Paste text into the editor, replacing selection if any.
    /// Unlike insert(), this always flushes and uses a single transaction
    /// for selection replacement.
    pub fn paste(&mut self, text: &str) {
        if self.selection.is_empty() {
            self.insert(text);
            self.flush_pending_insert();
            return;
        }

        self.flush_pending_insert();
        self.flush_pending_delete();

        let (sel_start, sel_end) = self.selection.range();
        let start_offset = self.buffer().point_to_offset(sel_start);
        let end_offset = self.buffer().point_to_offset(sel_end);
        let old_text = self
            .buffer()
            .slice_bytes(start_offset.value(), end_offset.value());

        let old_buffer = self.buffer().clone();
        let mut new_buffer = old_buffer.clone();

        new_buffer.delete(start_offset, end_offset);
        new_buffer.insert(start_offset, text);

        let new_byte_offset = start_offset.value() + text.len();
        let cursor_after = new_buffer.offset_to_point(Offset(new_byte_offset));

        self.pending_edit_events.push(EditEvent {
            start_byte: start_offset.value(),
            old_end_byte: end_offset.value(),
            new_end_byte: new_byte_offset,
            start_position: sel_start,
            old_end_position: sel_end,
            new_end_position: cursor_after,
        });

        // Use Replace transaction for atomic undo/redo of the selection replacement.
        let transaction =
            Transaction::replace(old_text, text.to_string(), self.cursor(), cursor_after);
        self.history.push(old_buffer, new_buffer, transaction);
        self.set_cursor(cursor_after);
        self.version += 1;
        self.last_edit_time = Instant::now();
    }

    /// Paste a whole line (VS Code behaviour).
    /// Always pastes as a new line above the current cursor position.
    pub fn paste_line(&mut self, text: &str) {
        self.flush_pending_insert();
        let row = self.cursor().row;
        self.set_cursor(crate::buffer::Point::new(row, 0));
        self.insert(text);
        if !text.ends_with('\n') {
            self.insert("\n");
        }
        self.flush_pending_insert();
    }

    pub fn backspace(&mut self) {
        if !self.selection.is_empty() {
            self.delete_selection();
            return;
        }
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

        // Boundary 1: crossed newline — seal current line's batch
        if deleted_char == "\n" && !self.pending_delete.is_empty() {
            self.flush_pending_delete();
        }
        // Boundary 2: pause >= 2s — seal batch
        if self.last_delete_time.elapsed().as_secs() >= 2 && !self.pending_delete.is_empty() {
            self.flush_pending_delete();
        }

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
            start_position: cursor_after,
            old_end_position: cursor,
            new_end_position: cursor_after,
        });
        self.pending_delete.insert_str(0, &deleted_char);
        self.history.update_current(new_buffer);
        self.set_cursor(cursor_after);
        self.version += 1;
        self.last_edit_time = Instant::now();
        self.last_delete_time = Instant::now();
    }

    pub fn delete(&mut self) {
        if !self.selection.is_empty() {
            self.delete_selection();
            return;
        }
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
                start_position: cursor,
                old_end_position: new_buffer.offset_to_point(end), // Use point before delete
                new_end_position: cursor,
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
        if !self.pending_delete.is_empty() {
            self.flush_pending_delete();
            if let Some(transaction) = self.history.undo() {
                let (start, old_end, new_end) = self.compute_txn_ranges(&transaction, true);
                self.pending_edit_events.push(EditEvent {
                    start_byte: start.0.value(),
                    old_end_byte: old_end.0.value(),
                    new_end_byte: new_end.0.value(),
                    start_position: start.1,
                    old_end_position: old_end.1,
                    new_end_position: new_end.1,
                });
                self.set_cursor(transaction.cursor_before);
                self.version += 1;
            }
            return;
        }
        if !self.pending_insert.is_empty() {
            if let Some(before_buffer) = self.pending_start_buffer.take() {
                let current_arc = self.history.current_arc();
                let transaction = Transaction::insert(
                    self.pending_insert.clone(),
                    self.pending_start_cursor.unwrap_or_else(Point::zero),
                    self.cursor(),
                );
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
        if let Some(transaction) = self.history.undo() {
            let (start, old_end, new_end) = self.compute_txn_ranges(&transaction, true);
            self.pending_edit_events.push(EditEvent {
                start_byte: start.0.value(),
                old_end_byte: old_end.0.value(),
                new_end_byte: new_end.0.value(),
                start_position: start.1,
                old_end_position: old_end.1,
                new_end_position: new_end.1,
            });
            self.set_cursor(transaction.cursor_before);
            self.version += 1;
        }
    }

    pub fn redo(&mut self) {
        self.pending_insert.clear();
        self.pending_start_cursor = None;
        self.pending_start_buffer = None;
        self.pending_delete.clear();
        self.pending_delete_cursor_before = None;
        self.pending_delete_start_buffer = None;
        if let Some(transaction) = self.history.redo() {
            let (start, old_end, new_end) = self.compute_txn_ranges(&transaction, false);
            self.pending_edit_events.push(EditEvent {
                start_byte: start.0.value(),
                old_end_byte: old_end.0.value(),
                new_end_byte: new_end.0.value(),
                start_position: start.1,
                old_end_position: old_end.1,
                new_end_position: new_end.1,
            });
            self.set_cursor(transaction.cursor_after);
            self.version += 1;
        }
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
    // Cursor movement
    // -------------------------------------------------------------------------

    pub fn move_left(&mut self) {
        self.flush_pending_insert();
        self.flush_pending_delete();
        if !self.selection.is_empty() {
            let (start, _) = self.selection.range();
            self.set_cursor(start);
            return;
        }
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
        if !self.selection.is_empty() {
            let (_, end) = self.selection.range();
            self.set_cursor(end);
            return;
        }
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
            let column = self
                .buffer()
                .line(new_row)
                .map(|l| cursor.column.min(l.len()))
                .unwrap_or(0);
            self.set_cursor(Point::new(new_row, column));
        }
    }

    pub fn move_down(&mut self) {
        self.flush_pending_insert();
        self.flush_pending_delete();
        let cursor = self.cursor();
        if cursor.row + 1 < self.buffer().line_count() {
            let new_row = cursor.row + 1;
            let column = self
                .buffer()
                .line(new_row)
                .map(|l| cursor.column.min(l.len()))
                .unwrap_or(0);
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

    /// Ctrl+← : jump to start of previous word
    pub fn move_word_left(&mut self) {
        self.flush_pending_insert();
        self.flush_pending_delete();
        if !self.selection.is_empty() {
            let (start, _) = self.selection.range();
            self.set_cursor(start);
            return;
        }
        let target = self.word_start_before_cursor();
        self.set_cursor(target);
    }

    /// Ctrl+→ : jump to end of next word
    pub fn move_word_right(&mut self) {
        self.flush_pending_insert();
        self.flush_pending_delete();
        if !self.selection.is_empty() {
            let (_, end) = self.selection.range();
            self.set_cursor(end);
            return;
        }
        let target = self.word_end_after_cursor();
        self.set_cursor(target);
    }

    /// Ctrl+Home
    pub fn move_to_top(&mut self) {
        self.flush_pending_insert();
        self.flush_pending_delete();
        self.set_cursor(Point::zero());
    }

    /// Ctrl+End
    pub fn move_to_bottom(&mut self) {
        self.flush_pending_insert();
        self.flush_pending_delete();
        let last_row = self.buffer().line_count().saturating_sub(1);
        let col = self.buffer().line(last_row).map(|l| l.len()).unwrap_or(0);
        self.set_cursor(Point::new(last_row, col));
    }

    // -------------------------------------------------------------------------
    // Word navigation helpers
    // -------------------------------------------------------------------------

    pub fn word_start_before_cursor(&self) -> Point {
        let cursor = self.cursor();
        let line = self.buffer().line(cursor.row).unwrap_or_default();
        let chars: Vec<char> = line.chars().collect();

        if cursor.column == 0 {
            if cursor.row > 0 {
                if let Some(prev_line) = self.buffer().line(cursor.row - 1) {
                    return Point::new(cursor.row - 1, prev_line.len());
                }
            }
            return cursor;
        }

        let mut col = cursor.column.min(chars.len());
        while col > 0 && chars[col - 1].is_whitespace() {
            col -= 1;
        }
        if col == 0 {
            return Point::new(cursor.row, 0);
        }
        let is_word = Self::is_word_char(chars[col - 1]);
        while col > 0 && Self::is_word_char(chars[col - 1]) == is_word {
            col -= 1;
        }
        Point::new(cursor.row, col)
    }

    pub fn word_end_after_cursor(&self) -> Point {
        let cursor = self.cursor();
        let line = self.buffer().line(cursor.row).unwrap_or_default();
        let chars: Vec<char> = line.chars().collect();

        if cursor.column >= chars.len() {
            if cursor.row + 1 < self.buffer().line_count() {
                return Point::new(cursor.row + 1, 0);
            }
            return cursor;
        }

        let mut col = cursor.column;
        while col < chars.len() && chars[col].is_whitespace() {
            col += 1;
        }
        if col >= chars.len() {
            return Point::new(cursor.row, chars.len());
        }
        let is_word = Self::is_word_char(chars[col]);
        while col < chars.len() && Self::is_word_char(chars[col]) == is_word {
            col += 1;
        }
        Point::new(cursor.row, col)
    }

    // -------------------------------------------------------------------------
    // Selection
    // -------------------------------------------------------------------------

    fn extend_to(&mut self, new_end: Point) {
        self.selection = Selection::new(self.selection.start, new_end);
    }

    /// Shift+←
    pub fn extend_selection_left(&mut self) {
        self.flush_pending_insert();
        self.flush_pending_delete();
        let end = self.selection.end;
        let new_end = if end.column > 0 {
            Point::new(end.row, end.column - 1)
        } else if end.row > 0 {
            let prev_len = self
                .buffer()
                .line(end.row - 1)
                .map(|l| l.len())
                .unwrap_or(0);
            Point::new(end.row - 1, prev_len)
        } else {
            end
        };
        self.extend_to(new_end);
    }

    /// Shift+→
    pub fn extend_selection_right(&mut self) {
        self.flush_pending_insert();
        self.flush_pending_delete();
        let end = self.selection.end;
        let new_end = if let Some(line) = self.buffer().line(end.row) {
            if end.column < line.len() {
                Point::new(end.row, end.column + 1)
            } else if end.row + 1 < self.buffer().line_count() {
                Point::new(end.row + 1, 0)
            } else {
                end
            }
        } else {
            end
        };
        self.extend_to(new_end);
    }

    /// Shift+↑
    pub fn extend_selection_up(&mut self) {
        self.flush_pending_insert();
        self.flush_pending_delete();
        let end = self.selection.end;
        if end.row > 0 {
            let new_row = end.row - 1;
            let col = self
                .buffer()
                .line(new_row)
                .map(|l| end.column.min(l.len()))
                .unwrap_or(0);
            self.extend_to(Point::new(new_row, col));
        }
    }

    /// Shift+↓
    pub fn extend_selection_down(&mut self) {
        self.flush_pending_insert();
        self.flush_pending_delete();
        let end = self.selection.end;
        if end.row + 1 < self.buffer().line_count() {
            let new_row = end.row + 1;
            let col = self
                .buffer()
                .line(new_row)
                .map(|l| end.column.min(l.len()))
                .unwrap_or(0);
            self.extend_to(Point::new(new_row, col));
        }
    }

    /// Ctrl+Shift+←
    pub fn extend_selection_word_left(&mut self) {
        self.flush_pending_insert();
        self.flush_pending_delete();
        let saved_start = self.selection.start;
        let target = self.word_start_before_cursor();
        self.selection = Selection::new(saved_start, target);
    }

    /// Ctrl+Shift+→
    pub fn extend_selection_word_right(&mut self) {
        self.flush_pending_insert();
        self.flush_pending_delete();
        let saved_start = self.selection.start;
        let target = self.word_end_after_cursor();
        self.selection = Selection::new(saved_start, target);
    }

    /// Shift+Home
    pub fn extend_selection_to_line_start(&mut self) {
        self.flush_pending_insert();
        self.flush_pending_delete();
        let end = self.selection.end;
        self.extend_to(Point::new(end.row, 0));
    }

    /// Shift+End
    pub fn extend_selection_to_line_end(&mut self) {
        self.flush_pending_insert();
        self.flush_pending_delete();
        let end = self.selection.end;
        let col = self.buffer().line(end.row).map(|l| l.len()).unwrap_or(0);
        self.extend_to(Point::new(end.row, col));
    }

    /// Ctrl+A
    pub fn select_all(&mut self) {
        self.flush_pending_insert();
        self.flush_pending_delete();
        let last_row = self.buffer().line_count().saturating_sub(1);
        let last_col = self.buffer().line(last_row).map(|l| l.len()).unwrap_or(0);
        self.selection = Selection::new(Point::zero(), Point::new(last_row, last_col));
    }

    /// Double click → select word under cursor
    pub fn select_word_at_cursor(&mut self) {
        self.flush_pending_insert();
        self.flush_pending_delete();
        let cursor = self.cursor();
        let line = self.buffer().line(cursor.row).unwrap_or_default();
        let chars: Vec<char> = line.chars().collect();
        if chars.is_empty() {
            return;
        }
        let col = cursor.column.min(chars.len().saturating_sub(1));
        let is_word = Self::is_word_char(chars[col]);
        let mut start = col;
        while start > 0 && Self::is_word_char(chars[start - 1]) == is_word {
            start -= 1;
        }
        let mut end = col;
        while end < chars.len() && Self::is_word_char(chars[end]) == is_word {
            end += 1;
        }
        self.selection = Selection::new(Point::new(cursor.row, start), Point::new(cursor.row, end));
    }

    /// Triple click → select entire line
    pub fn select_line_at_cursor(&mut self) {
        self.flush_pending_insert();
        self.flush_pending_delete();
        let cursor = self.cursor();
        let line_len = self.buffer().line(cursor.row).map(|l| l.len()).unwrap_or(0);
        self.selection =
            Selection::new(Point::new(cursor.row, 0), Point::new(cursor.row, line_len));
    }

    /// Return selected text, or None if no selection.
    pub fn selected_text(&self) -> Option<String> {
        if self.selection.is_empty() {
            return None;
        }
        let (start, end) = self.selection.range();
        let start_offset = self.buffer().point_to_offset(start);
        let end_offset = self.buffer().point_to_offset(end);
        Some(
            self.buffer()
                .slice_bytes(start_offset.value(), end_offset.value()),
        )
    }

    /// Current line text with trailing newline — for whole-line copy.
    pub fn current_line_text(&self) -> String {
        let cursor = self.cursor();
        let line = self.buffer().line(cursor.row).unwrap_or_default();
        if cursor.row + 1 < self.buffer().line_count() {
            format!("{}\n", line)
        } else {
            line
        }
    }

    /// Delete selected text as one undo unit. Returns true if anything deleted.
    pub fn delete_selection(&mut self) -> bool {
        if self.selection.is_empty() {
            return false;
        }
        self.flush_pending_insert();
        self.flush_pending_delete();
        let (start, end) = self.selection.range();
        let start_offset = self.buffer().point_to_offset(start);
        let end_offset = self.buffer().point_to_offset(end);
        let deleted_text = self
            .buffer()
            .slice_bytes(start_offset.value(), end_offset.value());
        let old_buffer = self.buffer().clone();
        let mut new_buffer = old_buffer.clone();
        new_buffer.delete(start_offset, end_offset);
        self.pending_edit_events.push(EditEvent {
            start_byte: start_offset.value(),
            old_end_byte: end_offset.value(),
            new_end_byte: start_offset.value(),
            start_position: start,
            old_end_position: end,
            new_end_position: start,
        });
        let transaction = Transaction::delete(deleted_text, end, start);
        self.history.push(old_buffer, new_buffer, transaction);
        self.set_cursor(start);
        self.version += 1;
        true
    }

    /// Delete entire current line (Ctrl+Shift+K).
    pub fn delete_line(&mut self) {
        self.flush_pending_insert();
        self.flush_pending_delete();
        let cursor = self.cursor();
        let line_count = self.buffer().line_count();

        let (start_offset, end_offset) = if cursor.row + 1 < line_count {
            let s = self.buffer().point_to_offset(Point::new(cursor.row, 0));
            let e = self.buffer().point_to_offset(Point::new(cursor.row + 1, 0));
            (s, e)
        } else if cursor.row > 0 {
            let prev_len = self
                .buffer()
                .line(cursor.row - 1)
                .map(|l| l.len())
                .unwrap_or(0);
            let s = self
                .buffer()
                .point_to_offset(Point::new(cursor.row - 1, prev_len));
            let end_col = self.buffer().line(cursor.row).map(|l| l.len()).unwrap_or(0);
            let e = self
                .buffer()
                .point_to_offset(Point::new(cursor.row, end_col));
            (s, e)
        } else {
            let end_col = self.buffer().line(0).map(|l| l.len()).unwrap_or(0);
            let s = self.buffer().point_to_offset(Point::new(0, 0));
            let e = self.buffer().point_to_offset(Point::new(0, end_col));
            (s, e)
        };

        let deleted_text = self
            .buffer()
            .slice_bytes(start_offset.value(), end_offset.value());
        let old_buffer = self.buffer().clone();
        let mut new_buffer = old_buffer.clone();
        new_buffer.delete(start_offset, end_offset);
        let new_cursor = if cursor.row < new_buffer.line_count() {
            Point::new(cursor.row, 0)
        } else {
            Point::new(new_buffer.line_count().saturating_sub(1), 0)
        };
        self.pending_edit_events.push(EditEvent {
            start_byte: start_offset.value(),
            old_end_byte: end_offset.value(),
            new_end_byte: start_offset.value(),
            start_position: Point::new(cursor.row, 0),
            old_end_position: if cursor.row + 1 < old_buffer.line_count() {
                Point::new(cursor.row + 1, 0)
            } else {
                Point::new(cursor.row, old_buffer.line(cursor.row).map(|l| l.len()).unwrap_or(0))
            },
            new_end_position: Point::new(cursor.row, 0),
        });
        let transaction = Transaction::delete(deleted_text, cursor, new_cursor);
        self.history.push(old_buffer, new_buffer, transaction);
        self.set_cursor(new_cursor);
        self.version += 1;
    }

    /// Ctrl+Backspace
    pub fn delete_word_backward(&mut self) {
        if self.delete_selection() {
            return;
        }
        self.flush_pending_insert();
        self.flush_pending_delete();
        let cursor = self.cursor();
        let target = self.word_start_before_cursor();
        if target == cursor {
            return;
        }
        let start_offset = self.buffer().point_to_offset(target);
        let end_offset = self.buffer().point_to_offset(cursor);
        let deleted_text = self
            .buffer()
            .slice_bytes(start_offset.value(), end_offset.value());
        let old_buffer = self.buffer().clone();
        let mut new_buffer = old_buffer.clone();
        new_buffer.delete(start_offset, end_offset);
        self.pending_edit_events.push(EditEvent {
            start_byte: start_offset.value(),
            old_end_byte: end_offset.value(),
            new_end_byte: start_offset.value(),
            start_position: target,
            old_end_position: cursor,
            new_end_position: target,
        });
        let transaction = Transaction::delete(deleted_text, cursor, target);
        self.history.push(old_buffer, new_buffer, transaction);
        self.set_cursor(target);
        self.version += 1;
    }

    /// Ctrl+Delete
    pub fn delete_word_forward(&mut self) {
        if self.delete_selection() {
            return;
        }
        self.flush_pending_insert();
        self.flush_pending_delete();
        let cursor = self.cursor();
        let target = self.word_end_after_cursor();
        if target == cursor {
            return;
        }
        let start_offset = self.buffer().point_to_offset(cursor);
        let end_offset = self.buffer().point_to_offset(target);
        let deleted_text = self
            .buffer()
            .slice_bytes(start_offset.value(), end_offset.value());
        let old_buffer = self.buffer().clone();
        let mut new_buffer = old_buffer.clone();
        new_buffer.delete(start_offset, end_offset);
        self.pending_edit_events.push(EditEvent {
            start_byte: start_offset.value(),
            old_end_byte: end_offset.value(),
            new_end_byte: start_offset.value(),
            start_position: cursor,
            old_end_position: target,
            new_end_position: cursor,
        });
        let transaction = Transaction::delete(deleted_text, cursor, cursor);
        self.history.push(old_buffer, new_buffer, transaction);
        self.version += 1;
    }

    // -------------------------------------------------------------------------
    // Misc
    // -------------------------------------------------------------------------

    fn compute_txn_ranges(
        &self,
        txn: &Transaction,
        is_undo: bool,
    ) -> ((Offset, Point), (Offset, Point), (Offset, Point)) {
        use crate::history::transaction::EditKind;
        let buffer = self.buffer();
        match &txn.edit {
            EditKind::Insert { text } => {
                let start_pos = txn.cursor_before;
                let start_offset = buffer.point_to_offset(start_pos);
                let end_pos = txn.cursor_after;
                let end_offset = Offset(start_offset.value() + text.len());

                if is_undo {
                    ((start_offset, start_pos), (end_offset, end_pos), (start_offset, start_pos))
                } else {
                    ((start_offset, start_pos), (start_offset, start_pos), (end_offset, end_pos))
                }
            }
            EditKind::Delete { text } => {
                let start_pos = txn.cursor_after;
                let start_offset = buffer.point_to_offset(start_pos);
                let end_pos = txn.cursor_before;
                let end_offset = Offset(start_offset.value() + text.len());

                if is_undo {
                    ((start_offset, start_pos), (start_offset, start_pos), (end_offset, end_pos))
                } else {
                    ((start_offset, start_pos), (end_offset, end_pos), (start_offset, start_pos))
                }
            }
            EditKind::Replace { old_text, new_text } => {
                let start_pos = Point::zero();
                let start_offset = Offset(0);
                let old_len = old_text.len();
                let new_len = new_text.len();

                if is_undo {
                    ((start_offset, start_pos), (Offset(new_len), buffer.offset_to_point(Offset(new_len))), (Offset(old_len), buffer.offset_to_point(Offset(old_len))))
                } else {
                    ((start_offset, start_pos), (Offset(old_len), buffer.offset_to_point(Offset(old_len))), (Offset(new_len), buffer.offset_to_point(Offset(new_len))))
                }
            }
        }
    }

    pub fn text(&self) -> String {
        self.buffer().to_string()
    }

    pub fn line_count(&self) -> usize {
        self.buffer().line_count()
    }

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
        self.pending_edit_events.push(EditEvent {
            start_byte: 0,
            old_end_byte: old_buffer.len(),
            new_end_byte: new_buffer.len(),
            start_position: Point::zero(),
            old_end_position: old_buffer.offset_to_point(Offset(old_buffer.len())),
            new_end_position: new_buffer.offset_to_point(Offset(new_buffer.len())),
        });
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
