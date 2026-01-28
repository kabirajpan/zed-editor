use crate::syntax::{HighlightSpan, SyntaxHighlighter};
use egui::{Color32, FontId, Pos2, Rect, Vec2};
use std::collections::HashMap;

/// Cached line with version tracking
#[derive(Clone)]
struct CachedLine {
    content: String,
    version: u64,
    width: f32,
}

impl CachedLine {
    fn new(content: String, version: u64) -> Self {
        Self {
            content,
            version,
            width: 0.0,
        }
    }

    fn is_valid(&self, current_version: u64) -> bool {
        self.version == current_version
    }
}

/// Fast viewport renderer with caching
pub struct ViewportRenderer {
    line_cache: HashMap<usize, CachedLine>,
    width_cache: HashMap<String, f32>,
    last_version: u64,
    frame_count: u64,
}

impl ViewportRenderer {
    pub fn new() -> Self {
        Self {
            line_cache: HashMap::new(),
            width_cache: HashMap::new(),
            last_version: 0,
            frame_count: 0,
        }
    }

    /// Get or cache a line
    fn get_line_cached(
        &mut self,
        editor: &crate::Editor,
        line_idx: usize,
        current_version: u64,
    ) -> String {
        // Check cache validity
        if let Some(cached) = self.line_cache.get(&line_idx) {
            if cached.is_valid(current_version) {
                return cached.content.clone();
            }
        }

        // Cache miss - fetch from editor
        let content = editor.buffer().line(line_idx).unwrap_or_default();

        // Cache it
        if self.line_cache.len() < 500 {
            self.line_cache
                .insert(line_idx, CachedLine::new(content.clone(), current_version));
        }

        content
    }

    /// Measure text width with caching
    fn measure_width(&mut self, ui: &egui::Ui, text: &str, font_id: &FontId) -> f32 {
        if text.is_empty() {
            return 0.0;
        }

        // Check cache
        if let Some(&width) = self.width_cache.get(text) {
            return width;
        }

        // Measure
        let width = ui
            .painter()
            .layout_no_wrap(text.to_string(), font_id.clone(), Color32::WHITE)
            .rect
            .width();

        // Cache it (limit cache size)
        if self.width_cache.len() < 200 {
            self.width_cache.insert(text.to_string(), width);
        }

        width
    }

    /// Invalidate cache on edit
    pub fn invalidate_from_line(&mut self, start_line: usize) {
        self.line_cache.retain(|&line, _| line < start_line);
        self.width_cache.clear();
    }

    /// Invalidate specific line
    pub fn invalidate_line(&mut self, line: usize) {
        self.line_cache.remove(&line);
    }

    /// Render viewport with syntax highlighting
    pub fn render_with_highlighting(
        &mut self,
        ui: &mut egui::Ui,
        editor: &crate::Editor,
        highlighter: &mut SyntaxHighlighter,
        cursor_blink: bool,
        should_auto_scroll: bool,
    ) {
        self.frame_count += 1;

        let cursor = editor.cursor();
        let current_version = editor.version();
        let font_id = FontId::monospace(14.0);
        let line_height = ui.fonts(|f| f.row_height(&font_id)) + 4.0;
        let cursor_y = cursor.row as f32 * line_height;

        // Update version tracking
        self.last_version = current_version;

        // Cleanup every 60 frames
        if self.frame_count % 60 == 0 {
            if self.line_cache.len() > 500 {
                self.line_cache.clear();
            }
            if self.width_cache.len() > 200 {
                self.width_cache.clear();
            }
        }

        let full_text = editor.text();
        let file_path = editor.file_path();

        egui::ScrollArea::both()
            .auto_shrink([false, false])
            .show_viewport(ui, |ui, viewport| {
                let total_lines = editor.line_count().max(1);
                let content_height = total_lines as f32 * line_height;

                // Virtual scrolling
                let visible_start = (viewport.min.y / line_height).floor().max(0.0) as usize;
                let visible_end =
                    ((viewport.max.y / line_height).ceil() as usize + 1).min(total_lines);

                let (response, painter) = ui.allocate_painter(
                    Vec2::new(ui.available_width(), content_height),
                    egui::Sense::click(),
                );

                let line_number_width = 60.0;
                let text_start_x = response.rect.min.x + line_number_width;

                // Render visible lines
                for row in visible_start..visible_end {
                    let y = response.rect.min.y + row as f32 * line_height;

                    // Get cached line
                    let line = self.get_line_cached(editor, row, current_version);

                    // Line number
                    let line_num = format!("{:4}", row + 1);
                    painter.text(
                        Pos2::new(response.rect.min.x + 10.0, y),
                        egui::Align2::LEFT_TOP,
                        line_num,
                        font_id.clone(),
                        Color32::from_rgb(100, 100, 100),
                    );

                    // Get syntax highlights for this line
                    let highlights = highlighter.highlight_line(&full_text, row, file_path);
                    // Render line content
                    if row == cursor.row {
                        // Cursor line - need to handle cursor position
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
                        // Regular line with highlighting
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

                // Auto-scroll with margin (keep 1 line visible below cursor)
                if should_auto_scroll {
                    let scroll_margin = line_height; // 1 line margin
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

    /// Render a line with syntax highlighting
    fn render_highlighted_line(
        &self,
        painter: &egui::Painter,
        line: &str,
        x: f32,
        y: f32,
        font_id: &FontId,
        highlights: &[HighlightSpan],
    ) {
        if highlights.is_empty() {
            // No highlighting - render as plain text
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

        for highlight in highlights {
            // Render unhighlighted text before this span
            if last_end < highlight.start {
                let text: String = chars[last_end..highlight.start].iter().collect();
                if !text.is_empty() {
                    let galley =
                        painter.layout_no_wrap(text.clone(), font_id.clone(), Color32::WHITE);
                    painter.galley(Pos2::new(current_x, y), galley.clone(), Color32::WHITE);
                    current_x += galley.rect.width();
                }
            }

            // Render highlighted span
            let span_end = highlight.end.min(chars.len());
            let text: String = chars[highlight.start..span_end].iter().collect();
            if !text.is_empty() {
                let galley = painter.layout_no_wrap(text.clone(), font_id.clone(), highlight.color);
                painter.galley(Pos2::new(current_x, y), galley.clone(), highlight.color);
                current_x += galley.rect.width();
            }

            last_end = span_end;
        }

        // Render any remaining unhighlighted text
        if last_end < chars.len() {
            let text: String = chars[last_end..].iter().collect();
            if !text.is_empty() {
                let galley = painter.layout_no_wrap(text.clone(), font_id.clone(), Color32::WHITE);
                painter.galley(Pos2::new(current_x, y), galley.clone(), Color32::WHITE);
            }
        }
    }

    /// Render line with cursor and highlighting
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
        highlights: &[HighlightSpan],
    ) {
        if line.is_empty() {
            // Empty line - just show cursor
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

        // Helper to get color for a character position
        let get_color_at = |pos: usize| -> Color32 {
            for highlight in highlights {
                if pos >= highlight.start && pos < highlight.end {
                    return highlight.color;
                }
            }
            Color32::WHITE
        };

        if highlights.is_empty() {
            // No highlighting - simpler rendering
            let before_cursor: String = chars.iter().take(cursor_pos).collect();
            let at_cursor = chars.get(cursor_pos).copied();
            let after_cursor: String = chars.iter().skip(cursor_pos + 1).collect();

            let before_width = self.measure_width(ui, &before_cursor, font_id);
            let cursor_x = x + before_width;

            if !before_cursor.is_empty() {
                painter.text(
                    Pos2::new(x, y),
                    egui::Align2::LEFT_TOP,
                    before_cursor,
                    font_id.clone(),
                    Color32::WHITE,
                );
            }

            // Render cursor
            if cursor_blink {
                painter.rect_filled(
                    Rect::from_min_size(Pos2::new(cursor_x, y), Vec2::new(2.0, line_height)),
                    0.0,
                    Color32::WHITE,
                );
            }

            let mut after_x = cursor_x;
            if let Some(ch) = at_cursor {
                let char_width = self.measure_width(ui, &ch.to_string(), font_id);
                if !cursor_blink {
                    painter.text(
                        Pos2::new(cursor_x, y),
                        egui::Align2::LEFT_TOP,
                        ch.to_string(),
                        font_id.clone(),
                        Color32::WHITE,
                    );
                }
                after_x += char_width;
            }

            if !after_cursor.is_empty() {
                painter.text(
                    Pos2::new(after_x, y),
                    egui::Align2::LEFT_TOP,
                    after_cursor,
                    font_id.clone(),
                    Color32::WHITE,
                );
            }
        } else {
            // With highlighting - render character by character
            let mut current_x = x;
            let mut cursor_x = x;
            let mut found_cursor = false;

            for (i, ch) in chars.iter().enumerate() {
                if i == cursor_pos && !found_cursor {
                    cursor_x = current_x;
                    found_cursor = true;

                    // Render cursor
                    if cursor_blink {
                        painter.rect_filled(
                            Rect::from_min_size(
                                Pos2::new(cursor_x, y),
                                Vec2::new(2.0, line_height),
                            ),
                            0.0,
                            Color32::WHITE,
                        );
                    }
                }

                let color = get_color_at(i);
                let ch_str = ch.to_string();
                let galley = painter.layout_no_wrap(ch_str.clone(), font_id.clone(), color);

                if i != cursor_pos || !cursor_blink {
                    painter.galley(Pos2::new(current_x, y), galley.clone(), color);
                }

                current_x += galley.rect.width();
            }

            // Cursor at end of line
            if cursor_pos >= chars.len() && !found_cursor {
                cursor_x = current_x;
                if cursor_blink {
                    painter.rect_filled(
                        Rect::from_min_size(Pos2::new(cursor_x, y), Vec2::new(2.0, line_height)),
                        0.0,
                        Color32::WHITE,
                    );
                }
            }
        }
    }

    /// Legacy render method without highlighting (for backward compatibility)
    pub fn render(
        &mut self,
        ui: &mut egui::Ui,
        editor: &crate::Editor,
        cursor_blink: bool,
        should_auto_scroll: bool,
    ) {
        let mut highlighter = SyntaxHighlighter::new(crate::syntax::SyntaxTheme::dark());
        self.render_with_highlighting(
            ui,
            editor,
            &mut highlighter,
            cursor_blink,
            should_auto_scroll,
        );
    }
}

impl Default for ViewportRenderer {
    fn default() -> Self {
        Self::new()
    }
}
