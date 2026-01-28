use std::path::Path;
use tree_sitter::{Language, Parser};

// Use the safe bindings provided by the crates (lowercase 'language')
use tree_sitter_javascript::language as tree_sitter_javascript_lang;
use tree_sitter_python::language as tree_sitter_python_lang;
use tree_sitter_rust::language as tree_sitter_rust_lang;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LanguageId {
    Rust,
    JavaScript,
    Python,
    Unknown,
}

#[derive(Clone)]
pub struct LanguageConfig {
    pub id: LanguageId,
    pub name: &'static str,
    pub language: Language,
    pub extensions: &'static [&'static str],
    pub indent_query: &'static str,
    pub highlight_query: &'static str,
}

impl LanguageConfig {
    pub fn rust() -> Self {
        Self {
            id: LanguageId::Rust,
            name: "Rust",
            language: tree_sitter_rust_lang(),
            extensions: &["rs"],
            indent_query: include_str!("queries/rust/indents.scm"),
            highlight_query: include_str!("queries/rust/highlights.scm"),
        }
    }

    pub fn javascript() -> Self {
        Self {
            id: LanguageId::JavaScript,
            name: "JavaScript",
            language: tree_sitter_javascript_lang(),
            extensions: &["js", "jsx", "mjs"],
            indent_query: include_str!("queries/javascript/indents.scm"),
            highlight_query: include_str!("queries/javascript/highlights.scm"),
        }
    }

    pub fn python() -> Self {
        Self {
            id: LanguageId::Python,
            name: "Python",
            language: tree_sitter_python_lang(),
            extensions: &["py"],
            indent_query: include_str!("queries/python/indents.scm"),
            highlight_query: include_str!("queries/python/highlights.scm"),
        }
    }
}

#[derive(Clone)]
pub struct LanguageRegistry {
    languages: Vec<LanguageConfig>,
}

impl LanguageRegistry {
    pub fn new() -> Self {
        Self {
            languages: vec![
                LanguageConfig::rust(),
                LanguageConfig::javascript(),
                LanguageConfig::python(),
            ],
        }
    }

    pub fn detect_language(&self, path: &Path) -> Option<&LanguageConfig> {
        let extension = path.extension()?.to_str()?;
        self.languages
            .iter()
            .find(|lang| lang.extensions.contains(&extension))
    }

    pub fn get_language(&self, id: LanguageId) -> Option<&LanguageConfig> {
        self.languages.iter().find(|lang| lang.id == id)
    }

    pub fn create_parser(&self, config: &LanguageConfig) -> Parser {
        let mut parser = Parser::new();
        parser.set_language(&config.language).unwrap();
        parser
    }
}

impl Default for LanguageRegistry {
    fn default() -> Self {
        Self::new()
    }
}
