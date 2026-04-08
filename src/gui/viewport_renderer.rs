use crate::buffer::Point;
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

/// Returned by render_with_highlighting so app.rs can handle clicks and drags
/// without reaching into renderer internals.
pub struct RenderInteraction {
    /// Screen position of a single click (not double)
    pub single_clicked_at: Option<Pos2>,
    /// Screen position of a double click
    pub double_clicked_at: Option<Pos2>,
    /// Current pointer position while dragging
    pub dragging_at: Option<Pos2>,
    /// True on the frame the drag started
    pub drag_started: bool,
}

pub struct ViewportRenderer {
    line_cache: HashMap<usize, CachedLine>,
    width_cache: HashMap<String, f32>,
    last_version: u64,
    frame_count: u64,
    pub highlighter: SyntaxHighlighter,
    last_viewport: (usize, usize),

    // Layout state stored after each render — used by screen_to_point
    content_rect: Rect,
    line_height: f32,
    char_width: f32,        // monospace char width, measured once
    line_number_width: f32, // always 60.0
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
            content_rect: Rect::ZERO,
            line_height: 18.0,
            char_width: 8.4,
            line_number_width: 60.0,
        }
    }

    pub fn notify_edit(
        &mut self,
        rope: &crate::rope::Rope,
        event: &crate::editor::EditEvent,
    ) {
        self.highlighter.notify_edit(rope, event);
    }

    pub fn full_reset(&mut self) {
        self.line_cache.clear();
        self.width_cache.clear();
        self.highlighter.reset();
    }

    pub fn invalidate_from_line(&mut self, start_line: usize) {
        self.line_cache.retain(|&line, _| line < start_line);
        self.width_cache.clear();
        self.highlighter.invalidate();
    }

    pub fn invalidate_line(&mut self, line: usize) {
        self.line_cache.remove(&line);
        self.highlighter.invalidate();
    }

    /// Convert a screen position (from pointer events) to a buffer Point.
    /// Uses the layout stored during the last render call.
    pub fn screen_to_point(&self, screen_pos: Pos2, editor: &crate::Editor) -> Point {
        let rel_y = screen_pos.y - self.content_rect.min.y;
        let row = (rel_y / self.line_height).floor() as usize;
        let row = row.min(editor.line_count().saturating_sub(1));

        let text_x = (screen_pos.x - (self.content_rect.min.x + self.line_number_width)).max(0.0);
        let line = editor.buffer().line(row).unwrap_or_default();
        let char_count = line.chars().count();

        // Use stored char_width (monospace — all chars same width)
        let col = if self.char_width > 0.0 {
            ((text_x / self.char_width).round() as usize).min(char_count)
        } else {
            0
        };

        Point::new(row, col)
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
    ) -> RenderInteraction {
        self.frame_count += 1;

        let cursor = editor.cursor();
        let selection = editor.selection();
        let current_version = editor.version();
        let font_id = FontId::monospace(14.0);
        let line_height = ui.fonts(|f| f.row_height(&font_id)) + 4.0;
        let cursor_y = cursor.row as f32 * line_height;

        // Measure char_width once from a reference character
        if self.char_width <= 0.1 {
            self.char_width = ui
                .painter()
                .layout_no_wrap("m".to_string(), font_id.clone(), Color32::WHITE)
                .rect
                .width();
        }
        self.line_height = line_height;

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
        let mut interaction = RenderInteraction {
            single_clicked_at: None,
            double_clicked_at: None,
            dragging_at: None,
            drag_started: false,
        };

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
                    egui::Sense::click_and_drag(),
                );

                // Store layout for screen_to_point
                self.content_rect = response.rect;

                let line_number_width = self.line_number_width;
                let text_start_x = response.rect.min.x + line_number_width;
                let rope = editor.buffer().rope();

                // ── Interaction detection ────────────────────────────────────
                if response.double_clicked() {
                    interaction.double_clicked_at = response.interact_pointer_pos();
                } else if response.clicked() {
                    interaction.single_clicked_at = response.interact_pointer_pos();
                }

                if response.drag_started() {
                    interaction.drag_started = true;
                    interaction.dragging_at = response.interact_pointer_pos();
                } else if response.dragged() {
                    interaction.dragging_at = ui.ctx().pointer_interact_pos();
                }

                let highlights_map = self.highlighter.highlight_viewport(
                    rope,
                    current_version,
                    file_path.as_deref(),
                    visible_start,
                    visible_end,
                );

                // Selection range for highlight rendering
                let (sel_start, sel_end) = selection.range();
                let has_selection = !selection.is_empty();

                for row in visible_start..visible_end {
                    let y = response.rect.min.y + row as f32 * line_height;
                    let line = self.get_line_cached(editor, row, current_version);

                    // ── Line number ──────────────────────────────────────────
                    painter.text(
                        Pos2::new(response.rect.min.x + 10.0, y),
                        egui::Align2::LEFT_TOP,
                        format!("{:4}", row + 1),
                        font_id.clone(),
                        Color32::from_rgb(100, 100, 100),
                    );

                    // ── Selection highlight rect ─────────────────────────────
                    if has_selection && row >= sel_start.row && row <= sel_end.row {
                        let chars: Vec<char> = line.chars().collect();
                        let line_char_count = chars.len();

                        let start_col = if row == sel_start.row {
                            sel_start.column
                        } else {
                            0
                        };
                        let end_col = if row == sel_end.row {
                            sel_end.column.min(line_char_count)
                        } else {
                            // Extend to cover the newline visually
                            line_char_count + 1
                        };

                        let start_col = start_col.min(line_char_count);

                        if start_col < end_col || (!line.is_empty() && row < sel_end.row) {
                            let before_start: String = chars[..start_col].iter().collect();
                            let x_start =
                                text_start_x + self.measure_width(ui, &before_start, &font_id);

                            let end_text: String =
                                chars[..end_col.min(line_char_count)].iter().collect();
                            let x_end = if end_col > line_char_count {
                                // Extend rect to cover the newline character space
                                text_start_x
                                    + self.measure_width(ui, &end_text, &font_id)
                                    + self.char_width
                            } else {
                                text_start_x + self.measure_width(ui, &end_text, &font_id)
                            };

                            if x_end > x_start {
                                painter.rect_filled(
                                    Rect::from_min_max(
                                        Pos2::new(x_start, y),
                                        Pos2::new(x_end, y + line_height),
                                    ),
                                    0.0,
                                    Color32::from_rgba_unmultiplied(70, 130, 180, 90),
                                );
                            }
                        }
                    }

                    // ── Text + cursor ────────────────────────────────────────
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

        interaction
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
    ) -> RenderInteraction {
        self.render_with_highlighting(ui, editor, cursor_blink, should_auto_scroll)
    }
}

impl Default for ViewportRenderer {
    fn default() -> Self {
        Self::new()
    }
}
