use crate::syntax::languages::{LanguageConfig, LanguageRegistry};
use std::path::Path;
use tree_sitter::{Node, Tree};

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

    /// 🚀 LEGACY METHOD: Keep for backward compatibility
    /// This still converts to string, but it's only used in non-hot paths
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

        self.tree_based_indent(text, cursor_line, &tree, lang_config)
    }

    /// 🚀 NEW OPTIMIZED METHOD: Uses Rope directly with context window!
    /// This is called on EVERY newline, so it must be fast
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

        // 🚀 PERFORMANCE FIX: Only parse a context window, not entire file!
        // Similar to syntax highlighter - we only need context around cursor
        const CONTEXT_LINES: usize = 50;

        let context_start_line = cursor_line.saturating_sub(CONTEXT_LINES);
        let context_end_line = (cursor_line + CONTEXT_LINES + 1).min(rope.line_count());

        let context_start_byte = rope.line_to_byte(context_start_line);
        let context_end_byte = if context_end_line < rope.line_count() {
            rope.line_to_byte(context_end_line)
        } else {
            rope.len()
        };

        // Extract ONLY the context window (not entire file!)
        let context_text = rope.slice_bytes(context_start_byte, context_end_byte);

        // Parse only the context
        let mut parser = self.registry.create_parser(lang_config);
        let Some(tree) = parser.parse(&context_text, None) else {
            return self.fallback_indent_with_rope(rope, cursor_line);
        };

        // Calculate which line within context window
        let line_in_context = cursor_line - context_start_line;

        self.tree_based_indent(&context_text, line_in_context, &tree, lang_config)
    }

    fn tree_based_indent(
        &self,
        text: &str,
        cursor_line: usize,
        tree: &Tree,
        _config: &LanguageConfig,
    ) -> String {
        let lines: Vec<&str> = text.lines().collect();

        if cursor_line >= lines.len() {
            return String::new();
        }

        let current_line = lines[cursor_line];
        let current_indent = Self::get_line_indent(current_line);

        let byte_offset: usize = lines
            .iter()
            .take(cursor_line + 1)
            .map(|line| line.len() + 1)
            .sum::<usize>()
            .saturating_sub(1);

        let root = tree.root_node();
        let node_at_cursor = self.find_node_at_position(root, byte_offset);

        let should_indent = self.should_increase_indent(&node_at_cursor, current_line);
        let should_dedent = self.should_decrease_indent(&node_at_cursor, current_line);

        if should_dedent {
            if current_indent.len() >= self.indent_width {
                current_indent[..current_indent.len() - self.indent_width].to_string()
            } else {
                String::new()
            }
        } else if should_indent {
            format!("{}{}", current_indent, " ".repeat(self.indent_width))
        } else {
            current_indent
        }
    }

    fn find_node_at_position<'a>(&self, node: Node<'a>, byte_offset: usize) -> Node<'a> {
        let mut current = node;

        loop {
            let mut found_child = false;

            for child in current.children(&mut current.walk()) {
                if child.start_byte() <= byte_offset && byte_offset <= child.end_byte() {
                    current = child;
                    found_child = true;
                    break;
                }
            }

            if !found_child {
                break;
            }
        }

        current
    }

    fn should_increase_indent(&self, node: &Node, line: &str) -> bool {
        let kind = node.kind();
        let trimmed = line.trim();

        let indent_nodes = [
            "block",
            "statement_block",
            "function_item",
            "function_declaration",
            "impl_item",
            "struct_item",
            "enum_item",
            "match_arm",
            "closure_expression",
            "if_statement",
            "for_statement",
            "while_statement",
            "loop_expression",
            "class_declaration",
            "class_definition",
            "object",
            "array",
        ];

        if indent_nodes.contains(&kind) {
            return true;
        }

        if trimmed.ends_with('{') || trimmed.ends_with('[') || trimmed.ends_with('(') {
            let opens = trimmed.matches('{').count()
                + trimmed.matches('[').count()
                + trimmed.matches('(').count();
            let closes = trimmed.matches('}').count()
                + trimmed.matches(']').count()
                + trimmed.matches(')').count();
            return opens > closes;
        }

        if trimmed.ends_with(':') && !trimmed.starts_with('#') {
            return true;
        }

        false
    }

    fn should_decrease_indent(&self, _node: &Node, line: &str) -> bool {
        let trimmed = line.trim();

        if trimmed.starts_with('}') || trimmed.starts_with(']') || trimmed.starts_with(')') {
            return true;
        }

        let dedent_keywords = ["else", "elif", "except", "finally", "case"];
        for keyword in dedent_keywords {
            if trimmed.starts_with(keyword) {
                return true;
            }
        }

        false
    }

    fn get_line_indent(line: &str) -> String {
        line.chars()
            .take_while(|c| c.is_whitespace() && *c != '\n')
            .collect()
    }

    /// 🚀 NEW: Fallback indent using Rope (efficient)
    fn fallback_indent_with_rope(&self, rope: &crate::rope::Rope, cursor_line: usize) -> String {
        // Get just the current line efficiently
        if let Some(line_text) = rope.line(cursor_line) {
            let indent = Self::get_line_indent(&line_text);
            let trimmed = line_text.trim();

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
        } else {
            String::new()
        }
    }

    /// Original fallback for legacy string-based API
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
