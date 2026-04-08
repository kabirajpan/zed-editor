use crate::buffer::Point;
use crate::editor::selection::Selection;
use crate::formatter::providers::{PrettierProvider, RustfmtProvider};
use crate::io::write_file_from_rope;
use crate::{read_file, Editor, Formatter, SyntaxHighlighter, SyntaxTheme};
use std::path::PathBuf;
use std::time::Instant;

use super::focus::{ActivePanels, FocusManager, FocusTarget};
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
    cursor_blink: bool,
    last_blink: Instant,
    last_input_time: Instant,
    status_message: String,
    auto_scroll: bool,
    current_file: Option<PathBuf>,
    loading_state: LoadingState,
    renderer: ViewportRenderer,
    formatter: Formatter,
    highlighter: SyntaxHighlighter,

    focus: FocusManager,
    active_panels: ActivePanels,

    // Clipboard state
    // We store what we last copied so we know whether to do a line-paste or
    // a regular paste.  clipboard_is_line is true when Ctrl+C/X was pressed
    // with no selection (whole-line copy, like VS Code).
    clipboard: String,
    clipboard_is_line: bool,

    // Click tracking for triple-click detection
    last_click_time: Instant,
    click_count: u8,
}

impl GuiApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let mut formatter = Formatter::new();
        formatter.register(Box::new(RustfmtProvider::new()));
        formatter.register(Box::new(PrettierProvider::new()));

        let highlighter = SyntaxHighlighter::new(SyntaxTheme::dark());

        Self {
            editor: Editor::new(),
            cursor_blink: true,
            last_blink: Instant::now(),
            last_input_time: Instant::now(),
            status_message: String::new(),
            auto_scroll: true,
            current_file: None,
            loading_state: LoadingState::Idle,
            renderer: ViewportRenderer::new(),
            formatter,
            highlighter,
            focus: FocusManager::new(),
            active_panels: ActivePanels::default(),
            clipboard: String::new(),
            clipboard_is_line: false,
            last_click_time: Instant::now(),
            click_count: 0,
        }
    }

    fn handle_text_input(&mut self, text: &str) {
        if !self.focus.is_focused(FocusTarget::Editor) {
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
        self.cursor_blink = true;
        self.renderer.invalidate_from_line(cursor_line);
    }

    fn handle_key(&mut self, key: egui::Key, modifiers: egui::Modifiers, ctx: &egui::Context) {
        // ── Tab (focus cycling or indent) ────────────────────────────────────
        if key == egui::Key::Tab {
            let consumed = self.focus.handle_tab(modifiers.shift, &self.active_panels);
            if !consumed && self.focus.is_focused(FocusTarget::Editor) {
                let cursor_line = self.editor.cursor().row;
                self.editor.insert("    ");
                self.status_message.clear();
                self.auto_scroll = true;
                self.last_input_time = Instant::now();
                self.cursor_blink = true;
                self.renderer.invalidate_from_line(cursor_line);
            }
            return;
        }

        if !self.focus.is_focused(FocusTarget::Editor) {
            return;
        }

        self.focus.on_key_pressed();
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
                if modifiers.shift {
                    self.editor.extend_selection_up();
                } else {
                    self.editor.move_up();
                }
            }
            egui::Key::ArrowDown => {
                if modifiers.shift {
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
                    self.renderer.full_reset();
                } else {
                    self.editor.backspace();
                    self.renderer
                        .invalidate_from_line(cursor_line.saturating_sub(1));
                }
                self.status_message.clear();
            }

            // ── Delete ───────────────────────────────────────────────────────
            egui::Key::Delete => {
                let cursor_line = self.editor.cursor().row;
                if modifiers.ctrl {
                    self.editor.delete_word_forward();
                    self.renderer.full_reset();
                } else if modifiers.shift {
                    // Ctrl+Shift+K equivalent on some keyboards
                    self.editor.delete_line();
                    self.renderer.full_reset();
                } else {
                    self.editor.delete();
                    self.renderer.invalidate_line(cursor_line);
                }
                self.status_message.clear();
            }

            // ── Enter ────────────────────────────────────────────────────────
            egui::Key::Enter => {
                let cursor_line = self.editor.cursor().row;
                self.editor.insert("\n");
                self.status_message.clear();
                self.renderer.invalidate_from_line(cursor_line);
            }

            // ── Ctrl shortcuts ───────────────────────────────────────────────
            egui::Key::A if modifiers.ctrl => {
                self.editor.select_all();
            }

            egui::Key::C if modifiers.ctrl => {
                let text = if let Some(selected) = self.editor.selected_text() {
                    self.clipboard_is_line = false;
                    selected
                } else {
                    // No selection — copy whole line (VS Code behaviour)
                    self.clipboard_is_line = true;
                    self.editor.current_line_text()
                };
                self.clipboard = text.clone();
                ctx.output_mut(|o| o.copied_text = text);
            }

            egui::Key::X if modifiers.ctrl => {
                let text = if let Some(selected) = self.editor.selected_text() {
                    self.clipboard_is_line = false;
                    self.editor.delete_selection();
                    self.renderer.full_reset();
                    selected
                } else {
                    // No selection — cut whole line
                    self.clipboard_is_line = true;
                    let line_text = self.editor.current_line_text();
                    self.editor.delete_line();
                    self.renderer.full_reset();
                    line_text
                };
                self.clipboard = text.clone();
                ctx.output_mut(|o| o.copied_text = text);
                self.status_message.clear();
            }

            egui::Key::V if modifiers.ctrl => {
                // Paste is handled via egui::Event::Paste in the event loop
                // to get the actual OS clipboard text. Nothing to do here.
            }

            egui::Key::Z if modifiers.ctrl => {
                if self.editor.can_undo() {
                    self.editor.undo();
                    self.status_message = "Undo".to_string();
                    self.renderer.full_reset();
                }
            }

            egui::Key::Y if modifiers.ctrl => {
                if self.editor.can_redo() {
                    self.editor.redo();
                    self.status_message = "Redo".to_string();
                    self.renderer.full_reset();
                }
            }

            egui::Key::K if modifiers.ctrl && modifiers.shift => {
                // Ctrl+Shift+K — delete line (VS Code)
                self.editor.delete_line();
                self.renderer.full_reset();
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

            _ => {}
        }

        let cursor_after = self.editor.cursor();
        if cursor_before != cursor_after {
            self.auto_scroll = true;
        }
    }

    fn do_paste(&mut self, text: String) {
        if !self.focus.is_focused(FocusTarget::Editor) {
            return;
        }

        // Check if this is a whole-line paste (we copied a line ourselves)
        let is_line_paste = self.clipboard_is_line && text == self.clipboard;

        if is_line_paste {
            // VS Code line paste: insert as a new line ABOVE the current line
            self.editor.move_to_line_start();
            self.editor.insert(&text); // text already has trailing \n
            self.editor.move_up();
        } else {
            // Regular paste — replaces selection if any
            self.editor.insert(&text);
        }

        self.renderer.full_reset();
        self.status_message.clear();
        self.auto_scroll = true;
        self.last_input_time = Instant::now();
        self.cursor_blink = true;
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
        // ── Cursor blink ──────────────────────────────────────────────────────
        let is_typing = self.last_input_time.elapsed().as_millis() < 800;
        if !is_typing && self.last_blink.elapsed().as_millis() > 500 {
            self.cursor_blink = !self.cursor_blink;
            self.last_blink = Instant::now();
        } else if is_typing {
            self.cursor_blink = true;
        }
        ctx.request_repaint();

        // ── Drain edit events → highlighter ──────────────────────────────────
        {
            let events = self.editor.drain_edit_events();
            for e in events {
                self.renderer.notify_edit(
                    self.editor.buffer().rope(),
                    e.start_byte,
                    e.old_end_byte,
                    e.new_end_byte,
                );
            }
        }

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
        ctx.input(|i| {
            for event in &i.events {
                match event {
                    egui::Event::Text(text) => {
                        self.handle_text_input(text);
                    }
                    egui::Event::Paste(text) => {
                        paste_text = Some(text.clone());
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

        if let Some(text) = paste_text {
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
                        self.renderer.full_reset();
                        ui.close_menu();
                    }
                    if ui
                        .add_enabled(self.editor.can_redo(), egui::Button::new("↷ Redo (Ctrl+Y)"))
                        .clicked()
                    {
                        self.editor.redo();
                        self.renderer.full_reset();
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("📋 Select All (Ctrl+A)").clicked() {
                        self.editor.select_all();
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
            let selection = self.editor.selection();
            let status = if !self.status_message.is_empty() {
                self.status_message.clone()
            } else if !selection.is_empty() {
                let (start, end) = selection.range();
                // Count selected chars
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
                        egui::RichText::new(self.focus.status_label())
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
                self.focus.set(FocusTarget::Editor);
            }

            let interaction = self.renderer.render_with_highlighting(
                ui,
                &self.editor,
                self.cursor_blink,
                self.auto_scroll,
            );
            self.auto_scroll = false;

            // ── Click handling ────────────────────────────────────────────
            if let Some(pos) = interaction.double_clicked_at {
                // Register double click
                let point = self.renderer.screen_to_point(pos, &self.editor);
                self.editor.set_cursor(point);
                self.editor.select_word_at_cursor();
                self.click_count = 2;
                self.last_click_time = Instant::now();
                self.auto_scroll = false;
            } else if let Some(pos) = interaction.single_clicked_at {
                let now = Instant::now();
                let elapsed = now.duration_since(self.last_click_time).as_millis();

                if self.click_count >= 2 && elapsed < 500 {
                    // Triple click
                    let point = self.renderer.screen_to_point(pos, &self.editor);
                    self.editor.set_cursor(point);
                    self.editor.select_line_at_cursor();
                    self.click_count = 0;
                } else {
                    // Single click — place cursor, clear selection
                    let point = self.renderer.screen_to_point(pos, &self.editor);
                    self.editor.set_cursor(point);
                    self.click_count = 1;
                }
                self.last_click_time = now;
                self.auto_scroll = false;
            }

            // ── Drag selection ────────────────────────────────────────────
            if interaction.drag_started {
                if let Some(pos) = interaction.dragging_at {
                    let point = self.renderer.screen_to_point(pos, &self.editor);
                    self.editor.set_cursor(point);
                    self.auto_scroll = false;
                }
            } else if let Some(pos) = interaction.dragging_at {
                // Extend selection while dragging
                let drag_point = self.renderer.screen_to_point(pos, &self.editor);
                let anchor = self.editor.selection().start;
                self.editor
                    .set_selection(crate::editor::selection::Selection::new(anchor, drag_point));
                self.auto_scroll = false;
            }
        });
    }
}
