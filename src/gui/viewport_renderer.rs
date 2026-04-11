use crate::buffer::Point;
use crate::syntax::{SyntaxHighlighter, SyntaxTheme};
use egui::text::{CCursor, LayoutJob, TextFormat};
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
    /// Screen position when the mouse is first pressed (down)
    pub pressed_at: Option<Pos2>,
    /// Screen position of a single click (released)
    pub single_clicked_at: Option<Pos2>,
    /// Screen position of a double click (released)
    pub double_clicked_at: Option<Pos2>,
    /// Current pointer position while dragging
    pub dragging_at: Option<Pos2>,
    /// Whether a drag just started this frame
    pub drag_started: bool,
}

pub struct ViewportRenderer {
    line_cache: HashMap<usize, CachedLine>,
    last_version: u64,
    frame_count: u64,
    pub highlighter: SyntaxHighlighter,
    last_viewport: (usize, usize),

    // Layout state stored after each render — used by screen_to_point
    content_rect: Rect,
    line_height: f32,
    char_width: f32,        // monospace char width, measured once
    line_number_width: f32, // always 50.0
    scroll_offset: Vec2,    // horizontal/vertical scroll from viewport
}

impl ViewportRenderer {
    pub fn new() -> Self {
        Self {
            line_cache: HashMap::new(),
            last_version: 0,
            frame_count: 0,
            highlighter: SyntaxHighlighter::new(SyntaxTheme::dark()),
            last_viewport: (0, 0),
            content_rect: Rect::ZERO,
            line_height: 18.0,
            char_width: 8.4,
            line_number_width: 50.0,
            scroll_offset: Vec2::ZERO,
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
        self.highlighter.reset();
    }

    pub fn invalidate_from_line(&mut self, start_line: usize) {
        self.line_cache.retain(|&line, _| line < start_line);
        self.highlighter.invalidate();
    }

    pub fn invalidate_line(&mut self, line: usize) {
        self.line_cache.remove(&line);
        self.highlighter.invalidate();
    }

    /// Convert a screen position (from pointer events) to a buffer Point.
    /// Uses the layout stored during the last render call.
    /// Convert a screen position (from pointer events) to a buffer Point.
    /// Uses the layout stored during the last render call.
    pub fn screen_to_point(&mut self, ui: &egui::Ui, screen_pos: Pos2, editor: &crate::Editor) -> Point {
        let rel_y = screen_pos.y - self.content_rect.min.y;
        let row = (rel_y / self.line_height).floor() as usize;
        let row = row.min(editor.line_count().saturating_sub(1));

        // text_x starts after line numbers. 
        // Note: response.rect.min.x already includes the scroll offset in egui.
        let gutter_end_x = self.content_rect.min.x + self.line_number_width;
        let text_x = (screen_pos.x - gutter_end_x).max(0.0);

        let line = editor.buffer().line(row).unwrap_or_default();
        
        // Use the same font as used in rendering
        let font_id = FontId::monospace(14.0);
        
        // Create a galley for this line to get precise character mapping
        let galley = ui.painter().layout_no_wrap(line.clone(), font_id, Color32::WHITE);
        
        // Calibration: Get the exact offset where char 0 starts within the galley.
        // Some egui versions/fonts add a small internal margin.
        let internal_margin = galley.pos_from_ccursor(CCursor::new(0)).min.x;
        
        // Use egui's built-in cursor-from-pos which is very accurate
        // Adjust the input x to account for the same internal margin used during rendering
        let calibrated_x = text_x + internal_margin;
        
        let cursor = galley.cursor_from_pos(Vec2::new(calibrated_x, self.line_height / 2.0));
        let col = cursor.ccursor.index;

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


    pub fn render_with_highlighting(
        &mut self,
        ui: &mut egui::Ui,
        editor: &crate::Editor,
        cursor_alpha: f32,
        should_auto_scroll: bool,
    ) -> RenderInteraction {
        self.frame_count += 1;

        let cursor = editor.cursor();
        let _selection = editor.selection();
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
        }

        let file_path = editor.file_path().map(|p| p.to_path_buf());
        let mut interaction = RenderInteraction {
            pressed_at: None,
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
                self.scroll_offset = viewport.min.to_vec2();

                let line_number_width = self.line_number_width;
                let text_start_x = response.rect.min.x + line_number_width;
                let rope = editor.buffer().rope();

                // ── Interaction detection ────────────────────────────────────
                if response.hovered() && ui.input(|i| i.pointer.primary_pressed()) {
                    interaction.pressed_at = response.interact_pointer_pos();
                }
                if response.double_clicked() {
                    interaction.double_clicked_at = response.interact_pointer_pos();
                }
                if response.clicked() {
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

                  let selections = editor.selections();
                let primary_cursor = editor.cursor(); // For active line highlighting and line numbers

                for row in visible_start..visible_end {
                    let y = response.rect.min.y + row as f32 * line_height;
                    let line = self.get_line_cached(editor, row, current_version);
                    let highlights = highlights_map.get(&row).cloned().unwrap_or_default();

                    // ── Active Line Highlight ───────────────────────────────
                    if row == primary_cursor.row {
                        painter.rect_filled(
                            Rect::from_min_max(
                                Pos2::new(response.rect.min.x, y),
                                Pos2::new(response.rect.max.x, y + line_height),
                            ),
                            0.0,
                            Color32::from_rgba_unmultiplied(255, 255, 255, 10),
                        );
                    }

                    // ── Line number ──────────────────────────────────────────
                    let line_num_color = if row == primary_cursor.row {
                        Color32::from_rgb(200, 200, 200)
                    } else {
                        Color32::from_rgb(100, 100, 100)
                    };
                    painter.text(
                        Pos2::new(response.rect.min.x + 10.0, y),
                        egui::Align2::LEFT_TOP,
                        format!("{:4}", row + 1),
                        font_id.clone(),
                        line_num_color,
                    );

                    // ── Create Galley for the entire line ───────────────────
                    let mut job = LayoutJob::default();
                    if highlights.is_empty() {
                        job.append(&line, 0.0, TextFormat::simple(font_id.clone(), Color32::WHITE));
                    } else {
                        let chars: Vec<char> = line.chars().collect();
                        let mut last_end = 0;
                        for &(start, end, color) in &highlights {
                            if last_end < start {
                                let text: String = chars[last_end..start.min(chars.len())].iter().collect();
                                job.append(&text, 0.0, TextFormat::simple(font_id.clone(), Color32::WHITE));
                            }
                            let span_end = end.min(chars.len());
                            if start < span_end {
                                let text: String = chars[start..span_end].iter().collect();
                                job.append(&text, 0.0, TextFormat::simple(font_id.clone(), color));
                            }
                            last_end = span_end;
                        }
                        if last_end < chars.len() {
                            let text: String = chars[last_end..].iter().collect();
                            job.append(&text, 0.0, TextFormat::simple(font_id.clone(), Color32::WHITE));
                        }
                    }
                    
                    let galley = ui.fonts(|f| f.layout_job(job));

                    // ── Render all selections and cursors for this line ──────
                    for selection in selections {
                        let (sel_start, sel_end) = selection.range();
                        
                        // 1. Selection Highlight
                        if !selection.is_empty() && row >= sel_start.row && row <= sel_end.row {
                            let line_char_count = line.chars().count();
                            let start_col = if row == sel_start.row { sel_start.column } else { 0 };
                            let end_col = if row == sel_end.row { sel_end.column } else { line_char_count + 1 };
                            
                            let start_col = start_col.min(line_char_count);
                            let end_col = end_col.min(line_char_count + 1);

                            if start_col < end_col || (row < sel_end.row) {
                                let x_start = if start_col == 0 {
                                    0.0
                                } else {
                                    galley.pos_from_ccursor(CCursor::new(start_col)).min.x
                                };

                                let x_end = if end_col > line_char_count {
                                    galley.rect.width() + self.char_width
                                } else {
                                    galley.pos_from_ccursor(CCursor::new(end_col)).min.x
                                };

                                if x_end > x_start {
                                    painter.rect_filled(
                                        Rect::from_min_max(
                                            Pos2::new(text_start_x + x_start, y),
                                            Pos2::new(text_start_x + x_end, y + line_height),
                                        ),
                                        0.0,
                                        Color32::from_rgba_unmultiplied(100, 150, 255, 60),
                                    );
                                }
                            }
                        }

                        // 2. Cursor
                        if row == selection.end.row && cursor_alpha > 0.01 {
                            let cursor_pos = selection.end.column.min(line.chars().count());
                            let cursor_x = if cursor_pos == 0 {
                                0.0
                            } else {
                                galley.pos_from_ccursor(CCursor::new(cursor_pos)).min.x
                            };

                            let cursor_height = line_height * 0.85;
                            let cursor_y_offset = (line_height - cursor_height) / 2.0;
                            painter.rect_filled(
                                Rect::from_min_size(
                                    Pos2::new(text_start_x + cursor_x, y + cursor_y_offset),
                                    Vec2::new(2.0, cursor_height),
                                ),
                                0.0,
                                Color32::from_rgba_unmultiplied(255, 255, 255, (cursor_alpha * 255.0) as u8),
                            );
                        }
                    }

                    // ── Text rendering ──────────────────────────────────────
                    painter.galley(Pos2::new(text_start_x, y), galley.clone(), Color32::WHITE);
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


    pub fn render(
        &mut self,
        ui: &mut egui::Ui,
        editor: &crate::Editor,
        cursor_alpha: f32,
        should_auto_scroll: bool,
    ) -> RenderInteraction {
        self.render_with_highlighting(ui, editor, cursor_alpha, should_auto_scroll)
    }
}

impl Default for ViewportRenderer {
    fn default() -> Self {
        Self::new()
    }
}
