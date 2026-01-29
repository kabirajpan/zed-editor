use crate::formatter::providers::{PrettierProvider, RustfmtProvider};
use crate::io::write_file_from_rope; // 🚀 Import new efficient rope writer
use crate::{read_file, Editor, Formatter, SyntaxHighlighter, SyntaxTheme};
use std::path::PathBuf;
use std::time::Instant;

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
    status_message: String,
    auto_scroll: bool,
    current_file: Option<PathBuf>,
    loading_state: LoadingState,
    renderer: ViewportRenderer,
    formatter: Formatter,
    highlighter: SyntaxHighlighter,
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
            status_message: String::new(),
            auto_scroll: true,
            current_file: None,
            loading_state: LoadingState::Idle,
            renderer: ViewportRenderer::new(),
            formatter,
            highlighter,
        }
    }

    fn handle_text_input(&mut self, text: &str) {
        let cursor_line = self.editor.cursor().row;

        // Auto-close brackets
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
        self.renderer.invalidate_from_line(cursor_line);
    }

    fn handle_key(&mut self, key: egui::Key, modifiers: egui::Modifiers) {
        let cursor_before = self.editor.cursor();

        match key {
            egui::Key::ArrowLeft => {
                self.editor.move_left();
            }
            egui::Key::ArrowRight => {
                self.editor.move_right();
            }
            egui::Key::ArrowUp => {
                self.editor.move_up();
            }
            egui::Key::ArrowDown => {
                self.editor.move_down();
            }
            egui::Key::Home => {
                self.editor.move_to_line_start();
            }
            egui::Key::End => {
                self.editor.move_to_line_end();
            }
            egui::Key::Backspace => {
                let cursor_line = self.editor.cursor().row;
                self.editor.backspace();
                self.status_message.clear();
                self.renderer
                    .invalidate_from_line(cursor_line.saturating_sub(1));
            }
            egui::Key::Delete => {
                let cursor_line = self.editor.cursor().row;
                self.editor.delete();
                self.status_message.clear();
                self.renderer.invalidate_line(cursor_line);
            }
            egui::Key::Enter => {
                let cursor_line = self.editor.cursor().row;
                self.editor.insert("\n");
                self.status_message.clear();
                self.renderer.invalidate_from_line(cursor_line);
            }
            egui::Key::Z if modifiers.ctrl => {
                if self.editor.can_undo() {
                    self.editor.undo();
                    self.status_message = "Undo".to_string();
                    self.renderer.invalidate_from_line(0);
                }
            }
            egui::Key::Y if modifiers.ctrl => {
                if self.editor.can_redo() {
                    self.editor.redo();
                    self.status_message = "Redo".to_string();
                    self.renderer.invalidate_from_line(0);
                }
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

    fn format_code(&mut self) {
        if let Some(ref file_path) = self.current_file {
            match self.editor.format(&self.formatter, Some(file_path)) {
                Ok(_) => {
                    self.status_message = "✨ Code formatted successfully".to_string();
                    self.renderer.invalidate_from_line(0);
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
                self.renderer.invalidate_from_line(0);

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

    /// 🚀 PERFORMANCE-FIXED: Save file using Rope directly (no string conversion!)
    fn save_file(&mut self) {
        if let Some(ref path) = self.current_file.clone() {
            // Format if formatter is available
            if self.formatter.find_provider(&path).is_some() {
                match self.editor.format(&self.formatter, Some(&path)) {
                    Ok(_) => {}
                    Err(e) => {
                        self.status_message = format!("⚠️ Format failed: {}, saving anyway", e);
                    }
                }
            }

            // 🚀 CRITICAL FIX: Write directly from Rope without converting to String!
            // OLD CODE:
            // match crate::write_file(&path, &self.editor.text()) {  // ❌ Converts entire file!

            // NEW CODE: Use efficient rope writer
            match write_file_from_rope(&path, self.editor.buffer().rope()) {
                Ok(_) => {
                    let filename = path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("Unknown");
                    self.status_message = format!("💾 Saved: {}", filename);
                    self.renderer.invalidate_from_line(0);
                }
                Err(e) => {
                    self.status_message = format!("❌ Error: {}", e);
                }
            }
        } else {
            self.save_file_as();
        }
    }

    /// 🚀 PERFORMANCE-FIXED: Save as using Rope directly
    fn save_file_as(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Text Files", &["txt"])
            .add_filter("Rust Files", &["rs"])
            .add_filter("JavaScript Files", &["js"])
            .add_filter("Python Files", &["py"])
            .add_filter("All Files", &["*"])
            .save_file()
        {
            // 🚀 CRITICAL FIX: Write directly from Rope
            // OLD CODE: match crate::write_file(&path, &self.editor.text()) {
            // NEW CODE:
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
        self.renderer.invalidate_from_line(0);
        self.status_message = "📄 New file".to_string();
    }
}

impl eframe::App for GuiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.last_blink.elapsed().as_millis() > 500 {
            self.cursor_blink = !self.cursor_blink;
            self.last_blink = Instant::now();
        }
        ctx.request_repaint();

        ctx.input(|i| {
            for event in &i.events {
                match event {
                    egui::Event::Text(text) => {
                        self.handle_text_input(text);
                    }
                    egui::Event::Key {
                        key,
                        pressed: true,
                        modifiers,
                        ..
                    } => {
                        self.handle_key(*key, *modifiers);
                    }
                    _ => {}
                }
            }
        });

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
                        self.renderer.invalidate_from_line(0);
                        ui.close_menu();
                    }
                    if ui
                        .add_enabled(self.editor.can_redo(), egui::Button::new("↷ Redo (Ctrl+Y)"))
                        .clicked()
                    {
                        self.editor.redo();
                        self.renderer.invalidate_from_line(0);
                        ui.close_menu();
                    }

                    ui.separator();

                    let can_format = self.current_file.is_some();
                    if ui
                        .add_enabled(
                            can_format,
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

        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            let cursor = self.editor.cursor();
            let status = if !self.status_message.is_empty() {
                self.status_message.clone()
            } else {
                format!(
                    "Line {}, Col {} | {} lines",
                    cursor.row + 1,
                    cursor.column + 1,
                    self.editor.line_count()
                )
            };
            ui.label(status);
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            self.renderer.render_with_highlighting(
                ui,
                &self.editor,
                &mut self.highlighter,
                self.cursor_blink,
                self.auto_scroll,
            );
            self.auto_scroll = false;
        });
    }
}
