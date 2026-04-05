use crate::syntax::languages::{LanguageConfig, LanguageId, LanguageRegistry};
use crate::syntax::theme::SyntaxTheme;
use egui::Color32;
use std::cmp::Reverse;
use std::collections::HashMap;
use std::path::Path;
use tree_sitter::{InputEdit, Parser, Point as TsPoint, Query, QueryCursor, Tree};

#[derive(Debug, Clone)]
pub struct HighlightSpan {
    pub start: usize,
    pub end: usize,
    pub color: Color32,
}

struct ParseState {
    tree: Tree,
    language_id: LanguageId,
    /// Always stored WITH a trailing '\n' to match what highlight_viewport
    /// feeds to the parser.  notify_edit must uphold this invariant.
    text: String,
}

pub struct SyntaxHighlighter {
    registry: LanguageRegistry,
    theme: SyntaxTheme,
    parser: Parser,
    query_cache: HashMap<LanguageId, Query>,
    parse_state: Option<ParseState>,
    highlight_cache: Option<(u64, HashMap<usize, Vec<(usize, usize, Color32)>>)>,
}

impl SyntaxHighlighter {
    pub fn new(theme: SyntaxTheme) -> Self {
        Self {
            registry: LanguageRegistry::new(),
            theme,
            parser: Parser::new(),
            query_cache: HashMap::new(),
            parse_state: None,
            highlight_cache: None,
        }
    }

    /// Invalidate the highlight cache (line-level cache miss next frame).
    pub fn invalidate(&mut self) {
        self.highlight_cache = None;
    }

    /// Full reset: discard the cached parse tree so the next highlight pass
    /// does a clean full parse.  Use this after undo/redo/load/format — any
    /// operation that jumps the buffer to an arbitrary prior state.
    pub fn reset(&mut self) {
        self.highlight_cache = None;
        self.parse_state = None;
    }

    /// Call this after every incremental rope edit so tree-sitter can reparse
    /// incrementally rather than from scratch.
    ///
    /// `rope` must be the **post-edit** rope.
    /// `start_byte`, `old_end_byte`, `new_end_byte` are byte offsets in the
    /// rope (without any appended newline).
    pub fn notify_edit(
        &mut self,
        rope: &crate::rope::Rope,
        start_byte: usize,
        old_end_byte: usize,
        new_end_byte: usize,
    ) {
        self.highlight_cache = None;

        let parse_state = match self.parse_state.as_mut() {
            Some(s) => s,
            None => return,
        };

        // parse_state.text is always stored with a trailing '\n'.
        // Use it directly for the old positions.
        let start_pos = byte_to_point(&parse_state.text, start_byte);
        let old_end_pos = byte_to_point(&parse_state.text, old_end_byte);

        // Build the new text WITH trailing '\n' to match highlight_viewport.
        let new_text = rope_to_text_with_newline(rope);
        let new_end_pos = byte_to_point(&new_text, new_end_byte);

        let edit = InputEdit {
            start_byte,
            old_end_byte,
            new_end_byte,
            start_position: start_pos,
            old_end_position: old_end_pos,
            new_end_position: new_end_pos,
        };

        parse_state.tree.edit(&edit);
        // Keep parse_state.text in sync (with trailing '\n').
        parse_state.text = new_text;
    }

    fn ensure_query_compiled(&mut self, config: &LanguageConfig) -> bool {
        if self.query_cache.contains_key(&config.id) {
            return true;
        }
        match Query::new(&config.language, config.highlight_query) {
            Ok(q) => {
                self.query_cache.insert(config.id, q);
                true
            }
            Err(e) => {
                eprintln!("[SyntaxHighlighter] Query error for {}: {}", config.name, e);
                false
            }
        }
    }

    pub fn highlight_viewport(
        &mut self,
        rope: &crate::rope::Rope,
        version: u64,
        file_path: Option<&Path>,
        visible_start: usize,
        visible_end: usize,
    ) -> HashMap<usize, Vec<(usize, usize, Color32)>> {
        let path = match file_path {
            Some(p) => p,
            None => return HashMap::new(),
        };

        let lang_config = match self.registry.detect_language(path) {
            Some(c) => c.clone(),
            None => return HashMap::new(),
        };

        // Return cached highlights if version unchanged.
        if let Some((cached_version, ref cached_lines)) = self.highlight_cache {
            if cached_version == version {
                return (visible_start..visible_end)
                    .filter_map(|line| cached_lines.get(&line).map(|s| (line, s.clone())))
                    .collect();
            }
        }

        let need_lang_change = self
            .parse_state
            .as_ref()
            .map(|s| s.language_id != lang_config.id)
            .unwrap_or(true);

        if need_lang_change {
            if let Err(e) = self.parser.set_language(&lang_config.language) {
                eprintln!("[SyntaxHighlighter] Failed to set language: {}", e);
                return HashMap::new();
            }
            self.parse_state = None;
        }

        // Always parse with a trailing '\n' so tree-sitter predicates can't
        // read past the last byte, and so parse_state.text stays consistent
        // with what notify_edit stores.
        let full_text = rope_to_text_with_newline(rope);

        let old_tree = self.parse_state.as_ref().map(|s| &s.tree);

        let tree = match self.parser.parse(&full_text, old_tree) {
            Some(t) => t,
            None => {
                eprintln!("[SyntaxHighlighter] Parse failed");
                return HashMap::new();
            }
        };

        self.parse_state = Some(ParseState {
            tree: tree.clone(),
            language_id: lang_config.id,
            text: full_text.clone(),
        });

        if !self.ensure_query_compiled(&lang_config) {
            return HashMap::new();
        }

        let query = self.query_cache.get(&lang_config.id).unwrap();
        let capture_names: Vec<String> = query
            .capture_names()
            .iter()
            .map(|s| s.to_string())
            .collect();

        let color_map: Vec<Color32> = capture_names
            .iter()
            .map(|name| self.theme.get_color(name))
            .collect();

        let mut all_lines: HashMap<usize, Vec<(usize, usize, Color32)>> = HashMap::new();
        let mut cursor = QueryCursor::new();
        let root_node = tree.root_node();

        for match_ in cursor.matches(query, root_node, full_text.as_bytes()) {
            for capture in match_.captures {
                let node = capture.node;
                let node_start = node.start_position();
                let node_end = node.end_position();
                let color = color_map[capture.index as usize];

                for row in node_start.row..=node_end.row {
                    let line_str = match rope.line(row) {
                        Some(s) => s,
                        None => continue,
                    };
                    let line_char_len = line_str.chars().count();
                    let line_byte_len = line_str.len();

                    let col_start = if row == node_start.row {
                        safe_char_count(&line_str, node_start.column.min(line_byte_len))
                    } else {
                        0
                    };

                    let col_end = if row == node_end.row {
                        safe_char_count(&line_str, node_end.column.min(line_byte_len))
                    } else {
                        line_char_len
                    };

                    if col_end > col_start {
                        all_lines
                            .entry(row)
                            .or_default()
                            .push((col_start, col_end, color));
                    }
                }
            }
        }

        // Sort and deduplicate per line.
        for spans in all_lines.values_mut() {
            spans.sort_by_key(|&(start, end, _)| (start, Reverse(end)));
            let mut merged: Vec<(usize, usize, Color32)> = Vec::new();
            for span in spans.drain(..) {
                if let Some(last) = merged.last() {
                    if span.0 < last.1 {
                        continue;
                    }
                }
                merged.push(span);
            }
            *spans = merged;
        }

        self.highlight_cache = Some((version, all_lines.clone()));

        (visible_start..visible_end)
            .filter_map(|line| all_lines.remove(&line).map(|s| (line, s)))
            .collect()
    }

    pub fn set_theme(&mut self, theme: SyntaxTheme) {
        self.theme = theme;
        self.highlight_cache = None;
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build the canonical text string fed to tree-sitter: rope content + '\n'.
/// Both highlight_viewport and notify_edit must use this so byte offsets are
/// always consistent with what is stored in parse_state.text.
fn rope_to_text_with_newline(rope: &crate::rope::Rope) -> String {
    let mut t = rope.to_string();
    if !t.ends_with('\n') {
        t.push('\n');
    }
    t
}

/// Convert byte offset to tree-sitter Point.
fn byte_to_point(text: &str, byte_offset: usize) -> TsPoint {
    let byte_offset = byte_offset.min(text.len());
    let before = &text[..byte_offset];
    let row = before.chars().filter(|&c| c == '\n').count();
    let col = before
        .rfind('\n')
        .map(|i| byte_offset - i - 1)
        .unwrap_or(byte_offset);
    TsPoint { row, column: col }
}

/// Safely count chars up to a byte offset, clamping to char boundary.
fn safe_char_count(s: &str, byte_offset: usize) -> usize {
    let clamped = (0..=byte_offset.min(s.len()))
        .rev()
        .find(|&i| s.is_char_boundary(i))
        .unwrap_or(0);
    s[..clamped].chars().count()
}
