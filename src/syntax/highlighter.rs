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
    logged_once: bool,
}

impl SyntaxHighlighter {
    pub fn new(theme: SyntaxTheme) -> Self {
        Self {
            registry: LanguageRegistry::new(),
            theme,
            logged_once: false,
        }
    }

    pub fn highlight_line(
        &mut self,
        text: &str,
        line_number: usize,
        file_path: Option<&Path>,
    ) -> Vec<HighlightSpan> {
        // Only log ONCE per actual file (not when no file is open)
        let should_log = !self.logged_once && line_number == 0 && file_path.is_some();

        if should_log {
            eprintln!("\n=== HIGHLIGHT DEBUG ===");
            eprintln!("File path: {:?}", file_path);
            eprintln!("Text length: {}", text.len());
        }

        let Some(path) = file_path else {
            return vec![];
        };

        let Some(lang_config) = self.registry.detect_language(path) else {
            if should_log {
                eprintln!("ERROR: Language not detected for {:?}", path);
                self.logged_once = true;
            }
            return vec![];
        };

        if should_log {
            eprintln!("Language detected: {}", lang_config.name);
        }

        let highlights =
            self.highlight_line_with_language(text, line_number, lang_config, should_log);

        if should_log {
            eprintln!("Highlights generated: {}", highlights.len());
            eprintln!("======================\n");
            self.logged_once = true;
        }

        highlights
    }

    fn highlight_line_with_language(
        &self,
        full_text: &str,
        line_number: usize,
        config: &LanguageConfig,
        should_log: bool,
    ) -> Vec<HighlightSpan> {
        let mut parser = self.registry.create_parser(config);

        if should_log {
            eprintln!("[PARSE] Attempting to parse {} bytes", full_text.len());
        }

        let Some(tree) = parser.parse(full_text, None) else {
            if should_log {
                eprintln!("[PARSE] ERROR: Parse failed!");
            }
            return vec![];
        };

        if should_log {
            eprintln!("[PARSE] Success! Root node: {:?}", tree.root_node());
            eprintln!("[QUERY] Creating query...");
        }

        let query = match Query::new(&config.language, config.highlight_query) {
            Ok(q) => {
                if should_log {
                    eprintln!("[QUERY] Success! {} patterns", q.pattern_count());
                }
                q
            }
            Err(e) => {
                if should_log {
                    eprintln!("[QUERY] ERROR: {}", e);
                }
                return vec![];
            }
        };

        let mut cursor = QueryCursor::new();
        let root_node = tree.root_node();

        // Calculate byte offsets correctly from the original text
        let mut line_start_byte = 0;
        let mut current_line = 0;

        for (byte_idx, ch) in full_text.char_indices() {
            if current_line == line_number {
                line_start_byte = byte_idx;
                break;
            }
            if ch == '\n' {
                current_line += 1;
            }
        }

        // Find line end
        let line_end_byte = full_text[line_start_byte..]
            .char_indices()
            .find(|(_, ch)| *ch == '\n')
            .map(|(idx, _)| line_start_byte + idx)
            .unwrap_or(full_text.len());

        let line_text = &full_text[line_start_byte..line_end_byte];
        let line_char_len = line_text.chars().count();

        let mut highlights = Vec::new();

        for match_ in cursor.matches(&query, root_node, full_text.as_bytes()) {
            for capture in match_.captures {
                let node = capture.node;
                let start = node.start_byte();
                let end = node.end_byte();

                // Check if this capture overlaps with our line
                if end > line_start_byte && start < line_end_byte {
                    let capture_name = &query.capture_names()[capture.index as usize];
                    let color = self.theme.get_color(capture_name);

                    // Convert byte offsets to character offsets within the line
                    let span_start = if start <= line_start_byte {
                        0
                    } else {
                        full_text[line_start_byte..start].chars().count()
                    };

                    let span_end = if end >= line_end_byte {
                        line_char_len
                    } else {
                        full_text[line_start_byte..end].chars().count()
                    };

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

        // Merge overlapping highlights
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
