# Language Extension System

This document outlines how to add and configure language support for the Z3N Editor. It covers the current hardcoded registry and the proposed move to a dynamic, file-based configuration system.

## 1. Current System (Centralized)
Currently, languages are defined in `src/syntax/languages.rs`. Adding a language requires:
1. Adding a variant to the `LanguageId` enum.
2. Implementing a constructor in `LanguageConfig` to load Tree-sitter grammars and `.scm` queries.
3. Registering the language in the `LanguageRegistry` constructor.

### Adding Comment Support
Every `LanguageConfig` includes a `line_comment` field. To add a new language, update the struct:
```rust
pub fn my_lang() -> Self {
    Self {
        id: LanguageId::MyLang,
        name: "My Language",
        extensions: &["mylang"],
        line_comment: "//", // Token used for Ctrl + /
        ...
    }
}
```

## 2. Proposed Dynamic System (Decentralized)
To make the editor "future-proof" and pluggable without recompiling, we will transition to a directory-based config system.

### Folder Structure
Each language would reside in its own folder under `src/syntax/languages/`:
```text
languages/
  ├── rust/
  │   ├── highlights.scm
  │   ├── indents.scm
  │   └── config.json   <-- Dynamic Metadata
  └── python/
      ├── highlights.scm
      └── config.json
```

### Proposed `config.json` Schema
The metadata currently in the Rust source will be moved to JSON:
```json
{
  "name": "Rust",
  "extensions": ["rs"],
  "comments": {
    "line": "//",
    "block": ["/*", "*/"]
  },
  "auto_close_pairs": [
    ["(", ")"],
    ["{", "}"],
    ["[", "]"],
    ["\"", "\""],
    ["'", "'"]
  ],
  "indentation": {
    "tab_width": 4,
    "insert_spaces": true
  }
}
```

## 3. Implementation Checklist for Later
- [ ] Refactor `LanguageRegistry` to scan the `languages/` directory on startup.
- [ ] Use `serde_json` to parse `config.json` into `LanguageConfig` objects.
- [ ] Implement a `BlockComment` handler in the Editor for languages without line comments (like HTML).
- [ ] Allow users to override these configs in a global `~/.config/antigravity/languages.toml`.

> [!TIP]
> This system is designed to mimic the professional standards of editors like **Zed** and **VS Code**, making it easy for the community to contribute new language packs.
