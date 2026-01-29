use crate::syntax::languages::{LanguageConfig, LanguageRegistry};
use crate::syntax::theme::SyntaxTheme;
use egui::Color32;
use std::path::Path;
use tree_sitter::{Query, QueryCursor, Tree};

#[derive(Debug, Clone)]
pub struct HighlightSpan {
    pub start: usize,
    pub end: usize,
    pub color: Color32,
}

struct ParseCache {
    text_hash: u64,
    tree: Tree,
    language_name: String,
}

pub struct SyntaxHighlighter {
    registry: LanguageRegistry,
    theme: SyntaxTheme,
    logged_once: bool,
    parse_cache: Option<ParseCache>,
}

impl SyntaxHighlighter {
    pub fn new(theme: SyntaxTheme) -> Self {
        Self {
            registry: LanguageRegistry::new(),
            theme,
            logged_once: false,
            parse_cache: None,
        }
    }

    // Simple hash function for text
    fn hash_text(text: &str) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        text.hash(&mut hasher);
        hasher.finish()
    }

    /// 🚀 PRODUCTION-FIXED: Highlight line using Rope directly (no full file conversion!)
    /// This is the main entry point called by viewport_renderer
    pub fn highlight_line(
        &mut self,
        rope: &crate::rope::Rope,
        line_number: usize,
        file_path: Option<&Path>,
    ) -> Vec<HighlightSpan> {
        // Only log ONCE per actual file (not when no file is open)
        let should_log = !self.logged_once && line_number == 0 && file_path.is_some();

        if should_log {
            eprintln!("\n=== HIGHLIGHT DEBUG ===");
            eprintln!("File path: {:?}", file_path);
            eprintln!("Rope length: {} bytes", rope.len());
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

        // Clone the config to avoid borrow issues
        let lang_config = lang_config.clone();

        if should_log {
            eprintln!("Language detected: {}", lang_config.name);
        }

        let highlights =
            self.highlight_line_with_language(rope, line_number, &lang_config, should_log);

        if should_log {
            eprintln!("Highlights generated: {}", highlights.len());
            eprintln!("======================\n");
            self.logged_once = true;
        }

        highlights
    }

    /// 🚀 PRODUCTION-FIXED: Core highlighting logic using context window
    /// Instead of parsing the entire file, we parse a small context window around the target line
    fn highlight_line_with_language(
        &mut self,
        rope: &crate::rope::Rope,
        line_number: usize,
        config: &LanguageConfig,
        should_log: bool,
    ) -> Vec<HighlightSpan> {
        // 🚀 CRITICAL FIX: Get line byte range using Rope's efficient O(log n) lookup
        // OLD CODE: Iterated through entire file character by character (O(n))
        // NEW CODE: Uses rope's cached newline positions (O(log n))
        let Some((line_start_byte, line_end_byte)) = rope.line_byte_range(line_number) else {
            return vec![];
        };

        // For syntax highlighting, we need context around the line
        // Get a reasonable context window (e.g., 50 lines before and after)
        const CONTEXT_LINES: usize = 50;

        let context_start_line = line_number.saturating_sub(CONTEXT_LINES);
        let context_end_line = (line_number + CONTEXT_LINES + 1).min(rope.line_count());

        let context_start_byte = rope.line_to_byte(context_start_line);
        let context_end_byte = if context_end_line < rope.line_count() {
            rope.line_to_byte(context_end_line)
        } else {
            rope.len()
        };

        // 🚀 CRITICAL FIX: Extract ONLY the context window, not the entire file!
        // OLD CODE: let full_text = editor.text(); // Converted entire rope to string!
        // NEW CODE: Only extract ~100 lines of context
        let context_text = rope.slice_bytes(context_start_byte, context_end_byte);

        if should_log {
            eprintln!(
                "[CONTEXT] Line {}: extracting {} bytes (lines {}-{})",
                line_number,
                context_text.len(),
                context_start_line,
                context_end_line
            );
        }

        // Parse only the context window (not the entire file!)
        let mut parser = self.registry.create_parser(config);
        let tree = match parser.parse(&context_text, None) {
            Some(t) => t,
            None => {
                if should_log {
                    eprintln!("[PARSE] ERROR: Parse failed!");
                }
                return vec![];
            }
        };

        if should_log {
            eprintln!("[PARSE] Context parsed successfully");
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

        // Calculate offsets relative to context window
        let target_line_start_in_context = line_start_byte - context_start_byte;
        let target_line_end_in_context = line_end_byte - context_start_byte;

        let line_text = rope.slice_bytes(line_start_byte, line_end_byte);
        let line_char_len = line_text.chars().count();

        let mut highlights = Vec::new();

        // Query for highlights in the context window
        for match_ in cursor.matches(&query, root_node, context_text.as_bytes()) {
            for capture in match_.captures {
                let node = capture.node;
                let start = node.start_byte();
                let end = node.end_byte();

                // Check if this capture overlaps with our target line
                if end > target_line_start_in_context && start < target_line_end_in_context {
                    let capture_name = &query.capture_names()[capture.index as usize];
                    let color = self.theme.get_color(capture_name);

                    // Convert byte offsets to character offsets within the line
                    let span_start = if start <= target_line_start_in_context {
                        0
                    } else {
                        context_text[target_line_start_in_context..start]
                            .chars()
                            .count()
                    };

                    let span_end = if end >= target_line_end_in_context {
                        line_char_len
                    } else {
                        context_text[target_line_start_in_context..end]
                            .chars()
                            .count()
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

        // Sort by start position, then by longest span first (for proper nesting)
        highlights.sort_by_key(|h| (h.start, std::cmp::Reverse(h.end)));

        // Merge overlapping highlights
        let mut merged = Vec::new();
        for highlight in highlights {
            if merged.is_empty() {
                merged.push(highlight);
            } else {
                let last = merged.last_mut().unwrap();
                if highlight.start < last.end {
                    // Overlapping - extend if needed
                    if highlight.end > last.end {
                        last.end = highlight.end;
                    }
                } else {
                    // No overlap - add new span
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
