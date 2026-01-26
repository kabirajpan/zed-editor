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

    /// Render viewport with optimizations
    pub fn render(
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

                    // Render line content
                    if row == cursor.row {
                        // Cursor line - need to handle cursor position
                        self.render_cursor_line(
                            &painter,
                            ui,
                            &line,
                            cursor.column,
                            cursor_blink,
                            text_start_x,
                            y,
                            line_height,
                            &font_id,
                        );
                    } else if !line.is_empty() {
                        // Regular line - single paint call
                        painter.text(
                            Pos2::new(text_start_x, y),
                            egui::Align2::LEFT_TOP,
                            line,
                            font_id.clone(),
                            Color32::WHITE,
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

    /// Render line with cursor (optimized)
    fn render_cursor_line(
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

        // Split line at cursor position
        let chars: Vec<char> = line.chars().collect();
        let cursor_pos = cursor_col.min(chars.len());

        let before_cursor: String = chars.iter().take(cursor_pos).collect();
        let at_cursor = chars.get(cursor_pos).copied();
        let after_cursor: String = chars.iter().skip(cursor_pos + 1).collect();

        // Measure text before cursor
        let before_width = self.measure_width(ui, &before_cursor, font_id);
        let cursor_x = x + before_width;

        // Render text before cursor
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
            let cursor_char = at_cursor.unwrap_or(' ');
            let cursor_str = cursor_char.to_string();
            let cursor_width = 2.0;
            painter.rect_filled(
                Rect::from_min_size(Pos2::new(cursor_x, y), Vec2::new(cursor_width, line_height)),
                0.0,
                Color32::WHITE,
            );

            // Render character on cursor (inverted color)
            if at_cursor.is_some() {
                painter.text(
                    Pos2::new(cursor_x, y),
                    egui::Align2::LEFT_TOP,
                    cursor_str,
                    font_id.clone(),
                    Color32::WHITE,
                );
            }
        } else if let Some(ch) = at_cursor {
            // Cursor not blinking - render character normally
            painter.text(
                Pos2::new(cursor_x, y),
                egui::Align2::LEFT_TOP,
                ch.to_string(),
                font_id.clone(),
                Color32::WHITE,
            );
        }

        // Render text after cursor
        if !after_cursor.is_empty() {
            let at_width = if let Some(ch) = at_cursor {
                self.measure_width(ui, &ch.to_string(), font_id)
            } else {
                0.0
            };
            painter.text(
                Pos2::new(cursor_x + at_width, y),
                egui::Align2::LEFT_TOP,
                after_cursor,
                font_id.clone(),
                Color32::WHITE,
            );
        }
    }
}

impl Default for ViewportRenderer {
    fn default() -> Self {
        Self::new()
    }
}
