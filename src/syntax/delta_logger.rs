use tree_sitter::{Node, Tree, TreeCursor};
use std::collections::HashMap;
use std::hash::{Hash, Hasher, DefaultHasher};

#[derive(Debug, Clone, serde::Serialize)]
pub enum EditType {
    Modified,
    Added,
    Deleted,
    Relocated,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SemanticDelta {
    pub node_path: String,
    pub edit_type: EditType,
    pub old_byte_range: Option<(usize, usize)>,
    pub new_byte_range: Option<(usize, usize)>,
}

#[derive(Clone)]
pub struct DeltaGenerator {
    // Maps NodePath -> ContentHash
    fingerprints: HashMap<String, (u64, (usize, usize))>,
}

impl DeltaGenerator {
    pub fn new() -> Self {
        Self {
            fingerprints: HashMap::new(),
        }
    }

    /// Capture a snapshot of the current state to compare against later.
    pub fn snapshot(&mut self, tree: &Tree, text: &str) {
        self.fingerprints = self.capture_fingerprints(tree, text);
    }

    /// Generate deltas between the last snapshot and the current tree.
    pub fn generate_deltas(&self, new_tree: &Tree, new_text: &str) -> Vec<SemanticDelta> {
        let new_fingerprints = self.capture_fingerprints(new_tree, new_text);
        let mut deltas = Vec::new();

        // 1. Find Modified and Relocated (in new, maybe in old)
        for (path, (new_hash, new_range)) in &new_fingerprints {
            if let Some((old_hash, old_range)) = self.fingerprints.get(path) {
                if old_hash != new_hash {
                    deltas.push(SemanticDelta {
                        node_path: path.clone(),
                        edit_type: EditType::Modified,
                        old_byte_range: Some(*old_range),
                        new_byte_range: Some(*new_range),
                    });
                } else if old_range != new_range {
                    deltas.push(SemanticDelta {
                        node_path: path.clone(),
                        edit_type: EditType::Relocated,
                        old_byte_range: Some(*old_range),
                        new_byte_range: Some(*new_range),
                    });
                }
            } else {
                deltas.push(SemanticDelta {
                    node_path: path.clone(),
                    edit_type: EditType::Added,
                    old_byte_range: None,
                    new_byte_range: Some(*new_range),
                });
            }
        }

        // 2. Find Deleted (in old, not in new)
        for (path, (_old_hash, old_range)) in &self.fingerprints {
            if !new_fingerprints.contains_key(path) {
                deltas.push(SemanticDelta {
                    node_path: path.clone(),
                    edit_type: EditType::Deleted,
                    old_byte_range: Some(*old_range),
                    new_byte_range: None,
                });
            }
        }

        deltas
    }

    fn capture_fingerprints(&self, tree: &Tree, text: &str) -> HashMap<String, (u64, (usize, usize))> {
        let mut fingerprints = HashMap::new();
        let mut cursor = tree.walk();
        self.walk_semantic_nodes(&mut cursor, text, &mut fingerprints, "");
        fingerprints
    }

    fn walk_semantic_nodes(
        &self,
        cursor: &mut TreeCursor,
        text: &str,
        map: &mut HashMap<String, (u64, (usize, usize))>,
        parent_path: &str,
    ) {
        let node = cursor.node();
        let kind = node.kind();
        
        if is_semantic_node(kind) {
            if let Some(name) = get_node_name(node, text) {
                let path = if parent_path.is_empty() {
                    format!("{} {}", kind, name)
                } else {
                    format!("{} > {} {}", parent_path, kind, name)
                };

                let content = &text[node.start_byte()..node.end_byte()];
                let mut hasher = DefaultHasher::new();
                content.hash(&mut hasher);
                let hash = hasher.finish();

                map.insert(path.clone(), (hash, (node.start_byte(), node.end_byte())));

                // Recurse into children to find nested semantic nodes
                if cursor.goto_first_child() {
                    self.walk_semantic_nodes(cursor, text, map, &path);
                    cursor.goto_parent();
                }
            }
        } else {
            // Not a semantic node, but its children might be
            if cursor.goto_first_child() {
                loop {
                    self.walk_semantic_nodes(cursor, text, map, parent_path);
                    if !cursor.goto_next_sibling() {
                        break;
                    }
                }
                cursor.goto_parent();
            }
        }
    }
}

fn is_semantic_node(kind: &str) -> bool {
    match kind {
        "function_item" | "struct_item" | "enum_item" | "trait_item" | "impl_item" | "mod_item" |
        "class_declaration" | "function_declaration" | "method_declaration" => true,
        _ => false,
    }
}

fn get_node_name(node: Node, text: &str) -> Option<String> {
    for field in ["name", "identifier"] {
        if let Some(child) = node.child_by_field_name(field) {
            return Some(text[child.start_byte()..child.end_byte()].to_string());
        }
    }
    None
}
