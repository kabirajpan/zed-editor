use super::selection::{Selection, SelectionMode};
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
    selections: Vec<Selection>,
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
            selections: vec![Selection::cursor(Point::zero())],
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
            selections: vec![Selection::cursor(Point::zero())],
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
        self.selections.last().map(|s| s.end).unwrap_or(Point::zero())
    }

    pub fn set_cursor(&mut self, point: Point) {
        self.selections = vec![Selection::cursor(point)];
    }

    pub fn selections(&self) -> &[Selection] {
        &self.selections
    }

    pub fn set_selections(&mut self, selections: Vec<Selection>) {
        self.selections = selections;
        self.normalize_selections();
    }

    pub fn selection(&self) -> Selection {
        self.selections.last().cloned().unwrap_or(Selection::cursor(Point::zero()))
    }

    pub fn set_selection(&mut self, selection: Selection) {
        self.selections = vec![selection];
    }

    pub fn add_selection(&mut self, point: Point) {
        self.selections.push(Selection::cursor(point));
        self.normalize_selections();
    }

    pub fn normalize_selections(&mut self) {
        if self.selections.is_empty() {
            self.selections = vec![Selection::cursor(Point::zero())];
            return;
        }

        // Sort by range start
        self.selections.sort_by_key(|s| s.range().0);

        let mut merged = Vec::new();
        let mut current = self.selections[0].clone();

        for next in self.selections.iter().skip(1) {
            let (c_start, c_end) = current.range();
            let (n_start, n_end) = next.range();

            if n_start <= c_end {
                // Overlap or touching -> merge
                let new_start = c_start;
                let new_end = if c_end > n_end { c_end } else { n_end };
                
                // Keep the cursor position of the 'next' one as it's the more recent addition
                // actually standard behavior is to combine. For now, let's keep the target end.
                current = Selection::new(new_start, new_end);
            } else {
                merged.push(current);
                current = next.clone();
            }
        }
        merged.push(current);
        self.selections = merged;
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
        self.delete_selections();
        self.flush_pending_delete();
        self.flush_pending_insert();

        let mut sorted_selections = self.selections.clone();
        sorted_selections.sort_by_key(|s| s.range().0);

        let mut new_selections = Vec::new();

        // Process in reverse to maintain offset validity
        for selection in sorted_selections.iter().rev() {
            let cursor_before = selection.end;
            let offset = self.buffer().point_to_offset(cursor_before);

            let text_to_insert = if text == "\n" {
                let rope = self.buffer().rope();
                let indent_val = self.indent_calculator.calculate_indent_with_rope(
                    rope,
                    cursor_before.row,
                    self.file_path.as_deref(),
                );
                format!("\n{}", indent_val)
            } else {
                text.to_string()
            };

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
            
            new_selections.push(Selection::cursor(cursor_after));
        }

        new_selections.reverse();
        self.selections = new_selections;
        self.normalize_selections();
        self.version += 1;
        self.last_edit_time = Instant::now();
    }

    /// Paste text into the editor, replacing selection if any.
    /// Unlike insert(), this always flushes and uses a single transaction
    /// for selection replacement.
    pub fn paste(&mut self, text: &str) {
        self.flush_pending_insert();
        self.flush_pending_delete();
        self.delete_selections();
        self.insert(text);
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
        if self.delete_selections() {
            return;
        }

        let mut sorted_selections = self.selections.clone();
        sorted_selections.sort_by_key(|s| s.range().0);
        
        let mut new_selections = Vec::new();
        
        for selection in sorted_selections.iter().rev() {
            let cursor = selection.end;
            if cursor == Point::zero() {
                new_selections.push(Selection::cursor(cursor));
                continue;
            }

            let before = self.move_point_left(cursor);
            let start_offset = self.buffer().point_to_offset(before);
            let end_offset = self.buffer().point_to_offset(cursor);
            let deleted_char = self.char_at(before).unwrap_or('\0');
            
            let old_buffer = self.buffer().clone();
            let mut new_buffer = old_buffer.clone();
            new_buffer.delete(start_offset, end_offset);
            
            self.pending_edit_events.push(EditEvent {
                start_byte: start_offset.value(),
                old_end_byte: end_offset.value(),
                new_end_byte: start_offset.value(),
                start_position: before,
                old_end_position: cursor,
                new_end_position: before,
            });
            
            let transaction = Transaction::delete(deleted_char.to_string(), before, cursor);
            self.history.push(old_buffer, new_buffer, transaction);
            new_selections.push(Selection::cursor(before));
        }

        new_selections.reverse();
        self.selections = new_selections;
        self.normalize_selections();
        self.version += 1;
        self.last_edit_time = Instant::now();
    }
    pub fn delete(&mut self) {
        if self.delete_selections() {
            return;
        }

        let mut sorted_selections = self.selections.clone();
        sorted_selections.sort_by_key(|s| s.range().0);
        
        let mut new_selections = Vec::new();
        
        for selection in sorted_selections.iter().rev() {
            let cursor = selection.end;
            let after = self.move_point_right(cursor);
            if after == cursor {
                new_selections.push(Selection::cursor(cursor));
                continue;
            }

            let start_offset = self.buffer().point_to_offset(cursor);
            let end_offset = self.buffer().point_to_offset(after);
            let deleted_char = self.char_at(cursor).unwrap_or('\0');
            
            let old_buffer = self.buffer().clone();
            let mut new_buffer = old_buffer.clone();
            new_buffer.delete(start_offset, end_offset);
            
            self.pending_edit_events.push(EditEvent {
                start_byte: start_offset.value(),
                old_end_byte: end_offset.value(),
                new_end_byte: start_offset.value(),
                start_position: cursor,
                old_end_position: after,
                new_end_position: cursor,
            });
            
            let transaction = Transaction::delete(deleted_char.to_string(), cursor, after);
            self.history.push(old_buffer, new_buffer, transaction);
            new_selections.push(Selection::cursor(cursor));
        }

        new_selections.reverse();
        self.selections = new_selections;
        self.normalize_selections();
        self.version += 1;
        self.last_edit_time = Instant::now();
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
    // Point-wise movement helpers
    // -------------------------------------------------------------------------

    fn move_point_left(&self, point: Point) -> Point {
        if point.column > 0 {
            Point::new(point.row, point.column - 1)
        } else if point.row > 0 {
            let prev_len = self.buffer().line(point.row - 1).map(|l| l.len()).unwrap_or(0);
            Point::new(point.row - 1, prev_len)
        } else {
            point
        }
    }

    fn move_point_right(&self, point: Point) -> Point {
        if let Some(line) = self.buffer().line(point.row) {
            if point.column < line.len() {
                Point::new(point.row, point.column + 1)
            } else if point.row + 1 < self.buffer().line_count() {
                Point::new(point.row + 1, 0)
            } else {
                point
            }
        } else {
            point
        }
    }

    fn move_point_up(&self, point: Point) -> Point {
        if point.row > 0 {
            let new_row = point.row - 1;
            let col = self.buffer().line(new_row).map(|l| point.column.min(l.len())).unwrap_or(0);
            Point::new(new_row, col)
        } else {
            point
        }
    }

    fn move_point_down(&self, point: Point) -> Point {
        if point.row + 1 < self.buffer().line_count() {
            let new_row = point.row + 1;
            let col = self.buffer().line(new_row).map(|l| point.column.min(l.len())).unwrap_or(0);
            Point::new(new_row, col)
        } else {
            point
        }
    }
    // Cursor movement (Multi-cursor)
    // -------------------------------------------------------------------------

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

    pub fn move_left(&mut self) {
        self.flush_pending_insert();
        self.flush_pending_delete();
        for i in 0..self.selections.len() {
            let selection = self.selections[i];
            if !selection.is_empty() {
                let (start, _) = selection.range();
                self.selections[i] = Selection::cursor(start);
            } else {
                let new_end = self.move_point_left(selection.end);
                self.selections[i] = Selection::cursor(new_end);
            }
        }
        self.normalize_selections();
    }

    pub fn move_right(&mut self) {
        self.flush_pending_insert();
        self.flush_pending_delete();
        for i in 0..self.selections.len() {
            let selection = self.selections[i];
            if !selection.is_empty() {
                let (_, end) = selection.range();
                self.selections[i] = Selection::cursor(end);
            } else {
                let new_end = self.move_point_right(selection.end);
                self.selections[i] = Selection::cursor(new_end);
            }
        }
        self.normalize_selections();
    }

    pub fn move_up(&mut self) {
        self.flush_pending_insert();
        self.flush_pending_delete();
        for i in 0..self.selections.len() {
            let new_end = self.move_point_up(self.selections[i].end);
            self.selections[i] = Selection::cursor(new_end);
        }
        self.normalize_selections();
    }

    pub fn move_down(&mut self) {
        self.flush_pending_insert();
        self.flush_pending_delete();
        for i in 0..self.selections.len() {
            let new_end = self.move_point_down(self.selections[i].end);
            self.selections[i] = Selection::cursor(new_end);
        }
        self.normalize_selections();
    }

    pub fn move_word_left(&mut self) {
        self.flush_pending_insert();
        self.flush_pending_delete();
        for i in 0..self.selections.len() {
            let target = self.word_start_before_point(self.selections[i].end);
            self.selections[i] = Selection::cursor(target);
        }
        self.normalize_selections();
    }

    pub fn move_word_right(&mut self) {
        self.flush_pending_insert();
        self.flush_pending_delete();
        for i in 0..self.selections.len() {
            let target = self.word_end_after_point(self.selections[i].end);
            self.selections[i] = Selection::cursor(target);
        }
        self.normalize_selections();
    }

    pub fn move_to_line_start(&mut self) {
        self.flush_pending_insert();
        self.flush_pending_delete();
        for i in 0..self.selections.len() {
            let row = self.selections[i].end.row;
            self.selections[i] = Selection::cursor(Point::new(row, 0));
        }
        self.normalize_selections();
    }

    pub fn move_to_line_end(&mut self) {
        self.flush_pending_insert();
        self.flush_pending_delete();
        for i in 0..self.selections.len() {
            let row = self.selections[i].end.row;
            let line_len = self.buffer().line(row).map(|l| l.len()).unwrap_or(0);
            self.selections[i] = Selection::cursor(Point::new(row, line_len));
        }
        self.normalize_selections();
    }

    // -------------------------------------------------------------------------
    // Word navigation helpers
    // -------------------------------------------------------------------------

    pub fn word_start_before_point(&self, point: Point) -> Point {
        let line = self.buffer().line(point.row).unwrap_or_default();
        let chars: Vec<char> = line.chars().collect();

        if point.column == 0 {
            if point.row > 0 {
                if let Some(prev_line) = self.buffer().line(point.row - 1) {
                    return Point::new(point.row - 1, prev_line.len());
                }
            }
            return point;
        }

        let mut col = point.column.min(chars.len());
        while col > 0 && chars[col - 1].is_whitespace() {
            col -= 1;
        }
        if col == 0 {
            return Point::new(point.row, 0);
        }
        let is_word = Self::is_word_char(chars[col - 1]);
        while col > 0 && Self::is_word_char(chars[col - 1]) == is_word {
            col -= 1;
        }
        Point::new(point.row, col)
    }

    pub fn word_end_after_point(&self, point: Point) -> Point {
        let line = self.buffer().line(point.row).unwrap_or_default();
        let chars: Vec<char> = line.chars().collect();

        if point.column >= chars.len() {
            if point.row + 1 < self.buffer().line_count() {
                return Point::new(point.row + 1, 0);
            }
            return point;
        }

        let mut col = point.column;
        while col < chars.len() && chars[col].is_whitespace() {
            col += 1;
        }
        if col >= chars.len() {
            return Point::new(point.row, chars.len());
        }
        let is_word = Self::is_word_char(chars[col]);
        while col < chars.len() && Self::is_word_char(chars[col]) == is_word {
            col += 1;
        }
        Point::new(point.row, col)
    }

    // -------------------------------------------------------------------------
    // Selection
    // -------------------------------------------------------------------------

    fn extend_to(&mut self, index: usize, new_end: Point) {
        if let Some(selection) = self.selections.get_mut(index) {
            *selection = Selection::new(selection.start, new_end);
        }
    }

    /// Shift+←
    pub fn extend_selection_left(&mut self) {
        self.flush_pending_insert();
        self.flush_pending_delete();
        for i in 0..self.selections.len() {
            let end = self.selections[i].end;
            let new_end = self.move_point_left(end);
            self.extend_to(i, new_end);
        }
        self.normalize_selections();
    }

    /// Shift+→
    pub fn extend_selection_right(&mut self) {
        self.flush_pending_insert();
        self.flush_pending_delete();
        for i in 0..self.selections.len() {
            let end = self.selections[i].end;
            let new_end = self.move_point_right(end);
            self.extend_to(i, new_end);
        }
        self.normalize_selections();
    }

    /// Shift+↑
    pub fn extend_selection_up(&mut self) {
        self.flush_pending_insert();
        self.flush_pending_delete();
        for i in 0..self.selections.len() {
            let end = self.selections[i].end;
            let new_end = self.move_point_up(end);
            self.extend_to(i, new_end);
        }
        self.normalize_selections();
    }

    /// Shift+↓
    pub fn extend_selection_down(&mut self) {
        self.flush_pending_insert();
        self.flush_pending_delete();
        for i in 0..self.selections.len() {
            let end = self.selections[i].end;
            let new_end = self.move_point_down(end);
            self.extend_to(i, new_end);
        }
        self.normalize_selections();
    }

    /// Ctrl+Shift+←
    pub fn extend_selection_word_left(&mut self) {
        self.flush_pending_insert();
        self.flush_pending_delete();
        for i in 0..self.selections.len() {
            let target = self.word_start_before_point(self.selections[i].end);
            self.extend_to(i, target);
        }
        self.normalize_selections();
    }

    /// Ctrl+Shift+→
    pub fn extend_selection_word_right(&mut self) {
        self.flush_pending_insert();
        self.flush_pending_delete();
        for i in 0..self.selections.len() {
            let target = self.word_end_after_point(self.selections[i].end);
            self.extend_to(i, target);
        }
        self.normalize_selections();
    }

    /// Shift+Home
    pub fn extend_selection_to_line_start(&mut self) {
        self.flush_pending_insert();
        self.flush_pending_delete();
        for i in 0..self.selections.len() {
            let target = Point::new(self.selections[i].end.row, 0);
            self.extend_to(i, target);
        }
        self.normalize_selections();
    }

    /// Shift+End
    pub fn extend_selection_to_line_end(&mut self) {
        self.flush_pending_insert();
        self.flush_pending_delete();
        for i in 0..self.selections.len() {
            let line_len = self.buffer().line(self.selections[i].end.row).map(|l| l.len()).unwrap_or(0);
            let target = Point::new(self.selections[i].end.row, line_len);
            self.extend_to(i, target);
        }
        self.normalize_selections();
    }

    /// Ctrl+A
    pub fn select_all(&mut self) {
        self.flush_pending_insert();
        self.flush_pending_delete();
        let last_row = self.buffer().line_count().saturating_sub(1);
        let last_col = self.buffer().line(last_row).map(|l| l.len()).unwrap_or(0);
        self.selections = vec![Selection::new(Point::zero(), Point::new(last_row, last_col))];
    }

    /// Get the boundaries of a word at a given point.
    pub fn word_range_at(&self, point: Point) -> (Point, Point) {
        let line = self.buffer().line(point.row).unwrap_or_default();
        let chars: Vec<char> = line.chars().collect();
        if chars.is_empty() {
            return (point, point);
        }
        let col = point.column.min(chars.len().saturating_sub(1));
        let is_word = Self::is_word_char(chars[col]);
        let mut start = col;
        while start > 0 && Self::is_word_char(chars[start - 1]) == is_word {
            start -= 1;
        }
        let mut end = col;
        while end < chars.len() && Self::is_word_char(chars[end]) == is_word {
            end += 1;
        }
        (Point::new(point.row, start), Point::new(point.row, end))
    }

    /// Get the boundaries of a line at a given point.
    pub fn line_range_at(&self, point: Point) -> (Point, Point) {
        let line_len = self.buffer().line(point.row).map(|l| l.len()).unwrap_or(0);
        (Point::new(point.row, 0), Point::new(point.row, line_len))
    }

    /// Extend selection from an anchor to a current point using a specific mode.
    pub fn set_selection_with_mode(&mut self, anchor: Point, current: Point, mode: SelectionMode) {
        let selection = match mode {
            SelectionMode::Character => {
                Selection::new(anchor, current)
            }
            SelectionMode::Word => {
                let (anchor_start, anchor_end) = self.word_range_at(anchor);
                let (current_start, current_end) = self.word_range_at(current);
                
                if current >= anchor {
                    Selection::new(anchor_start, current_end)
                } else {
                    Selection::new(anchor_end, current_start)
                }
            }
            SelectionMode::Line => {
                let (anchor_start, anchor_end) = self.line_range_at(anchor);
                let (current_start, current_end) = self.line_range_at(current);
                
                if current >= anchor {
                    Selection::new(anchor_start, current_end)
                } else {
                    Selection::new(anchor_end, current_start)
                }
            }
        };
        
        // Single selection for mouse dragging (unless we add multi-drag later)
        self.selections = vec![selection];
    }

    /// Double click → select word under cursor
    pub fn select_word_at_cursor(&mut self) {
        self.flush_pending_insert();
        self.flush_pending_delete();
        let mut new_selections = Vec::new();
        for selection in &self.selections {
            let (start, end) = self.word_range_at(selection.end);
            new_selections.push(Selection::new(start, end));
        }
        self.selections = new_selections;
        self.normalize_selections();
    }

    /// Triple click → select entire line under cursor
    pub fn select_line_at_cursor(&mut self) {
        self.flush_pending_insert();
        self.flush_pending_delete();
        let mut new_selections = Vec::new();
        for selection in &self.selections {
            let (start, end) = self.line_range_at(selection.end);
            new_selections.push(Selection::new(start, end));
        }
        self.selections = new_selections;
        self.normalize_selections();
    }

    /// Return selected text for all cursors, joined by newlines.
    pub fn selected_text(&self) -> Option<String> {
        let mut results = Vec::new();
        for selection in &self.selections {
            if selection.is_empty() { continue; }
            let (start, end) = selection.range();
            let start_offset = self.buffer().point_to_offset(start);
            let end_offset = self.buffer().point_to_offset(end);
            results.push(self.buffer().slice_bytes(start_offset.value(), end_offset.value()));
        }
        
        if results.is_empty() {
            None
        } else {
            Some(results.join("\n"))
        }
    }

    /// Primary cursor's current line text with trailing newline.
    pub fn current_line_text(&self) -> String {
        let cursor = self.cursor();
        let line = self.buffer().line(cursor.row).unwrap_or_default();
        if cursor.row + 1 < self.buffer().line_count() {
            format!("{}\n", line)
        } else {
            line
        }
    }

    /// Delete selected text for all cursors as one unit. Returns true if anything was deleted.
    pub fn delete_selections(&mut self) -> bool {
        let mut deleted = false;
        self.flush_pending_insert();
        self.flush_pending_delete();

        // Process in reverse to maintain offset validity
        let mut sorted_selections = self.selections.clone();
        sorted_selections.sort_by_key(|s| s.range().0);

        let mut new_selections = Vec::new();

        for selection in sorted_selections.iter().rev() {
            if selection.is_empty() { 
                new_selections.push(selection.clone());
                continue; 
            }
            deleted = true;
            
            let (start, end) = selection.range();
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
            
            new_selections.push(Selection::cursor(start));
        }

        if deleted {
            new_selections.reverse();
            self.selections = new_selections;
            self.normalize_selections();
            self.version += 1;
            self.last_edit_time = Instant::now();
        }
        deleted
    }

    /// Delete entire current line (Ctrl+Shift+K) for all cursors.
    pub fn delete_line(&mut self) {
        self.flush_pending_insert();
        self.flush_pending_delete();
        
        // Identify all unique lines to delete
        let mut rows: Vec<usize> = self.selections.iter().map(|s| s.end.row).collect();
        rows.sort_unstable();
        rows.dedup();
        
        // Process in reverse to maintain offset validity
        for &row in rows.iter().rev() {
            let line_count = self.buffer().line_count();
            if row >= line_count { continue; }

            let (start_offset, end_offset) = if row + 1 < line_count {
                let s = self.buffer().point_to_offset(Point::new(row, 0));
                let e = self.buffer().point_to_offset(Point::new(row + 1, 0));
                (s, e)
            } else if row > 0 {
                let prev_len = self.buffer().line(row - 1).map(|l| l.len()).unwrap_or(0);
                let s = self.buffer().point_to_offset(Point::new(row - 1, prev_len));
                let end_col = self.buffer().line(row).map(|l| l.len()).unwrap_or(0);
                let e = self.buffer().point_to_offset(Point::new(row, end_col));
                (s, e)
            } else {
                let end_col = self.buffer().line(0).map(|l| l.len()).unwrap_or(0);
                let s = self.buffer().point_to_offset(Point::new(0, 0));
                let e = self.buffer().point_to_offset(Point::new(0, end_col));
                (s, e)
            };

            let deleted_text = self.buffer().slice_bytes(start_offset.value(), end_offset.value());
            let old_buffer = self.buffer().clone();
            let mut new_buffer = old_buffer.clone();
            new_buffer.delete(start_offset, end_offset);
            
            self.pending_edit_events.push(EditEvent {
                start_byte: start_offset.value(),
                old_end_byte: end_offset.value(),
                new_end_byte: start_offset.value(),
                start_position: Point::new(row, 0),
                old_end_position: if row + 1 < old_buffer.line_count() {
                    Point::new(row + 1, 0)
                } else {
                    Point::new(row, old_buffer.line(row).map(|l| l.len()).unwrap_or(0))
                },
                new_end_position: Point::new(row, 0),
            });

            let transaction = Transaction::delete(deleted_text, Point::new(row, 0), Point::new(row, 0));
            self.history.push(old_buffer, new_buffer, transaction);
        }

        self.version += 1;
        self.last_edit_time = Instant::now();
        
        let line_count = self.buffer().line_count();
        // Cursors should collapse to start of the row they were on (clamped)
        for selection in self.selections.iter_mut() {
            let row = selection.end.row.min(line_count.saturating_sub(1));
            *selection = Selection::cursor(Point::new(row, 0));
        }
        self.normalize_selections();
    }

    /// Ctrl+Backspace
    pub fn delete_word_backward(&mut self) {
        if self.delete_selections() {
            return;
        }
        self.flush_pending_insert();
        self.flush_pending_delete();
        
        let mut sorted_selections = self.selections.clone();
        sorted_selections.sort_by_key(|s| s.range().0);
        
        // 1. Collect targets first (Immutable borrow phase)
        let mut deletion_tasks = Vec::new();
        for selection in sorted_selections.iter().rev() {
            let cursor = selection.end;
            let target = self.word_start_before_point(cursor);
            if target != cursor {
                deletion_tasks.push((cursor, target));
            }
        }

        if deletion_tasks.is_empty() { return; }

        // 2. Perform deletions (Mutable borrow phase)
        let mut new_selections = Vec::new();
        for (cursor, target) in deletion_tasks {
            let start_offset = self.buffer().point_to_offset(target);
            let end_offset = self.buffer().point_to_offset(cursor);
            let deleted_text = self.buffer().slice_bytes(start_offset.value(), end_offset.value());
            
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
            new_selections.push(Selection::cursor(target));
        }
        
        // Add back non-deleted cursors
        for selection in &sorted_selections {
            if self.word_start_before_point(selection.end) == selection.end {
                new_selections.push(selection.clone());
            }
        }

        new_selections.reverse();
        self.selections = new_selections;
        self.normalize_selections();
        self.version += 1;
        self.last_edit_time = Instant::now();
    }

    /// Ctrl+Delete
    pub fn delete_word_forward(&mut self) {
        if self.delete_selections() {
            return;
        }
        self.flush_pending_insert();
        self.flush_pending_delete();

        let mut sorted_selections = self.selections.clone();
        sorted_selections.sort_by_key(|s| s.range().0);
        
        // 1. Collect targets first (Immutable borrow phase)
        let mut deletion_tasks = Vec::new();
        for selection in sorted_selections.iter().rev() {
            let cursor = selection.end;
            let target = self.word_end_after_point(cursor);
            if target != cursor {
                deletion_tasks.push((cursor, target));
            }
        }

        if deletion_tasks.is_empty() { return; }

        // 2. Perform deletions (Mutable borrow phase)
        let mut new_selections = Vec::new();
        for (cursor, target) in deletion_tasks {
            let start_offset = self.buffer().point_to_offset(cursor);
            let end_offset = self.buffer().point_to_offset(target);
            let deleted_text = self.buffer().slice_bytes(start_offset.value(), end_offset.value());
            
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
            new_selections.push(Selection::cursor(cursor));
        }
        
        // Add back non-deleted cursors
        for selection in &sorted_selections {
            if self.word_end_after_point(selection.end) == selection.end {
                new_selections.push(selection.clone());
            }
        }

        new_selections.reverse();
        self.selections = new_selections;
        self.normalize_selections();
        self.version += 1;
        self.last_edit_time = Instant::now();
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
