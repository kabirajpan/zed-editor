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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SidebarTab {
    Investigator,
    Chat,
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

    // AI Simulation
    ai_stream_active: bool,
    ai_stream_tokens: Vec<String>,
    ai_stream_index: usize,
    ai_stream_timer: f32,
    
    // Layer 1 Investigation
    show_debug_panel: bool,

    // Real AI Provider
    ai_provider_type: crate::ai::provider::ProviderType,
    ai_api_key: String,
    ai_receiver: Option<tokio::sync::mpsc::UnboundedReceiver<String>>,

    // Layer 1.1: Proper Hardening
    last_pie_sync: Instant,

    // Phase 2: AI Chat Sidecar
    sidebar_tab: SidebarTab,
    chat_history: Vec<crate::ai::chat::ChatMessage>,
    chat_input: String,
    chat_receiver: Option<tokio::sync::mpsc::UnboundedReceiver<String>>,
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
            ai_stream_active: false,
            ai_stream_tokens: Vec::new(),
            ai_stream_index: 0,
            ai_stream_timer: 0.0,
            show_debug_panel: false,
            ai_provider_type: crate::ai::provider::ProviderType::Anthropic,
            ai_api_key: String::new(),
            ai_receiver: None,
            last_pie_sync: Instant::now(),
            sidebar_tab: SidebarTab::Chat,
            chat_history: Vec::new(),
            chat_input: String::new(),
            chat_receiver: None,
        }
    }

    fn handle_text_input(&mut self, text: &str) {
        if !self.manager.focus.is_focused(FocusTarget::Editor) {
            return;
        }

        let cursor_line = self.editor.cursor().row;
        self.editor.insert(text);

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
            // ── Save / Sync PIE (Ctrl+S) ──────────────────────────────────────────
        if key == egui::Key::S && modifiers.command {
            if let Some(tree) = self.renderer.highlighter.tree() {
                let text = self.editor.buffer().to_string();
                self.editor.update_semantic_deltas(tree, &text);
                self.status_message = format!("📊 PIE Sync Complete: {} deltas found", self.editor.last_semantic_deltas().len());
            } else {
                self.status_message = "⚠️ Save: No syntax tree available for PIE sync".to_string();
            }
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

        // ── Layer 1 Investigator Shortcut (Ctrl+I) ───────────────────────────
        if key == egui::Key::I && modifiers.command {
            self.show_debug_panel = !self.show_debug_panel;
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
                if modifiers.alt && modifiers.shift {
                    self.editor.duplicate_lines_up();
                } else if modifiers.alt {
                    self.editor.move_lines_up();
                } else if modifiers.shift {
                    self.editor.extend_selection_up();
                } else {
                    self.editor.move_up();
                }
            }
            egui::Key::ArrowDown => {
                if modifiers.alt && modifiers.shift {
                    self.editor.duplicate_lines_down();
                } else if modifiers.alt {
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

            egui::Key::A if modifiers.ctrl && modifiers.shift => {
                self.trigger_ai();
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

            egui::Key::L if modifiers.ctrl => {
                self.show_debug_panel = true;
                self.sidebar_tab = SidebarTab::Chat;
                // Focus the chat input on next frame
                _ctx.memory_mut(|mem| mem.request_focus(egui::Id::new("chat_input")));
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

                // ── Initialize PIE Baseline ───────────────────────────────────
                if let Some(tree) = self.renderer.highlighter.tree() {
                    self.editor.sync_semantic_checkpoint(tree, &contents);
                }

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

        // ── Autonomous PIE (Background Sync) ────────────────────────────────
        if self.last_input_time.elapsed().as_millis() > 500 && self.last_pie_sync < self.last_input_time {
            if let Some(tree) = self.renderer.highlighter.tree() {
                let text = self.editor.buffer().to_string();
                self.editor.update_semantic_deltas(tree, &text);
                self.last_pie_sync = Instant::now();
            }
        }

        // ── Real AI Token Handling (Editor) ──────────────────────────────────
        if let Some(ref mut rx) = self.ai_receiver {
            while let Ok(token) = rx.try_recv() {
                self.editor.insert_ai_stream(&token);
                self.renderer.invalidate_from_line(self.editor.cursor().row);
                self.auto_scroll = true;
            }
        }

        // ── AI Sidecar Token Handling (Chat) ──────────────────────────────────
        if let Some(ref mut rx) = self.chat_receiver {
            while let Ok(token) = rx.try_recv() {
                if let Some(last_msg) = self.chat_history.last_mut() {
                    if last_msg.role == crate::ai::chat::MessageRole::Assistant {
                        last_msg.content.push_str(&token);
                    } else {
                        self.chat_history.push(crate::ai::chat::ChatMessage::assistant(token));
                    }
                } else {
                    self.chat_history.push(crate::ai::chat::ChatMessage::assistant(token));
                }
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

        let mut paste_text: Option<String> = None;
        let mut do_copy = false;
        let mut do_cut = false;
        let mut keys_to_handle: Vec<(egui::Key, egui::Modifiers)> = Vec::new();
        ctx.input(|i| {
            if !i.events.is_empty() {
                self.last_input_time = Instant::now();
            }

            for event in &i.events {
                match event {
                    egui::Event::Text(text) => {
                        // 🛡️ Filter out control characters that shouldn't come through Text events
                        // (Tab and Enter are handled via Event::Key)
                        if text != "\t" && text != "\r" && text != "\n" {
                            self.handle_text_input(text);
                        }
                    }
                    egui::Event::Paste(text) => {
                        paste_text = Some(text.clone());
                    }
                    egui::Event::Copy => {
                        do_copy = true;
                    }
                    egui::Event::Cut => {
                        do_cut = true;
                    }
                    egui::Event::Key { key, pressed: true, modifiers, .. } => {
                        if *key != egui::Key::Tab {
                            keys_to_handle.push((*key, *modifiers));
                        }
                    }
                    _ => {}
                }
            }
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

        // ── Layer 1 Investigator (Right Side Panel) ──────────────────────────
        self.show_layer1_investigator(ctx);

        // ── Status bar ────────────────────────────────────────────────────────
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            let cursor = self.editor.cursor();
            let byte_offset = self.editor.buffer().point_to_offset(cursor).value();
            let semantic_context = self.renderer.get_code_context(byte_offset);

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
                ui.label(&status);
                
                ui.separator();
                
                if let Some(ctx) = semantic_context {
                    let mut semantic_info = Vec::new();
                    if let Some(f) = ctx.function_name {
                        semantic_info.push(format!("λ {}", f));
                    }
                    if let Some(s) = ctx.struct_name {
                        semantic_info.push(format!("⬢ {}", s));
                    }
                    
                    if !semantic_info.is_empty() {
                        ui.label(egui::RichText::new(semantic_info.join(" > ")).color(egui::Color32::from_rgb(150, 200, 150)).strong());
                        ui.separator();
                    }
                    ui.label(egui::RichText::new(&ctx.scope_path).color(egui::Color32::from_rgb(140, 140, 140)).small());
                }

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

        // ── AI Streaming Logic ───────────────────────────────────────────────
        if self.ai_stream_active {
            self.ai_stream_timer += ctx.input(|i| i.stable_dt);
            
            // 🚀 BLAZING FAST: Process multiple tokens in one frame if needed
            while self.ai_stream_timer > 0.015 { 
                self.ai_stream_timer -= 0.015;
                if self.ai_stream_index < self.ai_stream_tokens.len() {
                    let token = self.ai_stream_tokens[self.ai_stream_index].clone();
                    let cursor_line = self.editor.cursor().row;
                    self.editor.insert_ai_stream(&token);
                    self.renderer.invalidate_from_line(cursor_line);
                    self.ai_stream_index += 1;
                    ctx.request_repaint();
                } else {
                    self.editor.finish_ai_stream();
                    self.ai_stream_active = false;
                    self.status_message = "✅ AI Stream Complete".to_string();
                    break;
                }
            }
        }
    }
}

impl GuiApp {
    fn start_ai_simulation(&mut self) {
        if self.ai_stream_active { return; }
        
        let line_text = self.editor.current_line_text();
        let is_empty_line = line_text.trim().is_empty();

        self.ai_stream_active = true;
        self.ai_stream_index = 0;
        self.ai_stream_timer = 0.0;
        self.status_message = "🤖 AI is thinking...".to_string();
        
        // Mock Rust snippet for simulation
        let mut snippet = "fn calculate_factorial(n: u64) -> u64 {\n    if n == 0 {\n        1\n    } else {\n        n * calculate_factorial(n - 1)\n    }\n}".to_string();
        
        // Intelligent Spacing: Ensure we start on a new line if current line isn't empty
        if !is_empty_line {
            snippet = format!("\n\n{}", snippet);
        }

        // Tokenize by word-ish / character chunks for smooth streaming
        self.ai_stream_tokens = snippet.split_inclusive(|c: char| c.is_whitespace() || c == '(' || c == ')' || c == '{' || c == '}' || c == ':')
            .map(|s| s.to_string())
            .collect();
            
        self.editor.start_ai_stream();
    }

    fn trigger_ai(&mut self) {
        let needs_key = self.ai_provider_type == crate::ai::provider::ProviderType::Anthropic || 
                        self.ai_provider_type == crate::ai::provider::ProviderType::Grok ||
                        self.ai_provider_type == crate::ai::provider::ProviderType::Groq;
        
        if needs_key && self.ai_api_key.is_empty() {
            self.start_ai_simulation();
            self.status_message = format!("⚠️ No API Key found for {:?}. Using Mock Simulation.", self.ai_provider_type);
        } else {
            // 🔱 Layer 1.5: Intelligent Spacing Guard
            // Ensure we start on a new line if current line isn't empty (e.g. after a comment)
            let line_text = self.editor.current_line_text();
            if !line_text.trim().is_empty() {
                self.editor.insert("\n\n");
                self.renderer.invalidate_from_line(self.editor.cursor().row);
            }

            let cursor = self.editor.cursor();
            let offset = self.editor.buffer().point_to_offset(cursor).value();
            let text = self.editor.buffer().to_string();
            let (prefix, suffix) = text.split_at(offset);
            
            self.start_real_ai_stream(prefix, suffix);
        }
    }

    fn start_real_ai_stream(&mut self, prefix: &str, suffix: &str) {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        self.ai_receiver = Some(rx);
        self.editor.start_ai_stream();
        self.status_message = format!("🤖 AI ({:?}) is thinking...", self.ai_provider_type);

        let system_prompt = if let Some(tree) = self.renderer.highlighter.tree() {
            let cursor = self.editor.cursor();
            let offset = self.editor.buffer().point_to_offset(cursor).value();
            let node_path = self.editor.get_node_path_at(tree, offset);
            
            format!("You are a high-performance code completion engine for the Z3N editor.
You are currently coding inside this semantic path: {}
Output ONLY the raw code required to complete the user's intent based on the context.
NO EXPLANATIONS. NO MARKDOWN. NO INTRODUCTIONS.
If the code is already complete, output nothing.", node_path)
        } else {
            "You are a high-performance code completion engine for the Z3N editor.
Output ONLY the raw code required to complete the user's intent based on the context.
NO EXPLANATIONS. NO MARKDOWN. NO INTRODUCTIONS.
If the code is already complete, output nothing.".to_string()
        };

        let user_prompt = format!("### PREFIX\n{}\n### CURSOR HERE\n### SUFFIX\n{}", prefix, suffix);

        let provider: Box<dyn crate::ai::provider::ModelProvider> = match self.ai_provider_type {
            crate::ai::provider::ProviderType::Anthropic => Box::new(crate::ai::provider::AnthropicProvider),
            crate::ai::provider::ProviderType::Ollama => Box::new(crate::ai::provider::OllamaProvider),
            crate::ai::provider::ProviderType::Grok => Box::new(crate::ai::provider::GrokProvider),
            crate::ai::provider::ProviderType::Groq => Box::new(crate::ai::provider::GroqProvider),
        };

        provider.stream_completion(system_prompt, user_prompt, self.ai_api_key.clone(), tx);
    }

    fn show_layer1_investigator(&mut self, ctx: &egui::Context) {
        if !self.show_debug_panel { return; }

        egui::SidePanel::right("investigator")
            .resizable(true)
            .default_width(320.0)
            .show(ctx, |ui| {
                ui.add_space(8.0);
                
                // ── Tab Bar ──────────────────────────────────────────────────
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.sidebar_tab, SidebarTab::Chat, "🤖 Chat");
                    ui.selectable_value(&mut self.sidebar_tab, SidebarTab::Investigator, "🔍 Investigator");
                });
                ui.separator();

                match self.sidebar_tab {
                    SidebarTab::Investigator => self.render_investigator_tab(ui),
                    SidebarTab::Chat => self.render_chat_tab(ui),
                }
            });
    }

    fn render_chat_tab(&mut self, ui: &mut egui::Ui) {
        ui.vertical(|ui| {
            // ── Message Area (Top, flexible) ─────────────────────────────────
            let available_height = ui.available_height();
            let input_area_height = 80.0; 
            
            egui::ScrollArea::vertical()
                .max_height(available_height - input_area_height)
                .auto_shrink([false; 2])
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    if self.chat_history.is_empty() {
                        ui.vertical_centered(|ui| {
                            ui.add_space(20.0);
                            ui.label(egui::RichText::new("🤖 Z3N Intelligence Agent").strong().size(18.0));
                            ui.label(egui::RichText::new("How can I help you plan your code today?").weak());
                        });
                    }

                    for msg in &self.chat_history {
                        let is_user = msg.role == crate::ai::chat::MessageRole::User;
                        let bg = if is_user { 
                            egui::Color32::from_rgb(50, 60, 110) // Deep Blue
                        } else { 
                            egui::Color32::from_rgb(35, 35, 45) // Slate
                        };

                        ui.horizontal(|ui| {
                            if is_user { ui.add_space(32.0); }
                            
                            egui::Frame::none()
                                .fill(bg)
                                .rounding(12.0)
                                .inner_margin(10.0)
                                .show(ui, |ui| {
                                    ui.set_max_width(ui.available_width() - 32.0);
                                    ui.label(egui::RichText::new(&msg.content).color(egui::Color32::WHITE));
                                });
                            
                            if !is_user { ui.add_space(32.0); }
                        });
                        ui.add_space(8.0);
                    }
                });

            ui.separator();

            // ── Input Area (Bottom, fixed height) ────────────────────────────
            ui.add_space(4.0);
            let input_response = ui.add(
                egui::TextEdit::multiline(&mut self.chat_input)
                    .hint_text("Send a message (Ctrl+Enter to send)...")
                    .id(egui::Id::new("chat_input"))
                    .desired_rows(2)
                    .margin(egui::vec2(8.0, 8.0))
                    .lock_focus(true)
            );

            ui.horizontal(|ui| {
                ui.small("Shift+Enter for newline");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("🚀 Send").clicked() || 
                       (input_response.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter) && i.modifiers.command)) {
                        if !self.chat_input.trim().is_empty() {
                            self.trigger_chat();
                        }
                    }
                });
            });
        });
    }

    fn trigger_chat(&mut self) {
        let text = std::mem::take(&mut self.chat_input);
        self.chat_history.push(crate::ai::chat::ChatMessage::user(text));
        
        // Add system context if first message
        if self.chat_history.len() == 1 {
            let context = "You are a helpful AI assistant inside the Z3N code editor. Use the provided code context to answer accurately.";
            self.chat_history.insert(0, crate::ai::chat::ChatMessage::system(context));
        }

        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        self.chat_receiver = Some(rx);

        let provider: Box<dyn crate::ai::provider::ModelProvider> = match self.ai_provider_type {
            crate::ai::provider::ProviderType::Anthropic => Box::new(crate::ai::provider::AnthropicProvider),
            crate::ai::provider::ProviderType::Ollama => Box::new(crate::ai::provider::OllamaProvider),
            crate::ai::provider::ProviderType::Grok => Box::new(crate::ai::provider::GrokProvider),
            crate::ai::provider::ProviderType::Groq => Box::new(crate::ai::provider::GroqProvider),
        };

        let history = self.chat_history.clone();
        provider.stream_chat(history, self.ai_api_key.clone(), tx);
    }

    fn render_investigator_tab(&mut self, ui: &mut egui::Ui) {
        ui.heading("🔍 Layer 1 Investigator");
        ui.add_space(8.0);

                // ── Provenance ──────────────────────────────────────────────
                ui.collapsing("🧬 Provenance (Authorship)", |ui| {
                    let cursor = self.editor.cursor();
                    let offset = self.editor.buffer().point_to_offset(cursor).value();
                    // Interrogate the character to the left of the cursor (standard provenance behavior)
                    let interrogate_offset = offset.saturating_sub(1);
                    let (author, timestamp) = self.editor.provenance_at(interrogate_offset);

                    ui.horizontal(|ui| {
                        ui.label("Cursor Origin:");
                        let (text, color) = match author {
                            crate::history::transaction::Author::Human => ("HUMAN", egui::Color32::from_rgb(100, 255, 100)),
                            crate::history::transaction::Author::AiSuggested => ("AI_SUGGESTED", egui::Color32::from_rgb(255, 180, 0)),
                            crate::history::transaction::Author::AiModified => ("AI_MODIFIED", egui::Color32::from_rgb(255, 100, 100)),
                        };
                        ui.label(egui::RichText::new(text).color(color).strong());
                    });

                    if let Some(ts) = timestamp {
                        let elapsed = ts.elapsed().as_secs();
                        let time_str = if elapsed < 60 {
                            format!("{}s ago", elapsed)
                        } else {
                            format!("{}m ago", elapsed / 60)
                        };
                        ui.label(egui::RichText::new(format!("Modified: {}", time_str)).small().weak());
                    }

                    ui.add_space(4.0);
                    ui.label(egui::RichText::new("Interrogating the byte under the cursor to find its birth certificate.").small().italics());
                });

                ui.add_space(12.0);

                // ── PIE (Semantic Deltas) ───────────────────────────────────
                ui.collapsing("📊 PIE (Semantic Deltas)", |ui| {
                    let deltas = self.editor.last_semantic_deltas();
                    ui.label(format!("Active Deltas: {}", deltas.len()));
                    
                    egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                        if deltas.is_empty() {
                            ui.label(egui::RichText::new("No recent changes. Press Ctrl+S to sync PIE.").weak());
                        } else {
                            for delta in deltas {
                                ui.horizontal(|ui| {
                                    ui.label(egui::RichText::new(format!("[{:?}]", delta.edit_type)).strong());
                                    ui.label(&delta.node_path);
                                });
                                let range_text = if let Some((s, e)) = delta.new_byte_range {
                                    format!("Bytes: {}..{}", s, e)
                                } else if let Some((s, e)) = delta.old_byte_range {
                                    format!("Deleted Bytes: {}..{}", s, e)
                                } else {
                                    "Range: Unknown".to_string()
                                };
                                ui.label(egui::RichText::new(range_text).small().weak());
                                ui.separator();
                            }
                        }
                    });
                    
                    if ui.button("Sync Now (PIE)").clicked() {
                        // Trigger manual sync logic
                        if let Some(tree) = self.renderer.highlighter.tree() {
                            let text = self.editor.buffer().to_string();
                            self.editor.update_semantic_deltas(tree, &text);
                        }
                    }
                });

                ui.add_space(12.0);

                // ── Performance / Rope ──────────────────────────────────────
                ui.collapsing("⚙️ High-Performance Rope", |ui| {
                    ui.label(format!("Buffer Length: {} bytes", self.editor.buffer().len()));
                    ui.label(format!("Line Count: {}", self.editor.line_count()));
                    ui.label("Memory: O(1) edits, O(log N) splits.");
                });

                ui.add_space(12.0);

                // ── AI Settings ──────────────────────────────────────────────
                ui.collapsing("🤖 AI Settings", |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Provider:");
                        egui::ComboBox::from_id_source("ai_provider")
                            .selected_text(format!("{:?}", self.ai_provider_type))
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut self.ai_provider_type, crate::ai::provider::ProviderType::Anthropic, "Anthropic (Claude)");
                                ui.selectable_value(&mut self.ai_provider_type, crate::ai::provider::ProviderType::Ollama, "Ollama (Local)");
                                ui.selectable_value(&mut self.ai_provider_type, crate::ai::provider::ProviderType::Grok, "Grok (xAI)");
                                ui.selectable_value(&mut self.ai_provider_type, crate::ai::provider::ProviderType::Groq, "Groq (LPU Speed)");
                            });
                    });

                    let needs_key = self.ai_provider_type == crate::ai::provider::ProviderType::Anthropic || 
                                    self.ai_provider_type == crate::ai::provider::ProviderType::Grok ||
                                    self.ai_provider_type == crate::ai::provider::ProviderType::Groq;

                    if needs_key {
                        ui.horizontal(|ui| {
                            ui.label("API Key:");
                            let hint = match self.ai_provider_type {
                                crate::ai::provider::ProviderType::Anthropic => "sk-ant-...",
                                crate::ai::provider::ProviderType::Grok => "xai-...",
                                crate::ai::provider::ProviderType::Groq => "gsk-...",
                                _ => "api-key",
                            };
                            ui.add(egui::TextEdit::singleline(&mut self.ai_api_key).password(true).hint_text(hint));
                        });
                    } else {
                        ui.label(egui::RichText::new("Ensure Ollama is running at localhost:11434").small().color(egui::Color32::from_rgb(150, 150, 150)));
                    }
                    
                    ui.add_space(4.0);
                    if ui.button("🚀 Trigger AI Suggestion").clicked() {
                        self.trigger_ai();
                    }
                });

    }
}
