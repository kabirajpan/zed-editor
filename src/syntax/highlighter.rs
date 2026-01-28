use crate::syntax::languages::{LanguageConfig, LanguageRegistry};
use crate::syntax::theme::SyntaxTheme;
use egui::Color32;
use std::path::Path;
use tree_sitter::{Query, QueryCursor};

#[derive(Debug, Clone)]
pub struct HighlightSpan {
    pub start: usize,
    pub end: usize,
    pub color: Color32,
}

pub struct SyntaxHighlighter {
    registry: LanguageRegistry,
    theme: SyntaxTheme,
    debug_logged: bool,
}

impl SyntaxHighlighter {
    pub fn new(theme: SyntaxTheme) -> Self {
        Self {
            registry: LanguageRegistry::new(),
            theme,
            debug_logged: false,
        }
    }

    pub fn highlight_line(
        &mut self,
        text: &str,
        line_number: usize,
        file_path: Option<&Path>,
    ) -> Vec<HighlightSpan> {
        let Some(path) = file_path else {
            if !self.debug_logged {
                eprintln!("[HIGHLIGHT] No file path provided");
                self.debug_logged = true;
            }
            return vec![];
        };

        let Some(lang_config) = self.registry.detect_language(path) else {
            if !self.debug_logged {
                eprintln!("[HIGHLIGHT] Could not detect language for {:?}", path);
                self.debug_logged = true;
            }
            return vec![];
        };

        if !self.debug_logged {
            eprintln!("[HIGHLIGHT] Detected language: {}", lang_config.name);
            self.debug_logged = true;
        }

        self.highlight_line_with_language(text, line_number, lang_config)
    }

    fn highlight_line_with_language(
        &self,
        full_text: &str,
        line_number: usize,
        config: &LanguageConfig,
    ) -> Vec<HighlightSpan> {
        let mut parser = self.registry.create_parser(config);
        let Some(tree) = parser.parse(full_text, None) else {
            return vec![];
        };

        let Ok(query) = Query::new(&config.language, config.highlight_query) else {
            return vec![];
        };

        let mut cursor = QueryCursor::new();
        let root_node = tree.root_node();

        let lines: Vec<&str> = full_text.lines().collect();
        if line_number >= lines.len() {
            return vec![];
        }

        let line_start_byte: usize = lines
            .iter()
            .take(line_number)
            .map(|line| line.len() + 1)
            .sum();
        let line_end_byte = line_start_byte + lines[line_number].len();

        let mut highlights = Vec::new();

        for match_ in cursor.matches(&query, root_node, full_text.as_bytes()) {
            for capture in match_.captures {
                let node = capture.node;
                let start = node.start_byte();
                let end = node.end_byte();

                if end > line_start_byte && start < line_end_byte {
                    let capture_name = &query.capture_names()[capture.index as usize];
                    let color = self.theme.get_color(capture_name);

                    let span_start = start.saturating_sub(line_start_byte);
                    let span_end = (end - line_start_byte).min(lines[line_number].len());

                    if span_end > span_start {
                        highlights.push(HighlightSpan {
                            start: span_start,
                            end: span_end,
                            color,
                        });
                    }
                }
            }
        }

        highlights.sort_by_key(|h| (h.start, std::cmp::Reverse(h.end)));

        let mut merged = Vec::new();
        for highlight in highlights {
            if merged.is_empty() {
                merged.push(highlight);
            } else {
                let last = merged.last_mut().unwrap();
                if highlight.start < last.end {
                    if highlight.end > last.end {
                        last.end = highlight.end;
                    }
                } else {
                    merged.push(highlight);
                }
            }
        }

        merged
    }

    pub fn set_theme(&mut self, theme: SyntaxTheme) {
        self.theme = theme;
    }
}
