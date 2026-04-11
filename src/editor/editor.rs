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

    pub fn selection(&self) -> Selection {
        self.selections.last().cloned().unwrap_or(Selection::cursor(Point::zero()))
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
        // Obsolete in Phase 3 - insert() now handles history immediately via execute_edits()
        self.pending_insert.clear();
        self.pending_start_cursor = None;
        self.pending_start_buffer = None;
    }

    fn flush_pending_delete(&mut self) {
        // Obsolete in Phase 3 - delete/backspace now handle history immediately via execute_edits()
        self.pending_delete.clear();
        self.pending_delete_cursor_before = None;
        self.pending_delete_start_buffer = None;
    }

    // -------------------------------------------------------------------------
    // Editing
    // -------------------------------------------------------------------------

    /// Central hub for all document edits. 
    /// Ensures perfect multi-cursor synchronization and transactional history.
    fn execute_edits(&mut self, mut edits: Vec<crate::history::transaction::RawEdit>) {
        if edits.is_empty() { return; }

        let old_buffer = self.buffer().clone();
        
        // 1. Capture original cursor offsets
        let cursor_offsets_before: Vec<usize> = self.selections.iter()
            .map(|s| self.buffer().point_to_offset(s.end).value())
            .collect();

        // 2. Sort edits by offset (ascending for shift calculation)
        edits.sort_unstable_by_key(|e| e.offset.value());

        // 3. Mathematical Cursor Shift Calculation
        // New_Offset = Old_Offset + NetChange(edits before it)
        let mut cursor_offsets_after = Vec::new();
        for &old_off in &cursor_offsets_before {
            // Check if ANY edit at this position provides a specific cursor target
            let mut target_override = None;
            for edit in &edits {
                if edit.offset.value() == old_off {
                    if let Some(target) = edit.cursor_offset {
                        target_override = Some(target);
                        break;
                    }
                }
            }

            if let Some(target) = target_override {
                cursor_offsets_after.push(target);
            } else {
                let mut total_shift: isize = 0;
                for edit in &edits {
                    // Shift the cursor if the edit happens before it.
                    // If the edit happens exactly AT the cursor, only shift if it's an insertion
                    // (this pushes the cursor forward while typing).
                    if edit.offset.value() < old_off || (edit.offset.value() == old_off && edit.old_text.is_empty()) {
                        let delta = edit.new_text.len() as isize - edit.old_text.len() as isize;
                        total_shift += delta;
                    }
                }
                cursor_offsets_after.push((old_off as isize + total_shift) as usize);
            }
        }

        // 4. Apply edits in REVERSE (highest offset first) to keep current buffer offsets valid
        let current_buf = self.history.current_mut();
        current_buf.invalidate_cache();
        for edit in edits.iter().rev() {
            let end_off = Offset(edit.offset.value() + edit.old_text.len());
            // Clear old range then insert new
            if edit.old_text.len() > 0 {
                current_buf.delete(edit.offset, end_off);
            }
            if edit.new_text.len() > 0 {
                current_buf.insert(edit.offset, &edit.new_text);
            }

            // Sync with renderer
            self.pending_edit_events.push(EditEvent {
                start_byte: edit.offset.value(),
                old_end_byte: end_off.value(),
                new_end_byte: edit.offset.value() + edit.new_text.len(),
                start_position: old_buffer.offset_to_point(edit.offset),
                old_end_position: old_buffer.offset_to_point(end_off),
                new_end_position: current_buf.offset_to_point(Offset(edit.offset.value() + edit.new_text.len())),
            });
        }
        
        let new_buffer = self.buffer().clone();

        // 5. Update selections to their new synchronized positions
        let mut new_selections = Vec::new();
        for &new_off in &cursor_offsets_after {
            let point = self.buffer().offset_to_point(Offset(new_off));
            new_selections.push(Selection::cursor(point));
        }
        self.selections = new_selections;
        self.normalize_selections();

        // 6. Push single TRANSACTION for entire batch
        let transaction = crate::history::transaction::Transaction::new(
            edits,
            cursor_offsets_before,
            cursor_offsets_after,
        );
        self.history.push(old_buffer, new_buffer, transaction);

        self.version += 1;
        self.last_edit_time = Instant::now();
    }

    pub fn insert(&mut self, text: &str) {
        self.delete_selections();
        self.flush_pending_delete();
        self.flush_pending_insert();

        let mut edits = Vec::new();
        for selection in &self.selections {
            let cursor = selection.end;
            let offset = self.buffer().point_to_offset(cursor);

            let mut cursor_override = None;
            let text_to_insert = if text == "\n" {
                let indent = self.indent_calculator.calculate_indent_with_rope(
                    self.buffer().rope(),
                    cursor.row,
                    cursor.column,
                    self.file_path.as_deref(),
                );

                // Smart Split: Detect if we are between { } or similar
                let next_char = self.buffer()
                    .slice_bytes(offset.value(), (offset.value() + 1).min(self.buffer().len()))
                    .chars()
                    .next();
                let prev_char = if offset.value() > 0 {
                    self.buffer()
                        .slice_bytes(offset.value() - 1, offset.value())
                        .chars()
                        .next()
                } else {
                    None
                };

                let is_split = (prev_char == Some('{') && next_char == Some('}')) ||
                               (prev_char == Some('[') && next_char == Some(']')) ||
                               (prev_char == Some('(') && next_char == Some(')'));

                if is_split {
                    let parent_indent = if let Some(line) = self.buffer().line(cursor.row) {
                        line.chars().take_while(|c| c.is_whitespace()).collect::<String>()
                    } else {
                        String::new()
                    };
                    let result = format!("\n{}\n{}", indent, parent_indent);
                    cursor_override = Some(offset.value() + 1 + indent.len());
                    result
                } else {
                    format!("\n{}", indent)
                }
            } else {
                text.to_string()
            };

            edits.push(crate::history::transaction::RawEdit {
                offset,
                old_text: String::new(),
                new_text: text_to_insert,
                cursor_offset: cursor_override,
            });
        }

        self.execute_edits(edits);
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

        self.flush_pending_insert();
        self.flush_pending_delete();

        let mut edits = Vec::new();
        for selection in &self.selections {
            let cursor = selection.end;
            if cursor == Point::zero() { continue; }
            
            let before = self.move_point_left(cursor);
            let s_off = self.buffer().point_to_offset(before);
            let e_off = self.buffer().point_to_offset(cursor);
            
            edits.push(crate::history::transaction::RawEdit {
                offset: s_off,
                old_text: self.buffer().slice_bytes(s_off.value(), e_off.value()),
                new_text: String::new(),
                cursor_offset: None,
            });
        }

        self.execute_edits(edits);
    }

    pub fn delete(&mut self) {
        if self.delete_selections() {
            return;
        }

        self.flush_pending_insert();
        self.flush_pending_delete();

        let mut edits = Vec::new();
        for selection in &self.selections {
            let cursor = selection.end;
            let after = self.move_point_right(cursor);
            if after == cursor { continue; }
            
            let s_off = self.buffer().point_to_offset(cursor);
            let e_off = self.buffer().point_to_offset(after);

            edits.push(crate::history::transaction::RawEdit {
                offset: s_off,
                old_text: self.buffer().slice_bytes(s_off.value(), e_off.value()),
                new_text: String::new(),
                cursor_offset: None,
            });
        }

        self.execute_edits(edits);
    }


    // -------------------------------------------------------------------------
    // Undo / Redo
    // -------------------------------------------------------------------------

    pub fn undo(&mut self) {
        self.flush_pending_insert();
        self.flush_pending_delete();
        
        if let Some(transaction) = self.history.undo() {
            // 1. Restore cursors
            let mut new_selections = Vec::new();
            for &off in &transaction.cursor_offsets_before {
                let point = self.buffer().offset_to_point(Offset(off));
                new_selections.push(Selection::cursor(point));
            }
            self.selections = new_selections;

            // 2. Notify renderer of changes
            for edit in &transaction.edits {
                let start_off = edit.offset.value();
                let old_end_off = start_off + edit.new_text.len();
                let new_end_off = start_off + edit.old_text.len();
                
                self.pending_edit_events.push(EditEvent {
                    start_byte: start_off,
                    old_end_byte: old_end_off,
                    new_end_byte: new_end_off,
                    start_position: self.buffer().offset_to_point(Offset(start_off)),
                    old_end_position: self.buffer().offset_to_point(Offset(old_end_off)),
                    new_end_position: self.buffer().offset_to_point(Offset(new_end_off)),
                });
            }
            self.version += 1;
        }
    }

    pub fn redo(&mut self) {
        self.flush_pending_insert();
        self.flush_pending_delete();
        
        if let Some(transaction) = self.history.redo() {
            // 1. Restore cursors
            let mut new_selections = Vec::new();
            for &off in &transaction.cursor_offsets_after {
                let point = self.buffer().offset_to_point(Offset(off));
                new_selections.push(Selection::cursor(point));
            }
            self.selections = new_selections;

            // 2. Notify renderer
            for edit in &transaction.edits {
                let start_off = edit.offset.value();
                let old_end_off = start_off + edit.old_text.len();
                let new_end_off = start_off + edit.new_text.len();
                
                self.pending_edit_events.push(EditEvent {
                    start_byte: start_off,
                    old_end_byte: old_end_off,
                    new_end_byte: new_end_off,
                    start_position: self.buffer().offset_to_point(Offset(start_off)),
                    old_end_position: self.buffer().offset_to_point(Offset(old_end_off)),
                    new_end_position: self.buffer().offset_to_point(Offset(new_end_off)),
                });
            }
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

    pub fn select_next_occurrence(&mut self) {
        self.flush_pending_insert();
        self.flush_pending_delete();

        if self.selections.is_empty() {
            return;
        }

        // 1. Determine the query from the primary (last) selection
        let primary_idx = self.selections.len() - 1;
        let primary = self.selections[primary_idx].clone();
        
        let query = if primary.is_empty() {
            // Case A: Select current word
            let (start, end) = self.word_range_at(primary.end);
            if start == end { return; }
            self.selections[primary_idx] = Selection::new(start, end);
            return;
        } else {
            // Case B: Extract text of current selection
            let (start_p, end_p) = primary.range();
            let start_off = self.buffer().point_to_offset(start_p);
            let end_off = self.buffer().point_to_offset(end_p);
            self.buffer().slice_bytes(start_off.value(), end_off.value())
        };

        if query.is_empty() { return; }

        // 2. Search for next match
        let content = self.buffer().to_string();
        let last_sel_end = self.buffer().point_to_offset(self.selections.last().unwrap().end);
        
        let next_pos = if let Some(pos) = content[last_sel_end.value()..].find(&query) {
            Some(last_sel_end.value() + pos)
        } else if let Some(pos) = content[..last_sel_end.value()].find(&query) {
            // Wrap around
            Some(pos)
        } else {
            None
        };

        // 3. Add new selection if found and NOT overlapping
        if let Some(start_idx) = next_pos {
            let start = self.buffer().offset_to_point(Offset(start_idx));
            let end = self.buffer().offset_to_point(Offset(start_idx + query.len()));
            let new_sel = Selection::new(start, end);
            
            // Critical: Check for overlap with ANY existing selection
            let (new_s, new_e) = new_sel.range();
            let has_overlap = self.selections.iter().any(|s| {
                let (os, oe) = s.range();
                // Overlap if new_start inside existing, or new_end inside existing
                (new_s >= os && new_s < oe) || (new_e > os && new_e <= oe) || (os >= new_s && os < new_e)
            });

            if !has_overlap {
                self.selections.push(new_sel);
            }
        }
        
        self.normalize_selections();
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
        self.flush_pending_insert();
        self.flush_pending_delete();

        if self.selections.iter().all(|s| s.is_empty()) {
            return false;
        }

        let mut edits = Vec::new();
        for selection in &self.selections {
            if !selection.is_empty() {
                let (start, end) = selection.range();
                let s_off = self.buffer().point_to_offset(start);
                let e_off = self.buffer().point_to_offset(end);
                
                edits.push(crate::history::transaction::RawEdit {
                    offset: s_off,
                    old_text: self.buffer().slice_bytes(s_off.value(), e_off.value()),
                    new_text: String::new(),
                    cursor_offset: None,
                });
            }
        }

        if !edits.is_empty() {
            self.execute_edits(edits);
            true
        } else {
            false
        }
    }

    /// Delete entire current line (Ctrl+Shift+K) for all cursors.
    pub fn delete_line(&mut self) {
        self.flush_pending_insert();
        self.flush_pending_delete();
        
        let mut rows: Vec<usize> = self.selections.iter().map(|s| s.end.row).collect();
        rows.sort_unstable();
        rows.dedup();
        
        let mut edits = Vec::new();
        for &row in &rows {
            let start = Point::new(row, 0);
            let end = if row + 1 < self.buffer().line_count() {
                Point::new(row + 1, 0)
            } else {
                let last_col = self.buffer().line(row).map(|l| l.len()).unwrap_or(0);
                Point::new(row, last_col)
            };
            
            let s_off = self.buffer().point_to_offset(start);
            let e_off = self.buffer().point_to_offset(end);
            
            edits.push(crate::history::transaction::RawEdit {
                offset: s_off,
                old_text: self.buffer().slice_bytes(s_off.value(), e_off.value()),
                new_text: String::new(),
                cursor_offset: None,
            });
        }

        self.execute_edits(edits);
    }

    /// Ctrl+Backspace
    pub fn delete_word_backward(&mut self) {
        if self.delete_selections() {
            return;
        }
        self.flush_pending_insert();
        self.flush_pending_delete();
        
        let mut edits = Vec::new();
        for selection in &self.selections {
            let cursor = selection.end;
            let before = self.word_start_before_point(cursor);
            if before == cursor { continue; }
            
            let s_off = self.buffer().point_to_offset(before);
            let e_off = self.buffer().point_to_offset(cursor);
            
            edits.push(crate::history::transaction::RawEdit {
                offset: s_off,
                old_text: self.buffer().slice_bytes(s_off.value(), e_off.value()),
                new_text: String::new(),
                cursor_offset: None,
            });
        }
        
        self.execute_edits(edits);
    }

    /// Ctrl+Delete
    pub fn delete_word_forward(&mut self) {
        if self.delete_selections() {
            return;
        }
        self.flush_pending_insert();
        self.flush_pending_delete();

        let mut edits = Vec::new();
        for selection in &self.selections {
            let cursor = selection.end;
            let after = self.word_end_after_point(cursor);
            if after == cursor { continue; }
            
            let s_off = self.buffer().point_to_offset(cursor);
            let e_off = self.buffer().point_to_offset(after);

            edits.push(crate::history::transaction::RawEdit {
                offset: s_off,
                old_text: self.buffer().slice_bytes(s_off.value(), e_off.value()),
                new_text: String::new(),
                cursor_offset: None,
            });
        }

        self.execute_edits(edits);
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

    pub fn replace_all(&mut self, new_text: &str) {
        self.flush_pending_insert();
        self.flush_pending_delete();
        
        // Treat as one giant edit from Start to End of document
        let edit = crate::history::transaction::RawEdit {
            offset: Offset(0),
            old_text: self.buffer().to_string(),
            new_text: new_text.to_string(),
            cursor_offset: None,
        };
        
        self.execute_edits(vec![edit]);
        
        // Ensure cursor is at the end or sensible position
        let last_row = self.buffer().line_count().saturating_sub(1);
        let last_col = self.buffer().line(last_row).map(|l| l.len()).unwrap_or(0);
        self.set_cursor(Point::new(last_row, last_col));
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
