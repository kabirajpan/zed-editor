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

/// Cached highlights for a line
#[derive(Clone)]
struct CachedHighlights {
    highlights: Vec<HighlightSpan>,
    version: u64,
}

/// Fast viewport renderer with caching
pub struct ViewportRenderer {
    line_cache: HashMap<usize, CachedLine>,
    width_cache: HashMap<String, f32>,
    highlight_cache: HashMap<usize, CachedHighlights>,
    last_version: u64,
    frame_count: u64,
}

impl ViewportRenderer {
    pub fn new() -> Self {
        Self {
            line_cache: HashMap::new(),
            width_cache: HashMap::new(),
            highlight_cache: HashMap::new(),
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

    /// Measure text width with caching
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

    /// Invalidate cache on edit
    pub fn invalidate_from_line(&mut self, start_line: usize) {
        self.line_cache.retain(|&line, _| line < start_line);
        self.highlight_cache.retain(|&line, _| line < start_line);
        self.width_cache.clear();
    }

    /// Invalidate specific line
    pub fn invalidate_line(&mut self, line: usize) {
        self.line_cache.remove(&line);
        self.highlight_cache.remove(&line);
    }

    /// 🚀 PRODUCTION-FIXED: Render viewport with syntax highlighting
    /// This version uses Rope directly without converting entire file to String
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

        // Clear caches if version changed
        if self.last_version != current_version {
            self.highlight_cache.clear();
            self.last_version = current_version;
        }

        // Cleanup every 60 frames
        if self.frame_count % 60 == 0 {
            if self.line_cache.len() > 500 {
                self.line_cache.clear();
            }
            if self.width_cache.len() > 200 {
                self.width_cache.clear();
            }
            if self.highlight_cache.len() > 500 {
                self.highlight_cache.clear();
            }
        }

        // 🚀 CRITICAL FIX: Get Rope reference instead of converting to String!
        // OLD CODE: let full_text = editor.text(); // ❌ This converted entire file!
        // NEW CODE: Use rope directly
        let rope = editor.buffer().rope();
        let file_path = editor.file_path();

        egui::ScrollArea::both()
            .auto_shrink([false, false])
            .show_viewport(ui, |ui, viewport| {
                let total_lines = editor.line_count().max(1);
                let content_height = total_lines as f32 * line_height;

                let visible_start = (viewport.min.y / line_height).floor().max(0.0) as usize;
                let visible_end =
                    ((viewport.max.y / line_height).ceil() as usize + 1).min(total_lines);

                let (response, painter) = ui.allocate_painter(
                    Vec2::new(ui.available_width(), content_height),
                    egui::Sense::click(),
                );

                let line_number_width = 60.0;
                let text_start_x = response.rect.min.x + line_number_width;

                // Render visible lines only
                for row in visible_start..visible_end {
                    let y = response.rect.min.y + row as f32 * line_height;

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

                    // 🚀 Get CACHED syntax highlights - now uses Rope directly!
                    let highlights = self.get_highlights_cached_with_rope(
                        highlighter,
                        rope,
                        row,
                        file_path,
                        current_version,
                    );

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

    /// 🚀 NEW METHOD: Get highlights using Rope (efficient, no full text conversion)
    fn get_highlights_cached_with_rope(
        &mut self,
        highlighter: &mut SyntaxHighlighter,
        rope: &crate::rope::Rope,
        line_idx: usize,
        file_path: Option<&std::path::Path>,
        current_version: u64,
    ) -> Vec<HighlightSpan> {
        // Check cache first
        if let Some(cached) = self.highlight_cache.get(&line_idx) {
            if cached.version == current_version {
                return cached.highlights.clone();
            }
        }

        // Cache miss - generate highlights using Rope (not String!)
        let highlights = highlighter.highlight_line(rope, line_idx, file_path);

        // Cache the results
        if self.highlight_cache.len() < 500 {
            self.highlight_cache.insert(
                line_idx,
                CachedHighlights {
                    highlights: highlights.clone(),
                    version: current_version,
                },
            );
        }

        highlights
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

        // Render the line normally with highlighting
        let mut current_x = x;
        let mut cursor_x = x;
        let mut last_end = 0;

        // Calculate cursor X position first
        if cursor_pos > 0 {
            let before_cursor: String = chars[..cursor_pos].iter().collect();
            cursor_x = x + self.measure_width(ui, &before_cursor, font_id);
        }

        if highlights.is_empty() {
            // No highlighting - render as chunks
            painter.text(
                Pos2::new(x, y),
                egui::Align2::LEFT_TOP,
                line,
                font_id.clone(),
                Color32::WHITE,
            );
        } else {
            // With highlighting - render in colored chunks
            for highlight in highlights {
                // Render unhighlighted text before this span
                if last_end < highlight.start {
                    let text: String = chars[last_end..highlight.start].iter().collect();
                    if !text.is_empty() {
                        let galley = painter.layout_no_wrap(text, font_id.clone(), Color32::WHITE);
                        painter.galley(Pos2::new(current_x, y), galley.clone(), Color32::WHITE);
                        current_x += galley.rect.width();
                    }
                }

                // Render highlighted span
                let span_end = highlight.end.min(chars.len());
                let text: String = chars[highlight.start..span_end].iter().collect();
                if !text.is_empty() {
                    let galley = painter.layout_no_wrap(text, font_id.clone(), highlight.color);
                    painter.galley(Pos2::new(current_x, y), galley.clone(), highlight.color);
                    current_x += galley.rect.width();
                }

                last_end = span_end;
            }

            // Render remaining unhighlighted text
            if last_end < chars.len() {
                let text: String = chars[last_end..].iter().collect();
                if !text.is_empty() {
                    let galley = painter.layout_no_wrap(text, font_id.clone(), Color32::WHITE);
                    painter.galley(Pos2::new(current_x, y), galley.clone(), Color32::WHITE);
                }
            }
        }

        // Draw cursor on top
        if cursor_blink {
            painter.rect_filled(
                Rect::from_min_size(Pos2::new(cursor_x, y), Vec2::new(2.0, line_height)),
                0.0,
                Color32::WHITE,
            );
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
