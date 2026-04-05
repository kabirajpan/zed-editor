use crate::syntax::languages::{LanguageConfig, LanguageRegistry};
use std::path::Path;
use tree_sitter::{Query, QueryCursor, Tree};

#[derive(Clone)]
pub struct IndentCalculator {
    registry: LanguageRegistry,
    indent_width: usize,
}

impl IndentCalculator {
    pub fn new() -> Self {
        Self {
            registry: LanguageRegistry::new(),
            indent_width: 4,
        }
    }

    /// Legacy string-based API (kept for compatibility)
    pub fn calculate_indent(
        &self,
        text: &str,
        cursor_line: usize,
        file_path: Option<&Path>,
    ) -> String {
        let Some(path) = file_path else {
            return self.fallback_indent(text, cursor_line);
        };
        let Some(lang_config) = self.registry.detect_language(path) else {
            return self.fallback_indent(text, cursor_line);
        };
        let mut parser = self.registry.create_parser(lang_config);
        let Some(tree) = parser.parse(text, None) else {
            return self.fallback_indent(text, cursor_line);
        };
        self.query_based_indent(text, cursor_line, &tree, lang_config)
    }

    /// Called on every newline — uses a context window for performance
    pub fn calculate_indent_with_rope(
        &self,
        rope: &crate::rope::Rope,
        cursor_line: usize,
        file_path: Option<&Path>,
    ) -> String {
        let Some(path) = file_path else {
            return self.fallback_indent_with_rope(rope, cursor_line);
        };
        let Some(lang_config) = self.registry.detect_language(path) else {
            return self.fallback_indent_with_rope(rope, cursor_line);
        };

        // Only parse a context window, not the entire file
        const CONTEXT_LINES: usize = 50;
        let context_start_line = cursor_line.saturating_sub(CONTEXT_LINES);
        let context_end_line = (cursor_line + CONTEXT_LINES + 1).min(rope.line_count());

        let context_start_byte = rope.line_to_byte(context_start_line);
        let context_end_byte = if context_end_line < rope.line_count() {
            rope.line_to_byte(context_end_line)
        } else {
            rope.len()
        };

        let context_text = rope.slice_bytes(context_start_byte, context_end_byte);
        let mut parser = self.registry.create_parser(lang_config);
        let Some(tree) = parser.parse(&context_text, None) else {
            return self.fallback_indent_with_rope(rope, cursor_line);
        };

        let line_in_context = cursor_line - context_start_line;
        self.query_based_indent(&context_text, line_in_context, &tree, lang_config)
    }

    /// Core logic: use the indents.scm query to determine indent level
    fn query_based_indent(
        &self,
        text: &str,
        cursor_line: usize,
        tree: &Tree,
        config: &LanguageConfig,
    ) -> String {
        let lines: Vec<&str> = text.lines().collect();

        if cursor_line >= lines.len() {
            return String::new();
        }

        let current_line = lines[cursor_line];
        let current_indent = Self::get_line_indent(current_line);

        // Build the query from the language's indents.scm
        let query = match Query::new(&config.language, config.indent_query) {
            Ok(q) => q,
            Err(_) => return self.fallback_indent(text, cursor_line),
        };

        let indent_capture_idx = match query.capture_index_for_name("indent") {
            Some(idx) => idx,
            None => return self.fallback_indent(text, cursor_line),
        };

        // Byte offset at the end of the current line (where cursor sits after typing)
        let cursor_byte: usize = lines
            .iter()
            .take(cursor_line + 1)
            .map(|l| l.len() + 1) // +1 for newline
            .sum::<usize>()
            .saturating_sub(1);

        // Walk all query matches and find the deepest @indent node
        // that contains the cursor position
        let mut best_start_byte: Option<usize> = None;
        let mut best_end_byte: Option<usize> = None;
        let mut best_start_row: Option<usize> = None;

        let mut query_cursor = QueryCursor::new();
        let matches = query_cursor.matches(&query, tree.root_node(), text.as_bytes());

        for m in matches {
            for capture in m.captures {
                if capture.index != indent_capture_idx {
                    continue;
                }

                let node = capture.node;
                let node_start = node.start_byte();
                let node_end = node.end_byte();

                // Only consider nodes that contain the cursor
                if node_start > cursor_byte || cursor_byte >= node_end {
                    continue;
                }

                // Keep the most specific (narrowest) containing node
                let is_better = match (best_start_byte, best_end_byte) {
                    (None, _) => true,
                    (Some(prev_start), Some(prev_end)) => {
                        // Narrower range = more specific
                        node_start >= prev_start && node_end <= prev_end
                    }
                    _ => true,
                };

                if is_better {
                    best_start_byte = Some(node_start);
                    best_end_byte = Some(node_end);
                    best_start_row = Some(node.start_position().row);
                }
            }
        }

        // If we found an @indent node containing the cursor,
        // new indent = that node's start line indent + one level
        if let Some(start_row) = best_start_row {
            if start_row < lines.len() {
                let node_line_indent = Self::get_line_indent(lines[start_row]);
                return format!("{}{}", node_line_indent, " ".repeat(self.indent_width));
            }
        }

        // No @indent node found — preserve current indentation
        current_indent
    }

    fn get_line_indent(line: &str) -> String {
        line.chars()
            .take_while(|c| c.is_whitespace() && *c != '\n')
            .collect()
    }

    fn fallback_indent_with_rope(&self, rope: &crate::rope::Rope, cursor_line: usize) -> String {
        if let Some(line_text) = rope.line(cursor_line) {
            let trimmed = line_text.trim();
            let indent = Self::get_line_indent(&line_text);
            let opens = trimmed.matches('{').count()
                + trimmed.matches('[').count()
                + trimmed.matches('(').count();
            let closes = trimmed.matches('}').count()
                + trimmed.matches(']').count()
                + trimmed.matches(')').count();
            if opens > closes {
                format!("{}{}", indent, " ".repeat(self.indent_width))
            } else {
                indent
            }
        } else {
            String::new()
        }
    }

    fn fallback_indent(&self, text: &str, cursor_line: usize) -> String {
        let lines: Vec<&str> = text.lines().collect();
        if cursor_line >= lines.len() {
            return String::new();
        }
        let current_line = lines[cursor_line];
        let indent = Self::get_line_indent(current_line);
        let trimmed = current_line.trim();
        let opens = trimmed.matches('{').count()
            + trimmed.matches('[').count()
            + trimmed.matches('(').count();
        let closes = trimmed.matches('}').count()
            + trimmed.matches(']').count()
            + trimmed.matches(')').count();
        if opens > closes || trimmed.ends_with(':') {
            format!("{}{}", indent, " ".repeat(self.indent_width))
        } else {
            indent
        }
    }
}

impl Default for IndentCalculator {
    fn default() -> Self {
        Self::new()
    }
}
