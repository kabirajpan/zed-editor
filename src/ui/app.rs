use crate::Editor;
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use std::io;

/// Application state
pub struct App {
    pub editor: Editor,
    pub should_quit: bool,
    pub status_message: String,
}

impl App {
    /// Create new app with empty editor
    pub fn new() -> Self {
        Self {
            editor: Editor::new(),
            should_quit: false,
            status_message: "Press Ctrl+Q to quit | Ctrl+S to save".to_string(),
        }
    }

    /// Create app with text
    pub fn with_text(text: &str) -> Self {
        Self {
            editor: Editor::from_text(text),
            should_quit: false,
            status_message: "Press Ctrl+Q to quit | Ctrl+S to save".to_string(),
        }
    }

    /// Handle keyboard input
    pub fn handle_input(&mut self) -> io::Result<()> {
        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                // Handle Ctrl+Q - Quit
                if key.code == KeyCode::Char('q') && key.modifiers.contains(KeyModifiers::CONTROL) {
                    self.should_quit = true;
                    return Ok(());
                }

                // Handle Ctrl+Z - Undo
                if key.code == KeyCode::Char('z') && key.modifiers.contains(KeyModifiers::CONTROL) {
                    if self.editor.can_undo() {
                        self.editor.undo();
                        self.status_message = "Undo".to_string();
                    } else {
                        self.status_message = "Nothing to undo".to_string();
                    }
                    return Ok(());
                }

                // Handle Ctrl+Y - Redo
                if key.code == KeyCode::Char('y') && key.modifiers.contains(KeyModifiers::CONTROL) {
                    if self.editor.can_redo() {
                        self.editor.redo();
                        self.status_message = "Redo".to_string();
                    } else {
                        self.status_message = "Nothing to redo".to_string();
                    }
                    return Ok(());
                }

                // Regular key handling
                match key.code {
                    KeyCode::Char(c) => {
                        if !key.modifiers.contains(KeyModifiers::CONTROL) {
                            self.editor.insert(&c.to_string());
                            self.status_message.clear();
                        }
                    }
                    KeyCode::Enter => {
                        self.editor.insert("\n");
                        self.status_message.clear();
                    }
                    KeyCode::Backspace => {
                        self.editor.backspace();
                        self.status_message.clear();
                    }
                    KeyCode::Delete => {
                        self.editor.delete();
                        self.status_message.clear();
                    }
                    KeyCode::Left => self.editor.move_left(),
                    KeyCode::Right => self.editor.move_right(),
                    KeyCode::Up => self.editor.move_up(),
                    KeyCode::Down => self.editor.move_down(),
                    KeyCode::Home => self.editor.move_to_line_start(),
                    KeyCode::End => self.editor.move_to_line_end(),
                    _ => {}
                }
            }
        }
        Ok(())
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}
