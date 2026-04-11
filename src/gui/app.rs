use crate::editor::selection::{Selection, SelectionMode};
use crate::formatter::providers::{PrettierProvider, RustfmtProvider};
use crate::io::write_file_from_rope;
use crate::{read_file, Editor, Formatter, SyntaxHighlighter, SyntaxTheme};
use std::path::PathBuf;
use std::time::Instant;

use crate::manager::{FocusTarget, GlobalManager};
use super::viewport_renderer::ViewportRenderer;

#[derive(Clone, Debug)]
enum LoadingState {
    Idle,
    Loading { progress: f32, message: String },
    Complete,
    Error(String),
}

pub struct GuiApp {
    editor: Editor,
    last_blink: Instant,
    last_input_time: Instant,
    status_message: String,
    auto_scroll: bool,
    current_file: Option<PathBuf>,
    loading_state: LoadingState,
    renderer: ViewportRenderer,
    formatter: Formatter,
    selection_mode: SelectionMode,
    selection_anchor: Option<crate::buffer::Point>,

    manager: GlobalManager,

    // Multi-click detection
    last_click_time: Instant,
    click_count: u32,

    // Pending actions to be handled after the event loop
    pending_copy: bool,
    pending_cut: bool,
}

impl GuiApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let mut formatter = Formatter::new();
        formatter.register(Box::new(RustfmtProvider::new()));
        formatter.register(Box::new(PrettierProvider::new()));

        let highlighter = SyntaxHighlighter::new(SyntaxTheme::dark());
        let mut renderer = ViewportRenderer::new();
        renderer.highlighter = highlighter;

        Self {
            editor: Editor::new(),
            last_blink: Instant::now(),
            last_input_time: Instant::now(),
            status_message: String::new(),
            auto_scroll: true,
            current_file: None,
            loading_state: LoadingState::Idle,
            renderer,
            formatter,
            selection_mode: SelectionMode::Character,
            selection_anchor: None,
            manager: GlobalManager::new(),
            pending_copy: false,
            pending_cut: false,
            last_click_time: Instant::now(),
            click_count: 0,
        }
    }

    fn handle_text_input(&mut self, text: &str) {
        if !self.manager.focus.is_focused(FocusTarget::Editor) {
            return;
        }

        let cursor_line = self.editor.cursor().row;

        let auto_close = match text {
            "{" => Some("}"),
            "[" => Some("]"),
            "(" => Some(")"),
            "\"" => Some("\""),
            "'" => Some("'"),
            _ => None,
        };

        if let Some(closing) = auto_close {
            self.editor.insert(text);
            self.editor.insert(closing);
            self.editor.move_left();
        } else {
            self.editor.insert(text);
        }

        self.status_message.clear();
        self.auto_scroll = true;
        self.last_input_time = Instant::now();
        self.renderer.invalidate_from_line(cursor_line);
    }

    fn handle_key(&mut self, key: egui::Key, modifiers: egui::Modifiers, _ctx: &egui::Context) {
        // ── Tab (focus cycling or indent) ────────────────────────────────────
        if key == egui::Key::Tab {
            if self.manager.focus.handle_tab(modifiers.shift, &self.manager.panels) {
                return;
            }
            if self.manager.focus.is_focused(FocusTarget::Editor) {
                let cursor_line = self.editor.cursor().row;
                if modifiers.shift {
                    self.editor.outdent_selections(4);
                } else {
                    self.editor.indent_selections(4);
                }
                self.status_message.clear();
                self.auto_scroll = true;
                self.last_input_time = Instant::now();
                self.renderer.invalidate_from_line(cursor_line);
            }
            return;
        }

        if !self.manager.focus.is_focused(FocusTarget::Editor) {
            return;
        }

        self.manager.focus.on_key_pressed();
        let cursor_before = self.editor.cursor();

        match key {
            // ── Arrow keys ───────────────────────────────────────────────────
            egui::Key::ArrowLeft => {
                if modifiers.ctrl && modifiers.shift {
                    self.editor.extend_selection_word_left();
                } else if modifiers.shift {
                    self.editor.extend_selection_left();
                } else if modifiers.ctrl {
                    self.editor.move_word_left();
                } else {
                    self.editor.move_left();
                }
            }
            egui::Key::ArrowRight => {
                if modifiers.ctrl && modifiers.shift {
                    self.editor.extend_selection_word_right();
                } else if modifiers.shift {
                    self.editor.extend_selection_right();
                } else if modifiers.ctrl {
                    self.editor.move_word_right();
                } else {
                    self.editor.move_right();
                }
            }
            egui::Key::ArrowUp => {
                if modifiers.alt {
                    self.editor.move_lines_up();
                } else if modifiers.shift {
                    self.editor.extend_selection_up();
                } else {
                    self.editor.move_up();
                }
            }
            egui::Key::ArrowDown => {
                if modifiers.alt {
                    self.editor.move_lines_down();
                } else if modifiers.shift {
                    self.editor.extend_selection_down();
                } else {
                    self.editor.move_down();
                }
            }

            // ── Home / End ───────────────────────────────────────────────────
            egui::Key::Home => {
                if modifiers.ctrl && modifiers.shift {
                    // Ctrl+Shift+Home — not standard but nice to have
                    let saved = self.editor.selection().start;
                    self.editor.move_to_top();
                    let top = self.editor.cursor();
                    self.editor.set_selection(Selection::new(saved, top));
                } else if modifiers.ctrl {
                    self.editor.move_to_top();
                } else if modifiers.shift {
                    self.editor.extend_selection_to_line_start();
                } else {
                    self.editor.move_to_line_start();
                }
            }
            egui::Key::End => {
                if modifiers.ctrl && modifiers.shift {
                    let saved = self.editor.selection().start;
                    self.editor.move_to_bottom();
                    let bottom = self.editor.cursor();
                    self.editor.set_selection(Selection::new(saved, bottom));
                } else if modifiers.ctrl {
                    self.editor.move_to_bottom();
                } else if modifiers.shift {
                    self.editor.extend_selection_to_line_end();
                } else {
                    self.editor.move_to_line_end();
                }
            }

            // ── Backspace ────────────────────────────────────────────────────
            egui::Key::Backspace => {
                let cursor_line = self.editor.cursor().row;
                if modifiers.ctrl {
                    self.editor.delete_word_backward();
                } else {
                    self.editor.backspace();
                }
                self.status_message.clear();
                self.renderer.invalidate_from_line(cursor_line);
            }

            // ── Delete ───────────────────────────────────────────────────────
            egui::Key::Delete => {
                let cursor_line = self.editor.cursor().row;
                if modifiers.ctrl {
                    self.editor.delete_word_forward();
                } else if modifiers.shift {
                    // Ctrl+Shift+K equivalent on some keyboards
                    self.editor.delete_line();
                } else {
                    self.editor.delete();
                }
                self.status_message.clear();
                self.renderer.invalidate_from_line(cursor_line);
            }

            // ── Enter ────────────────────────────────────────────────────────
            egui::Key::Enter => {
                let cursor_line = self.editor.cursor().row;
                self.editor.insert("\n");
                self.status_message.clear();
                self.renderer.invalidate_from_line(cursor_line);
            }

            // ── Ctrl shortcuts ───────────────────────────────────────────────
            egui::Key::Slash if modifiers.ctrl => {
                self.editor.toggle_comments();
            }

            egui::Key::A if modifiers.ctrl => {
                self.editor.select_all();
            }

            egui::Key::Z if modifiers.ctrl => {
                if self.editor.can_undo() {
                    self.editor.undo();
                    self.status_message = "Undo".to_string();
                }
            }

            egui::Key::Y if modifiers.ctrl => {
                if self.editor.can_redo() {
                    self.editor.redo();
                    self.status_message = "Redo".to_string();
                }
            }
            

            egui::Key::D if modifiers.ctrl => {
                self.editor.select_next_occurrence();
                self.auto_scroll = true;
            }

            egui::Key::K if modifiers.ctrl && modifiers.shift => {
                // Ctrl+Shift+K — delete line (VS Code)
                self.editor.delete_line();
                self.status_message.clear();
            }

            egui::Key::S if modifiers.ctrl => {
                self.save_file();
            }

            egui::Key::O if modifiers.ctrl => {
                self.open_file();
            }

            egui::Key::F if modifiers.ctrl && modifiers.shift => {
                self.format_code();
            }

            // ── Clipboard Fallbacks ──────────────────────────────────────────
            // We handle these here directly to ensure the internal buffer is 
            // updated INSTANTLY, bypasssing any OS/egui sync delays.
            egui::Key::C if modifiers.ctrl => {
                self.pending_copy = true;
            }
            egui::Key::X if modifiers.ctrl => {
                self.pending_cut = true;
            }
            egui::Key::V if modifiers.ctrl => {
                // We'll let the main event loop's Paste event handle this
                // to get the text, but catching it here ensures focus logic.
            }

            _ => {}
        }

        let cursor_after = self.editor.cursor();
        if cursor_before != cursor_after {
            self.auto_scroll = true;
        }
    }
    fn do_copy(&mut self, ctx: &egui::Context) {
        if !self.manager.focus.is_focused(FocusTarget::Editor) {
            return;
        }

        let is_line;
        let text = if let Some(selected) = self.editor.selected_text() {
            is_line = false;
            selected
        } else {
            is_line = true;
            self.editor.current_line_text()
        };

        self.manager.clipboard.copy(text, is_line, ctx);
    }

    fn do_cut(&mut self, ctx: &egui::Context) {
        if !self.manager.focus.is_focused(FocusTarget::Editor) {
            return;
        }
        self.do_copy(ctx);
        self.editor.delete_selections();
        self.status_message.clear();
    }


    fn do_paste(&mut self, os_text: String) {
        if !self.manager.focus.is_focused(FocusTarget::Editor) {
            return;
        }

        let (text, is_line_paste) = self.manager.clipboard.paste(os_text);

        let refresh_row = if self.editor.selection().is_empty() {
            self.editor.cursor().row
        } else {
            self.editor.selection().range().0.row
        };

        if is_line_paste {
            // VS Code line paste: insert as a new line ABOVE the current line
            self.editor.move_to_line_start();
            self.editor.paste(&text); // text already has trailing \n
            self.editor.move_up();
        } else {
            // Regular paste — replaces selection if any
            self.editor.paste(&text);
        }

        self.status_message.clear();
        self.auto_scroll = true;
        self.last_input_time = Instant::now();
        self.renderer.invalidate_from_line(refresh_row);
    }

    fn format_code(&mut self) {
        if let Some(ref file_path) = self.current_file {
            match self.editor.format(&self.formatter, Some(file_path)) {
                Ok(_) => {
                    self.status_message = "✨ Code formatted successfully".to_string();
                    self.renderer.full_reset();
                }
                Err(e) => {
                    self.status_message = format!("⚠️ Format failed: {}", e);
                }
            }
        } else {
            self.status_message = "⚠️ Save file first to enable formatting".to_string();
        }
    }

    fn open_file(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter(
                "Text Files",
                &[
                    "txt", "rs", "md", "json", "toml", "py", "js", "html", "css", "xml",
                ],
            )
            .add_filter("All Files", &["*"])
            .pick_file()
        {
            match std::fs::metadata(&path) {
                Ok(metadata) => {
                    let file_size = metadata.len();
                    const MAX_SIZE: u64 = 100_000_000;
                    if file_size > MAX_SIZE {
                        self.status_message = format!(
                            "⚠️ File too large: {:.2} MB (max: 100 MB)",
                            file_size as f64 / 1_000_000.0
                        );
                        return;
                    }
                    self.load_file_simple(&path, file_size);
                }
                Err(e) => {
                    self.status_message = format!("❌ Error: {}", e);
                }
            }
        }
    }

    fn load_file_simple(&mut self, path: &PathBuf, file_size: u64) {
        match read_file(path) {
            Ok(contents) => {
                let line_count = contents.lines().count();
                self.editor = Editor::from_text(&contents);
                self.editor.set_file_path(Some(path.clone()));
                self.current_file = Some(path.clone());
                self.renderer.full_reset();
                let filename = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("Unknown");
                self.status_message = format!(
                    "✅ Opened: {} ({:.1} KB, {} lines)",
                    filename,
                    file_size as f64 / 1000.0,
                    line_count
                );
            }
            Err(e) => {
                self.status_message = format!("❌ Error: {}", e);
            }
        }
    }

    fn save_file(&mut self) {
        if let Some(ref path) = self.current_file.clone() {
            if self.formatter.find_provider(&path).is_some() {
                match self.editor.format(&self.formatter, Some(&path)) {
                    Ok(_) => {
                        self.renderer.full_reset();
                    }
                    Err(e) => {
                        self.status_message = format!("⚠️ Format failed: {}, saving anyway", e);
                    }
                }
            }
            match write_file_from_rope(&path, self.editor.buffer().rope()) {
                Ok(_) => {
                    let filename = path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("Unknown");
                    self.status_message = format!("💾 Saved: {}", filename);
                }
                Err(e) => {
                    self.status_message = format!("❌ Error: {}", e);
                }
            }
        } else {
            self.save_file_as();
        }
    }

    fn save_file_as(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Text Files", &["txt"])
            .add_filter("Rust Files", &["rs"])
            .add_filter("JavaScript Files", &["js"])
            .add_filter("Python Files", &["py"])
            .add_filter("All Files", &["*"])
            .save_file()
        {
            match write_file_from_rope(&path, self.editor.buffer().rope()) {
                Ok(_) => {
                    self.current_file = Some(path.clone());
                    self.editor.set_file_path(Some(path.clone()));
                    let filename = path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("Unknown");
                    self.status_message = format!("💾 Saved as: {}", filename);
                }
                Err(e) => {
                    self.status_message = format!("❌ Error: {}", e);
                }
            }
        }
    }

    fn new_file(&mut self) {
        self.editor = Editor::new();
        self.current_file = None;
        self.renderer.full_reset();
        self.status_message = "📄 New file".to_string();
    }
}

impl eframe::App for GuiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // ── Cursor alpha calculation ──────────────────────────────────────
        let elapsed = self.last_input_time.elapsed().as_millis();
        let cursor_alpha = if elapsed < 3000 {
            1.0 // Keep cursor solid for 3 seconds of interaction
        } else {
            // Start blinking after 3 seconds: toggle between 1.0 and 0.3
            let blink_phase = (ctx.input(|i| i.time * 2.0).floor() as i64 % 2) == 0;
            if blink_phase { 1.0 } else { 0.3 }
        };
        ctx.request_repaint();


        // ── Tab key (consumed before general input) ───────────────────────────
        let mut tab_pressed = false;
        let mut shift_tab_pressed = false;
        ctx.input_mut(|i| {
            if i.consume_key(egui::Modifiers::NONE, egui::Key::Tab) {
                tab_pressed = true;
            }
            if i.consume_key(egui::Modifiers::SHIFT, egui::Key::Tab) {
                shift_tab_pressed = true;
            }
        });
        if tab_pressed {
            self.handle_key(egui::Key::Tab, egui::Modifiers::NONE, ctx);
        }
        if shift_tab_pressed {
            self.handle_key(egui::Key::Tab, egui::Modifiers::SHIFT, ctx);
        }

        // ── Main input event loop ─────────────────────────────────────────────
        let mut paste_text: Option<String> = None;
        let mut do_copy = false;
        let mut do_cut = false;

        ctx.input(|i| {
            if !i.events.is_empty() {
                self.last_input_time = Instant::now();
            }

            for event in &i.events {
                match event {
                    egui::Event::Text(text) => {
                        self.handle_text_input(text);
                    }
                    egui::Event::Paste(text) => {
                        paste_text = Some(text.clone());
                    }
                    egui::Event::Copy => {
                        // Only copy if we haven't already handled it this frame
                        do_copy = true;
                    }
                    egui::Event::Cut => {
                        do_cut = true;
                    }
                    egui::Event::WindowFocused(focused) => {
                        if !focused {
                            self.manager.clipboard.invalidate_internal();
                        }
                    }
                    egui::Event::Key {
                        key,
                        pressed: true,
                        modifiers,
                        ..
                    } => {
                        if *key != egui::Key::Tab {
                            // Clone ctx reference to pass in
                            // (we handle it below outside the closure)
                            let _ = (key, modifiers);
                        }
                    }
                    _ => {}
                }
            }
        });

        // Handle key events outside the borrow
        let keys_to_handle: Vec<(egui::Key, egui::Modifiers)> = ctx.input(|i| {
            i.events
                .iter()
                .filter_map(|e| {
                    if let egui::Event::Key {
                        key,
                        pressed: true,
                        modifiers,
                        ..
                    } = e
                    {
                        if *key != egui::Key::Tab {
                            Some((*key, *modifiers))
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                })
                .collect()
        });

        for (key, modifiers) in keys_to_handle {
            self.handle_key(key, modifiers, ctx);
        }

        if self.pending_copy || do_copy {
            self.do_copy(ctx);
            self.pending_copy = false;
        }
        if self.pending_cut || do_cut {
            self.do_cut(ctx);
            self.pending_cut = false;
        }
        if let Some(text) = paste_text {
            // Priority: do_paste will check manager's freshness tracking
            self.do_paste(text);
        }

        // ── Top menu bar ──────────────────────────────────────────────────────
        egui::TopBottomPanel::top("menu").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("📄 New").clicked() {
                        self.new_file();
                        ui.close_menu();
                    }
                    if ui.button("📂 Open (Ctrl+O)").clicked() {
                        self.open_file();
                        ui.close_menu();
                    }
                    if ui.button("💾 Save (Ctrl+S)").clicked() {
                        self.save_file();
                        ui.close_menu();
                    }
                    if ui.button("💾 Save As...").clicked() {
                        self.save_file_as();
                        ui.close_menu();
                    }
                });

                ui.menu_button("Edit", |ui| {
                    if ui
                        .add_enabled(self.editor.can_undo(), egui::Button::new("↶ Undo (Ctrl+Z)"))
                        .clicked()
                    {
                        self.editor.undo();
                        ui.close_menu();
                    }
                    if ui
                        .add_enabled(self.editor.can_redo(), egui::Button::new("↷ Redo (Ctrl+Y)"))
                        .clicked()
                    {
                        self.editor.redo();
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("📋 Select All (Ctrl+A)").clicked() {
                        self.editor.select_all();
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("📄 Copy (Ctrl+C)").clicked() {
                        self.do_copy(ctx);
                        ui.close_menu();
                    }
                    if ui.button("✂ Cut (Ctrl+X)").clicked() {
                        self.do_cut(ctx);
                        ui.close_menu();
                    }
                    if ui.button("📋 Paste (Ctrl+V)").clicked() {
                        // Request paste text from egui if possible
                        // Note: actual text will arrive via egui::Event::Paste
                        ui.ctx().output_mut(|_o| {
                            // Some platforms support this to request clipboard
                        });
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui
                        .add_enabled(
                            self.current_file.is_some(),
                            egui::Button::new("✨ Format Code (Ctrl+Shift+F)"),
                        )
                        .clicked()
                    {
                        self.format_code();
                        ui.close_menu();
                    }
                });

                ui.separator();
                let filename = self
                    .current_file
                    .as_ref()
                    .and_then(|p| p.file_name())
                    .and_then(|n| n.to_str())
                    .unwrap_or("Untitled");
                ui.label(format!("📝 {}", filename));
            });
        });

        // ── Status bar ────────────────────────────────────────────────────────
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            let cursor = self.editor.cursor();
            let selections = self.editor.selections();
            let primary_selection = self.editor.selection();
            
            let status = if !self.status_message.is_empty() {
                self.status_message.clone()
            } else if selections.len() > 1 {
                format!("{} cursors", selections.len())
            } else if !primary_selection.is_empty() {
                let (start, end) = primary_selection.range();
                let selected = self.editor.selected_text().unwrap_or_default();
                let char_count = selected.chars().count();
                format!(
                    "Line {}, Col {} | Selected: {} chars ({} → {})",
                    cursor.row + 1,
                    cursor.column + 1,
                    char_count,
                    start.row + 1,
                    end.row + 1,
                )
            } else {
                format!(
                    "Line {}, Col {} | {} lines",
                    cursor.row + 1,
                    cursor.column + 1,
                    self.editor.line_count()
                )
            };
            ui.horizontal(|ui| {
                ui.label(status);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        egui::RichText::new(self.manager.focus.status_label())
                            .color(egui::Color32::from_rgb(100, 160, 255))
                            .small(),
                    );
                });
            });
        });

        // ── Editor (central panel) ────────────────────────────────────────────
        egui::CentralPanel::default().show(ctx, |ui| {
            if ui.rect_contains_pointer(ui.max_rect()) && ctx.input(|i| i.pointer.primary_clicked())
            {
                self.manager.focus.set(FocusTarget::Editor);
            }

            {
                let events = self.editor.drain_edit_events();
                for e in events {
                    self.renderer.notify_edit(self.editor.buffer().rope(), &e);
                }
            }

            let interaction = self.renderer.render_with_highlighting(
                ui,
                &self.editor,
                cursor_alpha,
                self.auto_scroll,
            );

            if ui.rect_contains_pointer(ui.max_rect()) {
                ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::Text);
            }
            self.auto_scroll = false;

            // ── Click & Press handling ────────────────────────────────────
            
            // 1. Immediate response on Mouse Down (Press)
            if let Some(pos) = interaction.pressed_at {
                let point = self.renderer.screen_to_point(ui, pos, &self.editor);
                self.last_input_time = Instant::now();
                
                // Multi-click detection (0.4s threshold)
                let now = Instant::now();
                if now.duration_since(self.last_click_time).as_secs_f32() < 0.4 {
                    self.click_count = (self.click_count + 1).min(3);
                } else {
                    self.click_count = 1;
                }
                self.last_click_time = now;

                self.selection_anchor = Some(point);
                
                match self.click_count {
                    1 => {
                        self.selection_mode = SelectionMode::Character;
                        if ui.input(|i| i.modifiers.alt) {
                            self.editor.add_selection(point);
                        } else {
                            self.editor.set_cursor(point);
                        }
                    }
                    2 => {
                        self.selection_mode = SelectionMode::Word;
                        self.editor.set_cursor(point);
                        self.editor.select_word_at_cursor();
                    }
                    3 => {
                        self.selection_mode = SelectionMode::Line;
                        self.editor.set_cursor(point);
                        self.editor.select_line_at_cursor();
                    }
                    _ => {}
                }
                self.auto_scroll = false;
            }

            // ── Drag selection ────────────────────────────────────────────
            if interaction.drag_started {
                if let Some(pos) = interaction.dragging_at {
                    let point = self.renderer.screen_to_point(ui, pos, &self.editor);
                    // On new drag start, if no anchor, set it
                    if self.selection_anchor.is_none() {
                        self.selection_anchor = Some(point);
                        self.selection_mode = SelectionMode::Character;
                    }
                }
                self.auto_scroll = false;
            } else if let Some(pos) = interaction.dragging_at {
                // Extend selection while dragging
                if let Some(anchor) = self.selection_anchor {
                    let drag_point = self.renderer.screen_to_point(ui, pos, &self.editor);
                    self.editor.set_selection_with_mode(anchor, drag_point, self.selection_mode);
                    self.auto_scroll = false;
                }
            }
        });
    }
}
