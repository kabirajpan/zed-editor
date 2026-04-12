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
    
    // 🔱 Layer 2: Virtual Shifting state for screen_to_point
    last_virtual_expansion: f32,
    last_hint_row: Option<usize>,
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
            last_virtual_expansion: 0.0,
            last_hint_row: None,
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

    pub fn get_code_context(&self, byte_offset: usize) -> Option<crate::syntax::CodeContext> {
        self.highlighter.get_code_context(byte_offset)
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
        let mut rel_y = screen_pos.y - self.content_rect.min.y;

        // 🔱 Layer 2: Inverse Virtual Shifting
        if let Some(h_row) = self.last_hint_row {
            let hint_y = h_row as f32 * self.line_height;
            // If we are below the hint, subtract the expansion to find the real buffer row
            if rel_y > hint_y + self.line_height {
                 rel_y = (rel_y - self.last_virtual_expansion).max(hint_y + self.line_height);
            }
        }

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
        nep_hint: Option<&crate::gui::app::NepHint>,
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
                
                // 🔱 Layer 2: Virtual Expansion for Multi-Line NEP
                let mut virtual_height_expansion = 0.0;
                let mut hint_row = None;
                if let Some(hint) = nep_hint {
                    if hint.version == editor.version() {
                        virtual_height_expansion = (hint.line_count - 1) as f32 * line_height;
                        hint_row = Some(hint.anchor.row);
                    }
                }

                self.last_virtual_expansion = virtual_height_expansion;
                self.last_hint_row = hint_row;

                let content_height = (total_lines as f32 * line_height) + virtual_height_expansion;

                // Simple visible range calculation (could be more precise but this works for single-hint)
                let visible_start = (viewport.min.y / line_height).floor().max(0.0) as usize;
                let visible_end =
                    (((viewport.max.y + virtual_height_expansion.abs()) / line_height).ceil() as usize + 1).min(total_lines);

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
                    // 🔱 Layer 2: Virtual Shifting
                    let mut y = response.rect.min.y + row as f32 * line_height;
                    if let Some(h_row) = hint_row {
                        if row > h_row {
                            y += virtual_height_expansion;
                        }
                    }

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

                    // 🔱 Layer 2: Ghost Line Numbers for Virtual Expansion
                    if let Some(h_row) = hint_row {
                        if row == h_row {
                            let hint_virtual_lines = (virtual_height_expansion / line_height) as usize;
                            for i in 1..=hint_virtual_lines {
                                painter.text(
                                    Pos2::new(response.rect.min.x + 10.0, y + i as f32 * line_height),
                                    egui::Align2::LEFT_TOP,
                                    "  + ".to_string(),
                                    font_id.clone(),
                                    Color32::from_rgb(80, 120, 80), // Faded green for ghost numbers
                                );
                            }
                        }
                    }

                    // ── Create Galley with Layer 2: Ghost Text Support ──────
                    let mut job = LayoutJob::default();
                    let line_start_val = editor.buffer().point_to_offset(crate::buffer::Point::new(row, 0)).value();
                    
                    // 🔱 Layer 2 Optimization: Fetch authorship spans for the whole line once
                    let authorship = if editor.is_speculative_active() || editor.ai_ghost_text.is_some() {
                        editor.get_line_authorship_spans(row)
                    } else {
                        Vec::new()
                    };

                    // 🔱 NEP Anchor Check
                    let mut nep_hint_content: Option<&String> = None;
                    let mut nep_column = 0;
                    if let Some(hint) = nep_hint {
                        if hint.anchor.row == row && hint.version == editor.version() {
                            nep_hint_content = Some(&hint.text);
                            nep_column = hint.anchor.column;
                        }
                    }

                    // Helper to append text and potentially inject NEP hint
                    let mut current_col = 0;
                    let mut append_text = |job: &mut LayoutJob, text: &str, format: TextFormat| {
                        let text_len = text.chars().count();
                        if let Some(h_text) = nep_hint_content {
                            if current_col <= nep_column && current_col + text_len >= nep_column {
                                // Split text at nep_column
                                let split_idx = nep_column - current_col;
                                let head: String = text.chars().take(split_idx).collect();
                                let tail: String = text.chars().skip(split_idx).collect();
                                
                                job.append(&head, 0.0, format.clone());
                                // Inject NEP Hint
                                let ghost_format = TextFormat::simple(font_id.clone(), Color32::from_gray(120));
                                job.append(h_text, 0.0, ghost_format);
                                job.append(&tail, 0.0, format);
                                nep_hint_content = None; // Only inject once
                            } else {
                                job.append(text, 0.0, format);
                            }
                        } else {
                            job.append(text, 0.0, format);
                        }
                        current_col += text_len;
                    };

                    if highlights.is_empty() {
                        if authorship.is_empty() {
                            append_text(&mut job, &line, TextFormat::simple(font_id.clone(), Color32::WHITE));
                        } else {
                            for span in &authorship {
                                let start_rel = span.offset.saturating_sub(line_start_val);
                                let end_rel = span.end().saturating_sub(line_start_val);
                                let text_slice = &line[start_rel.min(line.len())..end_rel.min(line.len())];
                                
                                let mut format = TextFormat::simple(font_id.clone(), Color32::WHITE);
                                if span.author == crate::history::transaction::Author::AiPending {
                                    format.color = Color32::from_rgb(120, 120, 120);
                                    format.italics = true;
                                }
                                append_text(&mut job, text_slice, format);
                            }
                        }
                    } else {
                        let mut last_end = 0;
                        for &(start, end, color) in &highlights {
                            if last_end < start {
                                let text_slice = &line[last_end..start.min(line.len())];
                                append_text(&mut job, text_slice, TextFormat::simple(font_id.clone(), Color32::WHITE));
                            }
                            
                            let span_end = end.min(line.len());
                            if start < span_end {
                                let is_ghost = authorship.iter().any(|p_span| {
                                    let abs_start = line_start_val + start;
                                    p_span.author == crate::history::transaction::Author::AiPending && abs_start >= p_span.offset && abs_start < p_span.end()
                                });

                                let mut format = TextFormat::simple(font_id.clone(), color);
                                if is_ghost {
                                    format.color = Color32::from_gray(120);
                                    format.italics = true;
                                }
                                let text_slice = &line[start..span_end];
                                append_text(&mut job, text_slice, format);
                            }
                            last_end = span_end;
                        }
                        if last_end < line.len() {
                            let text_slice = &line[last_end..];
                            append_text(&mut job, text_slice, TextFormat::simple(font_id.clone(), Color32::WHITE));
                        }
                    }

                    // Fallback: If NEP hint wasn't injected (e.g. anchor column is beyond line end)
                    if let Some(h_text) = nep_hint_content {
                        let ghost_format = TextFormat::simple(font_id.clone(), Color32::from_gray(120));
                        job.append(h_text, 0.0, ghost_format);
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

                            // 👻 3. Ghost Text (AI Suggestion)
                            if let Some(ghost) = &editor.ai_ghost_text {
                                if !ghost.is_empty() {
                                    let ghost_color = Color32::from_rgb(100, 100, 110);
                                    let ghost_font = FontId::new(14.0, egui::FontFamily::Monospace);
                                    
                                    // Only show the first line of multi-line ghost text inline for now
                                    let display_ghost = ghost.lines().next().unwrap_or("");
                                    
                                    painter.text(
                                        Pos2::new(text_start_x + cursor_x + 4.0, y),
                                        egui::Align2::LEFT_TOP,
                                        display_ghost,
                                        ghost_font,
                                        ghost_color,
                                    );
                                }
                            }
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
        self.render_with_highlighting(ui, editor, cursor_alpha, should_auto_scroll, None)
    }
}

impl Default for ViewportRenderer {
    fn default() -> Self {
        Self::new()
    }
}
