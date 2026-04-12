use crate::editor::Editor;
use crate::ai::chat::{ChatMessage, MessageRole};
use crate::ai::provider::{ModelProvider, ProviderType, AnthropicProvider, OllamaProvider, GrokProvider, GroqProvider};
use tokio::sync::mpsc;
use crate::util::project::get_project_structure;
use std::path::Path;
use crate::manager::{FocusTarget, GlobalManager};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RightPanelTab {
    Chat,
    Investigator,
}

pub struct RightPanel {
    pub active_tab: RightPanelTab,
    pub chat_history: Vec<ChatMessage>,
    pub chat_input: String,
    pub chat_receiver: Option<mpsc::UnboundedReceiver<String>>,
    pub is_visible: bool,
}

impl RightPanel {
    pub fn new() -> Self {
        Self {
            active_tab: RightPanelTab::Chat,
            chat_history: Vec::new(),
            chat_input: String::new(),
            chat_receiver: None,
            is_visible: false,
        }
    }

    pub fn render(
        &mut self,
        ctx: &egui::Context,
        editor: &mut Editor,
        provider_type: &mut ProviderType,
        api_key: &mut String,
        viewport_tree: Option<&tree_sitter::Tree>,
        manager: &mut GlobalManager,
        permissions: &mut crate::gui::app::AiPermissionPool,
    ) {
        if !self.is_visible {
            return;
        }

        // ── Handle incoming tokens ───────────────────────────────────────────
        if let Some(ref mut rx) = self.chat_receiver {
            while let Ok(token) = rx.try_recv() {
                if let Some(last_msg) = self.chat_history.last_mut() {
                    if last_msg.role == MessageRole::Assistant {
                        last_msg.content.push_str(&token);
                    } else {
                        self.chat_history.push(ChatMessage::assistant(token));
                    }
                } else {
                    self.chat_history.push(ChatMessage::assistant(token));
                }
            }
        }

        egui::SidePanel::right("panel_right")
            .resizable(true)
            .default_width(320.0)
            .max_width(600.0)
            .show(ctx, |ui| {
                ui.add_space(4.0);

                // ── 100% Width Tab Bar ──────────────────────────────────────────
                let tabs = [RightPanelTab::Chat, RightPanelTab::Investigator];
                let num_tabs = tabs.len();
                
                ui.columns(num_tabs, |cols| {
                    for (i, tab) in tabs.iter().enumerate() {
                        let label = match tab {
                            RightPanelTab::Chat => "🤖 Chat",
                            RightPanelTab::Investigator => "🔍 Investigator",
                        };
                        
                        let is_selected = self.active_tab == *tab;
                        let resp = cols[i].add_sized(
                            [cols[i].available_width(), 32.0],
                            egui::SelectableLabel::new(is_selected, label)
                        );
                        if resp.clicked() {
                            self.active_tab = *tab;
                        }
                    }
                });

                ui.separator();

                match self.active_tab {
                    RightPanelTab::Chat => self.render_chat_tab(ui, editor, provider_type, api_key, manager, permissions, viewport_tree),
                    RightPanelTab::Investigator => self.render_investigator_tab(ui, editor, viewport_tree, provider_type, api_key),
                }
            });
    }

    fn render_chat_tab(
        &mut self, 
        ui: &mut egui::Ui, 
        editor: &mut Editor, 
        provider_type: &mut ProviderType, 
        api_key: &mut String, 
        manager: &mut GlobalManager,
        permissions: &mut crate::gui::app::AiPermissionPool,
        viewport_tree: Option<&tree_sitter::Tree>,
    ) {
        ui.vertical(|ui| {
            let available_height = ui.available_height();
            let input_area_height = 100.0;
            
            egui::ScrollArea::vertical()
                .max_height(available_height - input_area_height)
                .auto_shrink([false; 2])
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    if self.chat_history.is_empty() {
                        ui.vertical_centered(|ui| {
                            ui.add_space(40.0);
                            ui.label(egui::RichText::new("🤖 Z3N Agent").strong().size(20.0));
                            ui.label(egui::RichText::new("Strategic focus is the key to velocity.").weak());
                        });
                    }

                    let mut pending_chat_responses = Vec::new();

                    for msg in &self.chat_history {
                        let is_user = msg.role == MessageRole::User;
                        let bg = if is_user { 
                            egui::Color32::from_rgb(50, 60, 110) 
                        } else { 
                            egui::Color32::from_rgb(35, 35, 45) 
                        };

                        ui.horizontal(|ui| {
                            if is_user { ui.add_space(20.0); }
                            egui::Frame::none()
                                .fill(bg)
                                .rounding(8.0)
                                .inner_margin(8.0)
                                .show(ui, |ui| {
                                    ui.set_max_width(ui.available_width() - 20.0);
                                    ui.vertical(|ui| {
                                        ui.label(egui::RichText::new(&msg.content).color(egui::Color32::WHITE));

                                        // 🪄 Agentic Action (Layer 2): Smart Code Extraction & Permission
                                        if !is_user && msg.content.contains("```") {
                                            let current_ext = editor.file_path()
                                                .and_then(|p| p.extension())
                                                .and_then(|e| e.to_str())
                                                .unwrap_or(if viewport_tree.is_some() { "rs" } else { "" }); // Simple fallback for now

                                            if let Some(code) = extract_smart_code_block(&msg.content, current_ext) {
                                                ui.add_space(8.0);
                                                ui.horizontal(|ui| {
                                                    let filename = editor.file_path()
                                                        .and_then(|p| p.file_name())
                                                        .and_then(|n| n.to_str())
                                                        .unwrap_or("active buffer");

                                                    if ui.button(egui::RichText::new(format!("✨ Apply to {}", filename)).strong()).clicked() {
                                                        if permissions.write_granted {
                                                            // 🔱 Layer 2: Permission Handshake Complete
                                                            editor.start_speculative_session();
                                                            editor.insert_ai_stream(&code);
                                                            manager.focus.set(FocusTarget::Editor);
                                                        } else {
                                                            pending_chat_responses.push(ChatMessage::assistant("🔒 Z3N requires your permission to WRITE to the file. Please grant 'Write Access' in the prompt below.".to_string()));
                                                        }
                                                    }
                                                });
                                            }
                                        }
                                    });
                                });
                             if !is_user { ui.add_space(20.0); }
                        });
                        ui.add_space(8.0);
                    }
                    
                    for resp in pending_chat_responses {
                        self.chat_history.push(resp);
                    }
                    
                    // 🛡️ Layer 2 Security Gating: Explicit Permission Controls
                    if !permissions.read_granted || !permissions.write_granted {
                        ui.add_space(16.0);
                        egui::Frame::none()
                            .fill(egui::Color32::from_rgb(45, 45, 55))
                            .rounding(4.0)
                            .inner_margin(8.0)
                            .show(ui, |ui| {
                                ui.vertical(|ui| {
                                    ui.label(egui::RichText::new("📊 AI Permissions Required").strong().color(egui::Color32::WHITE));
                                    ui.add_space(4.0);
                                    ui.checkbox(&mut permissions.read_granted, "Grant Read Access (Context Sharing)");
                                    ui.checkbox(&mut permissions.write_granted, "Grant Write Access (Speculative Coding)");
                                });
                            });
                    }
                });

            ui.separator();
            ui.add_space(4.0);

            let input_response = ui.add(
                egui::TextEdit::multiline(&mut self.chat_input)
                    .hint_text("Write a plan (Enter to send)...")
                    .id(egui::Id::new("chat_input"))
                    .desired_rows(2)
                    .desired_width(ui.available_width() - 8.0)
                    .margin(egui::vec2(8.0, 8.0))
            );

            // 🛡️ Focus Handshake: Notify the custom Focus Manager
            if input_response.has_focus() {
                manager.focus.set(FocusTarget::RightPanel);
            }

            ui.horizontal(|ui| {
                ui.small("Shift+Enter for newline");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let enter_pressed = ui.input(|i| i.key_pressed(egui::Key::Enter) && !i.modifiers.shift);
                    if ui.button("🚀 Send").clicked() || (input_response.has_focus() && enter_pressed) {
                        if !self.chat_input.trim().is_empty() {
                            if permissions.read_granted {
                                let content = editor.buffer().to_string();
                                let file_path = editor.file_path()
                                    .map(|p| p.to_string_lossy().to_string())
                                    .unwrap_or_else(|| "untitled".to_string());
                                
                                // 🔱 Layer 1: Selection & Semantic Cursor
                                let selection = editor.selected_text();
                                let cursor_offset = editor.buffer().point_to_offset(editor.cursor()).value();
                                let node_path = if let Some(tree) = viewport_tree {
                                    editor.get_node_path_at(tree, cursor_offset)
                                } else {
                                    "root".to_string()
                                };

                                self.trigger_chat(*provider_type, api_key.clone(), content, file_path, selection, node_path);
                            } else {
                                self.chat_history.push(ChatMessage::assistant("🔒 Access Denied: Z3N cannot read your file without permission. Please check the 'Grant Read Access' box below.".to_string()));
                            }
                        }
                    }
                });
            });
        });
    }

    fn trigger_chat(
        &mut self, 
        provider_type: ProviderType, 
        api_key: String, 
        code_context: String, 
        file_path: String,
        selection: Option<String>,
        node_path: String,
    ) {
        let text = std::mem::take(&mut self.chat_input);
        self.chat_history.push(ChatMessage::user(text));
        
        // 📁 Full Repository Context (File Tree)
        let repo_structure = get_project_structure(Path::new("."));
        
        let context = format!(
            "You are a helpful AI assistant inside the Z3N code editor. \n\
             ## PROJECT_MAP\n{}\n\n\
             ## ACTIVE_FILE_PATH\n{}\n\n\
             ## SEMANTIC_CURSOR_PATH\n{}\n\n\
             ## ACTIVE_FILE_CONTENT\n```\n{}\n```\n\n\
             {}\
             Instructions:\n\
             1. Answer building on this context. You are currently editing {} in a {} context.\n\
             2. Be concise and precise.\n\
             3. If you suggest code, ALWAYS put it inside triple backticks.\n\
             4. You can 'Write full code', 'Update code', or 'Delete code lines'.",
            repo_structure,
            file_path,
            node_path,
            code_context,
            if let Some(sel) = selection { format!("## SELECTED_CODE\n```\n{}\n```\n\n", sel) } else { "".to_string() },
            file_path,
            Path::new(&file_path).extension().and_then(|e| e.to_str()).unwrap_or("unknown")
        );
        
        if !self.chat_history.is_empty() && self.chat_history[0].role == MessageRole::System {
            self.chat_history[0].content = context;
        } else {
            self.chat_history.insert(0, ChatMessage::system(context));
        }

        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        self.chat_receiver = Some(rx);

        let provider: Box<dyn ModelProvider> = match provider_type {
            ProviderType::Anthropic => Box::new(AnthropicProvider),
            ProviderType::Ollama => Box::new(OllamaProvider),
            ProviderType::Grok => Box::new(GrokProvider),
            ProviderType::Groq => Box::new(GroqProvider),
        };

        let history = self.chat_history.clone();
        provider.stream_chat(history, api_key, tx);
    }

    fn render_investigator_tab(
        &mut self, 
        ui: &mut egui::Ui, 
        editor: &mut Editor, 
        viewport_tree: Option<&tree_sitter::Tree>,
        provider_type: &mut ProviderType,
        api_key: &mut String,
    ) {
        ui.heading("🔍 Layer 1 Investigator");
        ui.add_space(8.0);

        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.collapsing("🧬 Provenance", |ui| {
                let cursor = editor.cursor();
                let offset = editor.buffer().point_to_offset(cursor).value();
                let interrogate_offset = offset.saturating_sub(1);
                let (author, timestamp) = editor.provenance_at(interrogate_offset);

                ui.horizontal(|ui| {
                    ui.label("Origin:");
                    let (text, color) = match author {
                        crate::history::transaction::Author::Human => ("HUMAN", egui::Color32::from_rgb(100, 255, 100)),
                        crate::history::transaction::Author::AiSuggested => ("AI_SUGGESTED", egui::Color32::from_rgb(255, 180, 0)),
                        crate::history::transaction::Author::AiModified => ("AI_MODIFIED", egui::Color32::from_rgb(255, 100, 100)),
                        crate::history::transaction::Author::AiPending => ("AI_PENDING", egui::Color32::from_rgb(100, 200, 255)),
                    };
                    ui.label(egui::RichText::new(text).color(color).strong());
                });

                if let Some( ts) = timestamp {
                    ui.label(egui::RichText::new(format!("Modified: {}s ago", ts.elapsed().as_secs())).small().weak());
                }
            });

            ui.add_space(8.0);

            ui.collapsing("📊 PIE Deltas", |ui| {
                let deltas = editor.last_semantic_deltas();
                ui.label(format!("Active Deltas: {}", deltas.len()));
                
                for delta in deltas.iter().take(5) {
                    ui.label(format!("[{:?}] {}", delta.edit_type, delta.node_path));
                    ui.separator();
                }
                
                if ui.button("Force Sync").clicked() {
                    if let Some(tree) = viewport_tree {
                        let text = editor.buffer().to_string();
                        editor.update_semantic_deltas(tree, &text);
                    }
                }
            });

            ui.add_space(8.0);

            ui.collapsing("🤖 AI Provider", |ui| {
                ui.horizontal(|ui| {
                    ui.label("Provider:");
                    egui::ComboBox::from_id_salt("right_ai_provider_panel")
                        .selected_text(format!("{:?}", provider_type))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(provider_type, ProviderType::Anthropic, "Anthropic");
                            ui.selectable_value(provider_type, ProviderType::Ollama, "Ollama");
                            ui.selectable_value(provider_type, ProviderType::Grok, "Grok");
                            ui.selectable_value(provider_type, ProviderType::Groq, "Groq");
                        });
                });
                ui.add(egui::TextEdit::singleline(api_key).password(true).hint_text("API Key"));
            });
        });
    }
}

/// Smart Aggregator for Layer 2: Scans for multiple blocks and merges them if they share the language intent.
fn extract_smart_code_block(content: &str, target_ext: &str) -> Option<String> {
    let mut matching_blocks = Vec::new();
    let mut current_block = String::new();
    let mut current_lang = String::new();
    let mut in_block = false;

    for line in content.lines() {
        if line.trim().starts_with("```") {
            if in_block {
                let cleaned_lang = current_lang.trim().to_lowercase();
                // 🔱 Layer 2 Normalization: Map aliases (e.g. rust -> rs)
                let is_match = cleaned_lang == target_ext 
                    || (target_ext == "rs" && cleaned_lang == "rust") 
                    || (target_ext == "js" && cleaned_lang == "javascript")
                    || (target_ext == "ts" && cleaned_lang == "typescript")
                    || (target_ext == "py" && cleaned_lang == "python");

                if is_match || target_ext.is_empty() {
                    matching_blocks.push(current_block.trim().to_string());
                }
                current_block.clear();
                current_lang.clear();
                in_block = false;
            } else {
                in_block = true;
                current_lang = line.trim().trim_start_matches("```").to_lowercase();
            }
            continue;
        }

        if in_block {
            current_block.push_str(line);
            current_block.push('\n');
        }
    }

    if matching_blocks.is_empty() { return None; }

    // 🔱 Layer 2: Aggregation Logic
    // If we found multiple matching blocks, join them. This handles "Full Code" requests
    // where the AI splits the code into logical segments.
    Some(matching_blocks.join("\n\n"))
}
