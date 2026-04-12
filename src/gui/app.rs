use crate::editor::selection::{Selection, SelectionMode};
use crate::formatter::providers::{PrettierProvider, RustfmtProvider};
use crate::io::write_file_from_rope;
use crate::{read_file, Editor, Formatter, SyntaxHighlighter, SyntaxTheme};
use std::path::PathBuf;
use std::time::Instant;
use tokio::sync::mpsc;

use crate::manager::{FocusTarget, GlobalManager};
use super::viewport_renderer::ViewportRenderer;

#[derive(Clone, Debug)]
enum LoadingState {
    Idle,
    Loading { progress: f32, message: String },
    Complete,
    Error(String),
}

#[derive(Debug, Clone)]
pub struct NepHint {
    pub text: String,
    pub anchor: crate::buffer::Point,
    pub version: u64,
    pub line_count: usize,
}


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AiPermissionPool {
    pub read_granted: bool,
    pub write_granted: bool,
}

impl AiPermissionPool {
    pub fn none() -> Self {
        Self { read_granted: false, write_granted: false }
    }
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
    
    // Real AI Provider (for inline completions & NEP)
    ai_provider_type: crate::ai::provider::ProviderType,
    ai_api_key: String,
    ai_receiver: Option<tokio::sync::mpsc::UnboundedReceiver<String>>,
    
    // NEP: Next Edit Prediction
    current_nep_hint: Option<NepHint>,
    nep_receiver: Option<tokio::sync::mpsc::UnboundedReceiver<NepHint>>,
    nep_loading: bool,
    nep_start_time: Option<Instant>,
    
    // 🔱 Layer 3: Mason & LSP logic
    pub mason: crate::ai::mason::MasonManager,
    pub lsp_install_prompt: Option<String>,
    pub skipped_lsps: Vec<String>,
    pub mason_receiver: Option<tokio::sync::mpsc::UnboundedReceiver<crate::ai::mason::MasonEvent>>,
    pub mason_sender: tokio::sync::mpsc::UnboundedSender<crate::ai::mason::MasonEvent>,

    // Layer 1.1: Proper Hardening
    last_pie_sync: Instant,

    // Phase 3: Multi-Panel System
    pub panel_manager: crate::gui::panels::PanelManager,

    // Phase 4: Security Layer
    pub ai_permissions: AiPermissionPool,
}

impl GuiApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let mut formatter = Formatter::new();
        formatter.register(Box::new(RustfmtProvider::new()));
        formatter.register(Box::new(PrettierProvider::new()));

        let highlighter = SyntaxHighlighter::new(SyntaxTheme::dark());
        let mut renderer = ViewportRenderer::new();
        renderer.highlighter = highlighter;

        let (m_tx, m_rx) = mpsc::unbounded_channel();
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
            ai_provider_type: crate::ai::provider::ProviderType::Anthropic,
            ai_api_key: String::new(),
            ai_receiver: None,
            current_nep_hint: None,
            nep_receiver: None,
            nep_loading: false,
            nep_start_time: None,
            mason: crate::ai::mason::MasonManager::new(),
            lsp_install_prompt: None,
            skipped_lsps: Vec::new(),
            mason_receiver: Some(m_rx),
            mason_sender: m_tx,
            last_pie_sync: Instant::now(),
            panel_manager: crate::gui::panels::PanelManager::new(),
            ai_permissions: AiPermissionPool::none(),
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
        // ── Tab (focus cycling OR editor indent) ──────────────────────────────
        if key == egui::Key::Tab {
            // 🔱 Strict Buffer Focus: Tab is ONLY for the Editor.
            // Default focus navigation (panel cycling) is disabled to prevent "global tab" leaks.
            if !self.manager.focus.is_focused(FocusTarget::Editor) {
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
            self.panel_manager.right_panel.is_visible = !self.panel_manager.right_panel.is_visible;
            if self.panel_manager.right_panel.is_visible {
               self.manager.focus.set(crate::manager::FocusTarget::RightPanel);
               self.manager.panels.right_open = true;
            } else {
               self.manager.panels.right_open = false;
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
                self.panel_manager.right_panel.is_visible = true;
                self.panel_manager.right_panel.active_tab = crate::gui::panels::right::RightPanelTab::Chat;
                self.manager.panels.right_open = true;
                self.manager.focus.set(crate::manager::FocusTarget::RightPanel);
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
        let mut tab_pressed = false;
        let mut shift_tab_pressed = false;

        // 🔱 User-Docs Pattern: Use consume_shortcut to reliably hijack Tab/Shift+Tab.
        // This prevents egui from performing global focus navigation.
        let tab_shortcut = egui::KeyboardShortcut::new(egui::Modifiers::NONE, egui::Key::Tab);
        let shift_tab_shortcut = egui::KeyboardShortcut::new(egui::Modifiers::SHIFT, egui::Key::Tab);

        ctx.input_mut(|i| {
            if i.consume_shortcut(&tab_shortcut) {
                tab_pressed = true;
            }
            if i.consume_shortcut(&shift_tab_shortcut) {
                shift_tab_pressed = true;
            }
        });

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

        
        // 🔱 Mason Event Handling
        if let Some(ref mut rx) = self.mason_receiver {
            while let Ok(event) = rx.try_recv() {
                match event {
                    crate::ai::mason::MasonEvent::Progress(name, p) => {
                        self.mason.set_status(&name, crate::ai::mason::LspServiceStatus::Downloading(p));
                    }
                    crate::ai::mason::MasonEvent::Complete(name) => {
                        self.mason.set_status(&name, crate::ai::mason::LspServiceStatus::Installed);
                        self.status_message = format!("✅ {} installed successfully!", name);
                    }
                    crate::ai::mason::MasonEvent::Error(name, err) => {
                        self.mason.set_status(&name, crate::ai::mason::LspServiceStatus::Error(err));
                    }
                }
            }
        }

        // 🔱 NEP Stall Recovery & Receiver
        if self.nep_loading {
            if let Some(start) = self.nep_start_time {
                if start.elapsed().as_secs_f32() > 5.0 {
                    self.nep_loading = false;
                    self.status_message = "⚠️ NEP request timed out. Resetting...".to_string();
                }
            }
        }

        if let Some(ref mut rx) = self.nep_receiver {
            while let Ok(mut hint) = rx.try_recv() {
                if hint.version == self.editor.version() {
                    // 🔱 Layer 2: Stitching Logic (Remove duplicate prefixes) - Longest Overlap Detection
                    let last_line = self.editor.buffer().line(hint.anchor.row).unwrap_or_default();
                    let prefix_segment = last_line[..hint.anchor.column.min(last_line.len())].to_string();
                    
                    if !prefix_segment.trim().is_empty() {
                        let overlap_len = find_longest_overlap(&prefix_segment, &hint.text);
                        if overlap_len > 0 {
                            hint.text = hint.text[overlap_len..].to_string();
                        }
                    }

                    self.current_nep_hint = Some(hint);
                    self.nep_loading = false;
                    self.nep_start_time = None;
                }
            }
        }

        // 🔱 NEP Stale Check: Clear hint if version changed or cursor moved
        if let Some(hint) = &self.current_nep_hint {
            if hint.version != self.editor.version() || hint.anchor != self.editor.cursor() {
                self.current_nep_hint = None;
            }
        }
        
        // 🔱 NEP Idle Trigger: If no hint and idle for 1.5s
        if self.current_nep_hint.is_none() 
            && !self.nep_loading 
            && !self.ai_api_key.is_empty() 
            && self.last_input_time.elapsed().as_secs_f32() > 1.5 
            && !self.editor.is_speculative_active() 
        {
            self.trigger_nep();
        }

        // 🔱 Layer 3: LSP Auto-Detection Logic
        if let Some(file) = &self.current_file {
            if let Some(ext_str) = file.extension().and_then(|e| e.to_str()) {
                let mut target_lsp = None;
                for (name, ext_cfg) in &self.mason.registry {
                    if ext_cfg.supported_extensions.contains(&ext_str.to_string()) {
                        target_lsp = Some(name.clone());
                        break;
                    }
                }

                if let Some(lsp_name) = target_lsp {
                    if self.mason.get_status(&lsp_name) == crate::ai::mason::LspServiceStatus::NotInstalled 
                        && !self.skipped_lsps.contains(&lsp_name) 
                        && self.lsp_install_prompt.is_none() 
                    {
                        self.lsp_install_prompt = Some(lsp_name);
                    }
                }
            }
        }

        // ── Real AI Token Handling (Layer 2: Speculative Transaction) ────────
        if let Some(ref mut rx) = self.ai_receiver {
            while let Ok(token) = rx.try_recv() {
                if !self.editor.is_speculative_active() {
                    self.editor.start_speculative_session();
                }
                self.editor.insert_ai_stream(&token);
                self.renderer.invalidate_from_line(self.editor.cursor().row);
                self.auto_scroll = true;
                self.status_message = "🤖 AI Drafting... [Enter] Commit, [Esc] Rollback".to_string();
            }
        }


        let mut enter_pressed_speculative = false;
        
        ctx.input_mut(|i| {
            if i.consume_key(egui::Modifiers::NONE, egui::Key::Escape) {
                if self.current_nep_hint.is_some() {
                    self.current_nep_hint = None;
                    self.status_message = "NEP Prediction dismissed.".to_string();
                } else if self.editor.is_speculative_active() {
                    self.editor.rollback_speculative();
                    self.status_message = "AI suggestion rolled back.".to_string();
                } else if self.editor.ai_ghost_text.is_some() {
                    self.editor.discard_ai_suggestion();
                    self.status_message = "AI suggestion discarded.".to_string();
                }
            }
            
            // Only consume Enter if we are in a Review Session
            if self.editor.is_speculative_active() {
                if i.consume_key(egui::Modifiers::NONE, egui::Key::Enter) {
                    enter_pressed_speculative = true;
                }
            }
        });

        if enter_pressed_speculative {
            self.editor.commit_speculative();
            self.status_message = "✨ AI changes committed.".to_string();
            ctx.memory_mut(|m| {
                if let Some(id) = m.focused() {
                    m.surrender_focus(id);
                }
            });
        }

        if tab_pressed {
            if let Some(hint) = self.current_nep_hint.take() {
                if hint.version == self.editor.version() {
                    self.editor.insert(&hint.text);
                    self.status_message = "✨ NEP Prediction applied.".to_string();
                    self.last_input_time = Instant::now();
                }
            } else if self.editor.is_speculative_active() {
                self.editor.commit_speculative();
                self.status_message = "✨ AI changes committed.".to_string();
            } else if self.editor.ai_ghost_text.is_some() {
                self.editor.accept_ai_suggestion();
                self.status_message = "✨ Suggestion applied.".to_string();
            } else {
                self.handle_key(egui::Key::Tab, egui::Modifiers::NONE, ctx);
            }
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
                        // Only handle if NO UI widget has focus (e.g. Chat Input)
                        // 🛡️ Lenient Input Guard: Only block Editor if another panel (like Chat) is explicitly active
                        let is_chat_active = self.manager.focus.is_focused(crate::manager::FocusTarget::RightPanel);
                        if !is_chat_active && text != "\t" && text != "\r" && text != "\n" {
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

        if !self.manager.focus.is_focused(crate::manager::FocusTarget::RightPanel) {
            for (key, modifiers) in keys_to_handle {
                self.handle_key(key, modifiers, ctx);
            }
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

        egui::TopBottomPanel::top("menu").show(ctx, |ui| {
            // ── AI Review Bar (Layer 2) ──────────────────────────────────────
            let ghost_lines = self.editor.ai_ghost_text.as_ref().map(|g| g.lines().count());
            
            if let Some(lines) = ghost_lines {
                egui::Frame::none()
                    .fill(egui::Color32::from_rgb(40, 50, 90))
                    .inner_margin(4.0)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("🤖 AI Draft ready").color(egui::Color32::WHITE).strong());
                            ui.add_space(8.0);
                            
                            if ui.button("✅ [Tab] Accept").clicked() {
                                self.editor.accept_ai_suggestion();
                            }
                            if ui.button("❌ [Esc] Discard").clicked() {
                                self.editor.discard_ai_suggestion();
                            }
                            
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.label(egui::RichText::new(format!("{} lines proposed", lines)).weak());
                            });
                        });
                    });
                ui.separator();
            }

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

        // ── Multi-Panel System Render ────────────────────────────────────────
        self.panel_manager.right_panel.render(
            ctx, 
            &mut self.editor, 
            &mut self.ai_provider_type, 
            &mut self.ai_api_key,
            self.renderer.highlighter.tree(),
            &mut self.manager,
            &mut self.ai_permissions,
            &mut self.mason,
            &self.mason_sender,
        );

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
                self.current_nep_hint.as_ref(), // 🔱 Pass NEP Hint
            );

            self.auto_scroll = false;
            
            // ── Layer 2: Speculative Transaction Review Bar ──────────────
            if self.editor.is_speculative_active() {
                egui::Area::new(egui::Id::new("review_bar"))
                    .anchor(egui::Align2::CENTER_BOTTOM, egui::vec2(0.0, -40.0))
                    .show(ctx, |ui| {
                        egui::Frame::none()
                            .fill(egui::Color32::from_rgb(30, 30, 40))
                            .rounding(10.0)
                            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(100, 150, 255)))
                            .inner_margin(8.0)
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(egui::RichText::new("🤖 AI Proposal ready").strong().color(egui::Color32::WHITE));
                                    ui.separator();
                                    
                                    if ui.button(egui::RichText::new("✅ Accept (Enter)").color(egui::Color32::GREEN)).clicked() {
                                        self.editor.commit_speculative();
                                        self.status_message = "✨ AI changes committed.".to_string();
                                    }
                                    
                                    if ui.button(egui::RichText::new("❌ Discard (Esc)").color(egui::Color32::LIGHT_RED)).clicked() {
                                        self.editor.rollback_speculative();
                                        self.status_message = "AI suggestion rolled back.".to_string();
                                    }
                                });
                            });
                    });
            }

            // ── LSP Install Prompt Overlay ────────────────────────────────
            if let Some(lsp_name) = self.lsp_install_prompt.clone() {
                egui::Window::new("📥 Missing LSP Tool")
                    .anchor(egui::Align2::CENTER_BOTTOM, [0.0, -100.0])
                    .collapsible(false)
                    .resizable(false)
                    .show(ctx, |ui| {
                        ui.label(format!("Z3N detected you are working with a new file type. Would you like to install [{}] for instant, local intelligence?", lsp_name));
                        ui.horizontal(|ui| {
                            if ui.button(egui::RichText::new("✅ Install Now").strong()).clicked() {
                                if let Some(ext) = self.mason.registry.get_mut(&lsp_name) {
                                    ext.status = crate::ai::mason::LspServiceStatus::Downloading(0.0);
                                    self.mason.trigger_install(lsp_name.clone(), self.mason_sender.clone());
                                }
                                self.lsp_install_prompt = None;
                            }
                            if ui.button("❌ Skip for now").clicked() {
                                self.skipped_lsps.push(lsp_name);
                                self.lsp_install_prompt = None;
                            }
                        });
                    });
            }

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

/// 🔱 Layer 2: Suffix-Prefix Overlap Detection
/// Finds the longest suffix of 'prefix' that is also a prefix of 'suggestion'.
fn find_longest_overlap(prefix: &str, suggestion: &str) -> usize {
    let prefix = prefix.trim_end();
    let suggestion = suggestion.trim_start();
    
    if prefix.is_empty() || suggestion.is_empty() {
        return 0;
    }

    let mut overlap = 0;
    let max_possible = prefix.len().min(suggestion.len());

    for len in (1..=max_possible).rev() {
        let prefix_suffix = &prefix[prefix.len() - len..];
        let suggestion_prefix = &suggestion[..len];
        if prefix_suffix == suggestion_prefix {
            overlap = len;
            break;
        }
    }

    // Secondary Check: If the suggestion starts with the prefix exactly (case insensitive / trimmed)
    if overlap == 0 {
        let p_trim = prefix.trim();
        let s_trim = suggestion.trim();
        if s_trim.starts_with(p_trim) && !p_trim.is_empty() {
            // Find where p_trim ends in the original suggestion
            if let Some(pos) = suggestion.find(p_trim) {
                return pos + p_trim.len();
            }
        }
    }

    overlap
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
        } else if !self.ai_permissions.read_granted || !self.ai_permissions.write_granted {
            self.status_message = "🔒 Access Denied: Z3N requires both READ and WRITE permissions for inline AI. Please grant them in the Chat Panel.".to_string();
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

    fn trigger_nep(&mut self) {
        if self.ai_api_key.is_empty() { return; }
        self.nep_loading = true;
        self.nep_start_time = Some(Instant::now());
        
        let cursor = self.editor.cursor();
        let offset = self.editor.buffer().point_to_offset(cursor).value();
        let text = self.editor.buffer().to_string();
        let version = self.editor.version();
        
        // Split context
        let (prefix, suffix) = text.split_at(offset);
        let prefix = prefix.to_string();
        let suffix = suffix.to_string();
        
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        self.nep_receiver = Some(rx);
        
        let provider_type = self.ai_provider_type;
        let api_key = self.ai_api_key.clone();
        
        let extension = self.current_file.as_ref()
            .and_then(|p| p.extension())
            .and_then(|e| e.to_str())
            .unwrap_or("unknown");

        // 🔱 Layer 2: Strict Language Mapping
        let lang_name = match extension {
            "rs" => "Rust",
            "js" => "JavaScript",
            "ts" => "TypeScript",
            "py" => "Python",
            "c" => "C",
            "cpp" => "C++",
            "md" => "Markdown",
            "html" => "HTML",
            "css" => "CSS",
            _ => "Text"
        };

        let system_prompt = format!(
            "You are a Next Edit Prediction (NEP) engine inside the Z3N Editor. \n\
             Your goal is to predict the code that follows the current cursor position in a {} file.\n\n\
             CRITICAL RULES:\n\
             1. Output ONLY the code to be appended. NO triple backticks. NO explanations.\n\
             2. DO NOT repeat the code already provided in the prefix. If the prefix ends with 'fn main()', your code should start with ' {{\\n    ...'.\n\
             3. Maintain strict indentation matching the prefix.\n\
             4. START EXACTLY at the cursor. If the cursor is at the end of a line, start with a newline if appropriate.",
            lang_name
        );
        
        let user_prompt = format!("### PREFIX\n{}\n### CURSOR\n### SUFFIX\n{}", prefix, suffix);

        tokio::spawn(async move {
            let provider: Box<dyn crate::ai::provider::ModelProvider> = match provider_type {
                crate::ai::provider::ProviderType::Anthropic => Box::new(crate::ai::provider::AnthropicProvider),
                crate::ai::provider::ProviderType::Ollama => Box::new(crate::ai::provider::OllamaProvider),
                crate::ai::provider::ProviderType::Grok => Box::new(crate::ai::provider::GrokProvider),
                crate::ai::provider::ProviderType::Groq => Box::new(crate::ai::provider::GroqProvider),
            };
            
            let (inner_tx, mut inner_rx) = tokio::sync::mpsc::unbounded_channel();
            provider.stream_completion(system_prompt, user_prompt, api_key, inner_tx);
            
            let mut full_text = String::new();
            while let Some(token) = inner_rx.recv().await {
                full_text.push_str(&token);
                // 🔱 Layer 2 Optimization: Cap NEP hints at 500 chars for multi-line snippets
                if full_text.len() > 500 { break; }
            }

            if !full_text.trim().is_empty() {
                let line_count = full_text.lines().count().max(1);
                let _ = tx.send(NepHint {
                    text: full_text,
                    anchor: cursor,
                    version,
                    line_count,
                });
            }
        });
    }

    fn start_real_ai_stream(&mut self, prefix: &str, suffix: &str) {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        self.ai_receiver = Some(rx);
        self.editor.start_ai_stream();
        self.status_message = format!("🤖 AI ({:?}) is thinking...", self.ai_provider_type);

        let extension = self.current_file.as_ref()
            .and_then(|p| p.extension())
            .and_then(|e| e.to_str())
            .unwrap_or("text");

        let system_prompt = if let Some(tree) = self.renderer.highlighter.tree() {
            let cursor = self.editor.cursor();
            let offset = self.editor.buffer().point_to_offset(cursor).value();
            let node_path = self.editor.get_node_path_at(tree, offset);
            
            format!("You are the core intelligence of Z3N, a state-of-the-art AI-native code editor.
CRITICAL: You are currently working in a [{}] file. 
You must output ONLY raw [{}] code. NEVER output JavaScript, Python, or Markdown unless the file type is explicitly one of those.
Current semantic path (AST): {}

TASK: Complete the code at the cursor.
- Output ONLY raw code.
- NO markdown backticks.
- NO explanations.
- Match existing indentation and style exactly.
- If the code is already complete, output exactly nothing.", extension, extension, node_path)
        } else {
            format!("You are the core intelligence of Z3N, a state-of-the-art AI-native code editor.
CRITICAL: You are currently working in a [{}] file.
You must output ONLY raw [{}] code.
TASK: Complete the code at the cursor.
- Output ONLY raw code.
- NO markdown backticks.
- Match existing indentation and style exactly.", extension, extension)
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


}
