use crate::syntax::{HighlightedRange, InstantHighlighter};
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

/// 🚀 NEW: Cached line offset (line number -> byte offset)
#[derive(Clone)]
struct CachedLineOffset {
    offset: usize,
    version: u64,
}

/// Fast viewport renderer with caching
pub struct ViewportRenderer {
    line_cache: HashMap<usize, CachedLine>,
    width_cache: HashMap<String, f32>,
    // 🚀 NEW: Cache line_to_byte conversions (this is the expensive operation!)
    line_offset_cache: HashMap<usize, CachedLineOffset>,
    last_version: u64,
    frame_count: u64,
    highlighter: InstantHighlighter,
    predictive_cache: crate::buffer::PredictiveCache,
    last_viewport: (usize, usize),
    last_cursor_line: usize,
}

impl ViewportRenderer {
    pub fn new() -> Self {
        Self {
            line_cache: HashMap::new(),
            width_cache: HashMap::new(),
            line_offset_cache: HashMap::new(),
            last_version: 0,
            frame_count: 0,
            highlighter: InstantHighlighter::new(),
            predictive_cache: crate::buffer::PredictiveCache::new(),
            last_viewport: (0, 0),
            last_cursor_line: 0,
        }
    }

    /// 🚀 NEW: Get line offset with caching (avoids expensive rope scans!)
    fn get_line_offset_cached(
        &mut self,
        rope: &crate::rope::Rope,
        line_idx: usize,
        current_version: u64,
    ) -> usize {
        // Check cache first
        if let Some(cached) = self.line_offset_cache.get(&line_idx) {
            if cached.version == current_version {
                return cached.offset;
            }
        }

        // Cache miss - calculate and store
        let offset = rope.line_to_byte(line_idx);

        // Only cache if we have room (prevent unbounded growth)
        if self.line_offset_cache.len() < 10_000 {
            self.line_offset_cache.insert(
                line_idx,
                CachedLineOffset {
                    offset,
                    version: current_version,
                },
            );
        }

        offset
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
        self.line_offset_cache.retain(|&line, _| line < start_line);
        self.width_cache.clear();
    }

    /// Invalidate specific line
    pub fn invalidate_line(&mut self, line: usize) {
        self.line_cache.remove(&line);
        self.line_offset_cache.remove(&line);
    }

    /// 🚀 ULTRA-OPTIMIZED: Render viewport with FAST regex-based syntax highlighting
    /// Uses InstantHighlighter instead of slow tree-sitter (100-1000x faster!)
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

        // Clear caches if version changed
        if self.last_version != current_version {
            // Clear line offset cache on version change
            self.line_offset_cache.clear();
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
            if self.line_offset_cache.len() > 10_000 {
                // Keep only viewport lines and nearby buffer
                let viewport_start = self.last_viewport.0.saturating_sub(100);
                let viewport_end = self.last_viewport.1 + 100;
                self.line_offset_cache
                    .retain(|k, _| *k >= viewport_start && *k <= viewport_end);
            }
        }

        let file_path = editor.file_path();

        egui::ScrollArea::both()
            .auto_shrink([false, false])
            .show_viewport(ui, |ui, viewport| {
                let total_lines = editor.line_count().max(1);
                let content_height = total_lines as f32 * line_height;

                let visible_start = (viewport.min.y / line_height).floor().max(0.0) as usize;
                let visible_end =
                    ((viewport.max.y / line_height).ceil() as usize + 1).min(total_lines);

                // 🚀 SCROLL PREDICTION: Track scroll delta for predictive caching
                let scroll_delta = ui.input(|i| i.smooth_scroll_delta.y);
                let frame_time = ui.input(|i| i.stable_dt);

                // Detect viewport changes (scrollbar drag or smooth scroll)
                let viewport_changed =
                    self.last_viewport.0 != visible_start || self.last_viewport.1 != visible_end;

                if viewport_changed || scroll_delta.abs() > 0.1 {
                    self.predictive_cache.update_scroll_prediction(
                        visible_start..visible_end,
                        scroll_delta,
                        frame_time,
                    );
                    self.last_viewport = (visible_start, visible_end);

                    // 🚀 PRE-CACHE: When scrolling, pre-cache line offsets in scroll direction
                    let rope = editor.buffer().rope();
                    let precache_range = if scroll_delta < -0.1 {
                        // Scrolling up - pre-cache lines above
                        let start = visible_start.saturating_sub(100);
                        let end = visible_start;
                        start..end
                    } else if scroll_delta > 0.1 {
                        // Scrolling down - pre-cache lines below
                        let start = visible_end;
                        let end = (visible_end + 100).min(total_lines);
                        start..end
                    } else {
                        // Big jump (scrollbar drag) - cache visible range
                        visible_start..visible_end
                    };

                    // Pre-cache the predicted range
                    for line_idx in precache_range {
                        if !self.line_offset_cache.contains_key(&line_idx) {
                            let offset = rope.line_to_byte(line_idx);
                            if self.line_offset_cache.len() < 10_000 {
                                self.line_offset_cache.insert(
                                    line_idx,
                                    CachedLineOffset {
                                        offset,
                                        version: current_version,
                                    },
                                );
                            }
                        }
                    }
                }

                let (response, painter) = ui.allocate_painter(
                    Vec2::new(ui.available_width(), content_height),
                    egui::Sense::click(),
                );

                let line_number_width = 60.0;
                let text_start_x = response.rect.min.x + line_number_width;

                // 🚀 SIMPLIFIED: Just calculate highlights for visible region (regex is fast!)
                let language = InstantHighlighter::detect_language(file_path);
                let highlights = self.get_highlights_for_viewport(
                    editor,
                    visible_start,
                    visible_end,
                    language,
                    current_version,
                );

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

                    // Get highlights for this specific line
                    let line_highlights =
                        self.filter_highlights_for_line(&highlights, editor, row, current_version);

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
                            &line_highlights,
                        );
                    } else if !line.is_empty() {
                        self.render_highlighted_line(
                            &painter,
                            &line,
                            text_start_x,
                            y,
                            &font_id,
                            &line_highlights,
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

    /// 🚀 SIMPLIFIED: No caching - just calculate fresh (regex is fast!)
    /// This matches the old editor's proven approach
    fn get_highlights_for_viewport(
        &mut self,
        editor: &crate::Editor,
        visible_start: usize,
        visible_end: usize,
        language: &str,
        current_version: u64,
    ) -> Vec<HighlightedRange> {
        let rope = editor.buffer().rope();

        // 🚀 USE CACHED line_to_byte calls!
        let visible_start_byte = self.get_line_offset_cached(rope, visible_start, current_version);
        let visible_end_byte = if visible_end < editor.line_count() {
            self.get_line_offset_cached(rope, visible_end, current_version)
        } else {
            rope.len()
        };

        let visible_text = rope.slice_bytes(visible_start_byte, visible_end_byte);

        // 🚀 ALWAYS FRESH: Regex is fast enough, caching adds overhead
        self.highlighter
            .highlight_visible_region(&visible_text, 0, visible_text.len(), language)
            .into_iter()
            .map(|mut h| {
                h.start += visible_start_byte;
                h.end += visible_start_byte;
                h
            })
            .collect()
    }

    /// 🚀 Filter highlights to only those affecting a specific line
    fn filter_highlights_for_line(
        &mut self,
        highlights: &[HighlightedRange],
        editor: &crate::Editor,
        line_idx: usize,
        current_version: u64,
    ) -> Vec<(usize, usize, Color32)> {
        let rope = editor.buffer().rope();

        // 🚀 USE CACHED line_to_byte calls!
        let line_start_byte = self.get_line_offset_cached(rope, line_idx, current_version);

        let line_end_byte = if line_idx + 1 < editor.line_count() {
            self.get_line_offset_cached(rope, line_idx + 1, current_version)
        } else {
            rope.len()
        };

        let line_content = editor.buffer().line(line_idx).unwrap_or_default();

        highlights
            .iter()
            .filter(|h| h.end > line_start_byte && h.start < line_end_byte)
            .filter_map(|h| {
                let start_in_line = if h.start > line_start_byte {
                    h.start - line_start_byte
                } else {
                    0
                };

                let end_in_line = if h.end < line_end_byte {
                    h.end - line_start_byte
                } else {
                    line_content.len()
                };

                // Convert byte offsets to character offsets
                let start_char = line_content[..start_in_line.min(line_content.len())]
                    .chars()
                    .count();
                let end_char = line_content[..end_in_line.min(line_content.len())]
                    .chars()
                    .count();

                if start_char < end_char {
                    Some((start_char, end_char, h.highlight.to_color()))
                } else {
                    None
                }
            })
            .collect()
    }

    /// Render a line with syntax highlighting
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

        for &(start, end, color) in highlights {
            // Render unhighlighted text before this span
            if last_end < start {
                let text: String = chars[last_end..start].iter().collect();
                if !text.is_empty() {
                    let galley =
                        painter.layout_no_wrap(text.clone(), font_id.clone(), Color32::WHITE);
                    painter.galley(Pos2::new(current_x, y), galley.clone(), Color32::WHITE);
                    current_x += galley.rect.width();
                }
            }

            // Render highlighted span
            let span_end = end.min(chars.len());
            let text: String = chars[start..span_end].iter().collect();
            if !text.is_empty() {
                let galley = painter.layout_no_wrap(text.clone(), font_id.clone(), color);
                painter.galley(Pos2::new(current_x, y), galley.clone(), color);
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
        highlights: &[(usize, usize, Color32)],
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
            for &(start, end, color) in highlights {
                // Render unhighlighted text before this span
                if last_end < start {
                    let text: String = chars[last_end..start].iter().collect();
                    if !text.is_empty() {
                        let galley = painter.layout_no_wrap(text, font_id.clone(), Color32::WHITE);
                        painter.galley(Pos2::new(current_x, y), galley.clone(), Color32::WHITE);
                        current_x += galley.rect.width();
                    }
                }

                // Render highlighted span
                let span_end = end.min(chars.len());
                let text: String = chars[start..span_end].iter().collect();
                if !text.is_empty() {
                    let galley = painter.layout_no_wrap(text, font_id.clone(), color);
                    painter.galley(Pos2::new(current_x, y), galley.clone(), color);
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

    /// Simplified render method (no external highlighter needed)
    pub fn render(
        &mut self,
        ui: &mut egui::Ui,
        editor: &crate::Editor,
        cursor_blink: bool,
        should_auto_scroll: bool,
    ) {
        // Just call the highlighting version (highlighter is built-in now)
        self.render_with_highlighting(ui, editor, cursor_blink, should_auto_scroll);
    }
}

impl Default for ViewportRenderer {
    fn default() -> Self {
        Self::new()
    }
}
