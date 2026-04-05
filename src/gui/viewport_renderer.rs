use crate::syntax::{SyntaxHighlighter, SyntaxTheme};
use egui::{Color32, FontId, Pos2, Rect, Vec2};
use std::collections::HashMap;

#[derive(Clone)]
struct CachedLine {
    content: String,
    version: u64,
}

impl CachedLine {
    fn new(content: String, version: u64) -> Self {
        Self { content, version }
    }

    fn is_valid(&self, current_version: u64) -> bool {
        self.version == current_version
    }
}

pub struct ViewportRenderer {
    line_cache: HashMap<usize, CachedLine>,
    width_cache: HashMap<String, f32>,
    last_version: u64,
    frame_count: u64,
    highlighter: SyntaxHighlighter,
    last_viewport: (usize, usize),
}

impl ViewportRenderer {
    pub fn new() -> Self {
        Self {
            line_cache: HashMap::new(),
            width_cache: HashMap::new(),
            last_version: 0,
            frame_count: 0,
            highlighter: SyntaxHighlighter::new(SyntaxTheme::dark()),
            last_viewport: (0, 0),
        }
    }

    /// Forward an incremental rope edit to the highlighter so tree-sitter can
    /// reparse incrementally.  Call this once per EditEvent drained from the
    /// Editor, before calling render_with_highlighting.
    pub fn notify_edit(
        &mut self,
        rope: &crate::rope::Rope,
        start_byte: usize,
        old_end_byte: usize,
        new_end_byte: usize,
    ) {
        self.highlighter
            .notify_edit(rope, start_byte, old_end_byte, new_end_byte);
    }

    /// Full reset: discard the parse tree and all caches.  Use this after
    /// undo/redo/load/format — any operation that jumps the buffer to an
    /// arbitrary state rather than applying an incremental edit.
    pub fn full_reset(&mut self) {
        self.line_cache.clear();
        self.width_cache.clear();
        self.highlighter.reset();
    }

    /// Partial invalidation: discard line cache from `start_line` onward and
    /// invalidate the highlight cache.  The parse tree is kept so the next
    /// highlight pass can still reuse it (incremental reparse).
    pub fn invalidate_from_line(&mut self, start_line: usize) {
        self.line_cache.retain(|&line, _| line < start_line);
        self.width_cache.clear();
        self.highlighter.invalidate();
    }

    pub fn invalidate_line(&mut self, line: usize) {
        self.line_cache.remove(&line);
        self.highlighter.invalidate();
    }

    fn get_line_cached(
        &mut self,
        editor: &crate::Editor,
        line_idx: usize,
        current_version: u64,
    ) -> String {
        if let Some(cached) = self.line_cache.get(&line_idx) {
            if cached.is_valid(current_version) {
                return cached.content.clone();
            }
        }
        let content = editor.buffer().line(line_idx).unwrap_or_default();
        if self.line_cache.len() < 500 {
            self.line_cache
                .insert(line_idx, CachedLine::new(content.clone(), current_version));
        }
        content
    }

    fn measure_width(&mut self, ui: &egui::Ui, text: &str, font_id: &FontId) -> f32 {
        if text.is_empty() {
            return 0.0;
        }
        if let Some(&width) = self.width_cache.get(text) {
            return width;
        }
        let width = ui
            .painter()
            .layout_no_wrap(text.to_string(), font_id.clone(), Color32::WHITE)
            .rect
            .width();
        if self.width_cache.len() < 200 {
            self.width_cache.insert(text.to_string(), width);
        }
        width
    }

    pub fn render_with_highlighting(
        &mut self,
        ui: &mut egui::Ui,
        editor: &crate::Editor,
        cursor_blink: bool,
        should_auto_scroll: bool,
    ) {
        self.frame_count += 1;

        let cursor = editor.cursor();
        let current_version = editor.version();
        let font_id = FontId::monospace(14.0);
        let line_height = ui.fonts(|f| f.row_height(&font_id)) + 4.0;
        let cursor_y = cursor.row as f32 * line_height;

        if self.last_version != current_version {
            self.last_version = current_version;
        }

        if self.frame_count % 60 == 0 {
            if self.line_cache.len() > 500 {
                self.line_cache.clear();
            }
            if self.width_cache.len() > 200 {
                self.width_cache.clear();
            }
        }

        let file_path = editor.file_path().map(|p| p.to_path_buf());

        egui::ScrollArea::both()
            .auto_shrink([false, false])
            .show_viewport(ui, |ui, viewport| {
                let total_lines = editor.line_count().max(1);
                let content_height = total_lines as f32 * line_height;

                let visible_start = (viewport.min.y / line_height).floor().max(0.0) as usize;
                let visible_end =
                    ((viewport.max.y / line_height).ceil() as usize + 1).min(total_lines);

                self.last_viewport = (visible_start, visible_end);

                let (response, painter) = ui.allocate_painter(
                    Vec2::new(ui.available_width(), content_height),
                    egui::Sense::click(),
                );

                let line_number_width = 60.0;
                let text_start_x = response.rect.min.x + line_number_width;
                let rope = editor.buffer().rope();

                let highlights_map = self.highlighter.highlight_viewport(
                    rope,
                    current_version,
                    file_path.as_deref(),
                    visible_start,
                    visible_end,
                );

                for row in visible_start..visible_end {
                    let y = response.rect.min.y + row as f32 * line_height;
                    let line = self.get_line_cached(editor, row, current_version);

                    painter.text(
                        Pos2::new(response.rect.min.x + 10.0, y),
                        egui::Align2::LEFT_TOP,
                        format!("{:4}", row + 1),
                        font_id.clone(),
                        Color32::from_rgb(100, 100, 100),
                    );

                    let highlights = highlights_map.get(&row).cloned().unwrap_or_default();

                    if row == cursor.row {
                        self.render_cursor_line_highlighted(
                            &painter,
                            ui,
                            &line,
                            cursor.column,
                            cursor_blink,
                            text_start_x,
                            y,
                            line_height,
                            &font_id,
                            &highlights,
                        );
                    } else if !line.is_empty() {
                        self.render_highlighted_line(
                            &painter,
                            &line,
                            text_start_x,
                            y,
                            &font_id,
                            &highlights,
                        );
                    }
                }

                if should_auto_scroll {
                    let scroll_margin = line_height;
                    let cursor_rect = Rect::from_min_size(
                        Pos2::new(
                            response.rect.min.x,
                            response.rect.min.y + cursor_y - scroll_margin,
                        ),
                        Vec2::new(response.rect.width(), line_height + (scroll_margin * 2.0)),
                    );
                    ui.scroll_to_rect(cursor_rect, None);
                }
            });
    }

    fn render_highlighted_line(
        &self,
        painter: &egui::Painter,
        line: &str,
        x: f32,
        y: f32,
        font_id: &FontId,
        highlights: &[(usize, usize, Color32)],
    ) {
        if highlights.is_empty() {
            painter.text(
                Pos2::new(x, y),
                egui::Align2::LEFT_TOP,
                line,
                font_id.clone(),
                Color32::WHITE,
            );
            return;
        }

        let chars: Vec<char> = line.chars().collect();
        let mut current_x = x;
        let mut last_end = 0;

        for &(start, end, color) in highlights {
            if last_end < start {
                let text: String = chars[last_end..start.min(chars.len())].iter().collect();
                if !text.is_empty() {
                    let galley = painter.layout_no_wrap(text, font_id.clone(), Color32::WHITE);
                    painter.galley(Pos2::new(current_x, y), galley.clone(), Color32::WHITE);
                    current_x += galley.rect.width();
                }
            }
            let span_end = end.min(chars.len());
            if start < span_end {
                let text: String = chars[start..span_end].iter().collect();
                if !text.is_empty() {
                    let galley = painter.layout_no_wrap(text, font_id.clone(), color);
                    painter.galley(Pos2::new(current_x, y), galley.clone(), color);
                    current_x += galley.rect.width();
                }
            }
            last_end = span_end;
        }

        if last_end < chars.len() {
            let text: String = chars[last_end..].iter().collect();
            if !text.is_empty() {
                let galley = painter.layout_no_wrap(text, font_id.clone(), Color32::WHITE);
                painter.galley(Pos2::new(current_x, y), galley.clone(), Color32::WHITE);
            }
        }
    }

    fn render_cursor_line_highlighted(
        &mut self,
        painter: &egui::Painter,
        ui: &egui::Ui,
        line: &str,
        cursor_col: usize,
        cursor_blink: bool,
        x: f32,
        y: f32,
        line_height: f32,
        font_id: &FontId,
        highlights: &[(usize, usize, Color32)],
    ) {
        if line.is_empty() {
            if cursor_blink {
                painter.rect_filled(
                    Rect::from_min_size(Pos2::new(x, y), Vec2::new(2.0, line_height)),
                    0.0,
                    Color32::WHITE,
                );
            }
            return;
        }

        let chars: Vec<char> = line.chars().collect();
        let cursor_pos = cursor_col.min(chars.len());

        let cursor_x = if cursor_pos > 0 {
            let before_cursor: String = chars[..cursor_pos].iter().collect();
            x + self.measure_width(ui, &before_cursor, font_id)
        } else {
            x
        };

        self.render_highlighted_line(painter, line, x, y, font_id, highlights);

        if cursor_blink {
            let cursor_height = line_height * 0.85;
            let cursor_y_offset = (line_height - cursor_height) / 2.0;
            painter.rect_filled(
                Rect::from_min_size(
                    Pos2::new(cursor_x, y + cursor_y_offset),
                    Vec2::new(2.0, cursor_height),
                ),
                0.0,
                Color32::WHITE,
            );
        }
    }

    pub fn render(
        &mut self,
        ui: &mut egui::Ui,
        editor: &crate::Editor,
        cursor_blink: bool,
        should_auto_scroll: bool,
    ) {
        self.render_with_highlighting(ui, editor, cursor_blink, should_auto_scroll);
    }
}

impl Default for ViewportRenderer {
    fn default() -> Self {
        Self::new()
    }
}
