use std::path::Path;
use std::fs;

/// Scans the project directory and returns a string representation of the file tree.
pub fn get_project_structure(root: &Path) -> String {
    let mut tree = String::new();
    walk_dir(root, 0, &mut tree);
    tree
}

fn walk_dir(dir: &Path, depth: usize, tree: &mut String) {
    if depth > 3 { return; } // Limit depth for token safety
    
    if let Ok(entries) = fs::read_dir(dir) {
        let mut entries: Vec<_> = entries.filter_map(|e| e.ok()).collect();
        entries.sort_by_key(|e| e.file_name());

        for entry in entries {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();

            // Ignore common noise
            if name == "target" || name == ".git" || name == "node_modules" {
                continue;
            }

            for _ in 0..depth { tree.push_str("  "); }
            if path.is_dir() {
                tree.push_str(&format!("📁 {}/\n", name));
                walk_dir(&path, depth + 1, tree);
            } else {
                tree.push_str(&format!("📄 {}\n", name));
            }
        }
    }
}
