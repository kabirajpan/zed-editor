use tree_sitter::{Node, Tree};

#[derive(Debug, Clone, serde::Serialize)]
pub struct CodeContext {
    pub function_name: Option<String>,
    pub struct_name: Option<String>,
    pub node_type: String,
    pub parent_types: Vec<String>,
    pub scope_path: String,
}

impl CodeContext {
    pub fn new(tree: &Tree, byte_offset: usize, text: &str) -> Self {
        let root = tree.root_node();
        let node = root
            .descendant_for_byte_range(byte_offset, byte_offset)
            .unwrap_or(root);

        let node_type = node.kind().to_string();
        let mut parent_types = Vec::new();
        let mut simplified_parts = Vec::new();
        let mut function_name = None;
        let mut struct_name = None;

        let mut current = Some(node);
        while let Some(n) = current {
            let kind = n.kind();
            parent_types.push(kind.to_string());

            match kind {
                "function_item" | "function_declaration" | "method_declaration" => {
                    if function_name.is_none() {
                        function_name = get_name_from_node(n, text);
                    }
                }
                "struct_item" | "class_declaration" | "impl_item" | "trait_item" => {
                    if struct_name.is_none() {
                        struct_name = get_name_from_node(n, text);
                    }
                }
                _ => {}
            }

            if is_significant(kind) {
                let mut label = pretty_kind(kind).to_string();
                
                // Add name or preview if available
                if let Some(name) = get_name_from_node(n, text) {
                    label = format!("{} {}", label, name);
                } else if let Some(preview) = get_preview(n, text) {
                    label = format!("{} {}", label, preview);
                }
                
                simplified_parts.push(label);
            }

            current = n.parent();
        }

        let scope_path = simplified_parts
            .iter()
            .rev()
            .cloned()
            .collect::<Vec<_>>()
            .join(" > ");

        Self {
            function_name,
            struct_name,
            node_type,
            parent_types,
            scope_path,
        }
    }
}

fn is_significant(kind: &str) -> bool {
    match kind {
        "source_file" | "block" | "token_tree" | "expression_statement" | 
        "string_content" | "visibility_modifier" | "arguments" | "parameters" => false,
        _ => true,
    }
}

fn pretty_kind(kind: &str) -> &str {
    match kind {
        "function_item" | "function_declaration" => "fn",
        "method_declaration" => "method",
        "struct_item" | "class_declaration" => "struct",
        "impl_item" => "impl",
        "trait_item" => "trait",
        "macro_invocation" => "macro",
        "if_expression" => "if",
        "match_expression" => "match",
        "for_expression" => "for",
        "while_expression" => "while",
        "string_literal" => "str",
        _ => kind,
    }
}

fn get_preview(node: Node, text: &str) -> Option<String> {
    match node.kind() {
        "string_literal" | "identifier" | "type_identifier" => {
            let start = node.start_byte();
            let end = node.end_byte();
            if end <= text.len() {
                let content = &text[start..end];
                let content = content.trim_matches('"');
                if content.len() > 15 {
                    return Some(format!("\"{}...\"", &content[..12]));
                } else {
                    return Some(format!("\"{}\"", content));
                }
            }
        }
        _ => {}
    }
    None
}

fn get_name_from_node(node: Node, text: &str) -> Option<String> {
    // 1. Try common field names first
    for field in ["name", "identifier", "type_identifier", "type"] {
        if let Some(child) = node.child_by_field_name(field) {
            let start = child.start_byte();
            let end = child.end_byte();
            if end <= text.len() {
                return Some(text[start..end].to_string());
            }
        }
    }

    // 2. Fallback to searching all children for specific kinds if fields didn't work
    for i in 0..node.child_count() {
        let child = node.child(i).unwrap();
        if child.kind() == "name" || child.kind() == "identifier" || child.kind() == "type_identifier" {
            let start = child.start_byte();
            let end = child.end_byte();
            if end <= text.len() {
                return Some(text[start..end].to_string());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use tree_sitter::Parser;

    #[test]
    fn test_semantic_context_rust() {
        let text = "fn main() {\n    println!(\"hellow rold\");\n}";
        let mut parser = Parser::new();
        parser.set_language(&tree_sitter_rust::language()).unwrap();
        let tree = parser.parse(text, None).unwrap();

        // Cursor inside the string literal "hellow rold"
        let context = CodeContext::new(&tree, 26, text);
        
        println!("Output Path: {}", context.scope_path);
        
        assert_eq!(context.function_name, Some("main".to_string()));
        assert!(context.scope_path.contains("fn main"));
        assert!(context.scope_path.contains("macro println!"));
        assert!(context.scope_path.contains("str \"hellow rold\""));
    }
}
