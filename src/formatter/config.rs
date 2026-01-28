use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormatterConfig {
    /// Enable/disable automatic formatting on save
    pub format_on_save: bool,

    /// Timeout for formatter execution in seconds
    pub timeout_seconds: u64,

    /// Per-language formatter settings
    pub language_settings: HashMap<String, LanguageFormatterConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageFormatterConfig {
    /// Enable formatting for this language
    pub enabled: bool,

    /// Formatter to use (e.g., "rustfmt", "prettier")
    pub formatter: String,

    /// Additional arguments to pass to the formatter
    pub args: Vec<String>,
}

impl Default for FormatterConfig {
    fn default() -> Self {
        Self {
            format_on_save: false,
            timeout_seconds: 5,
            language_settings: HashMap::new(),
        }
    }
}

impl FormatterConfig {
    pub fn new() -> Self {
        Self::default()
    }

    /// Get formatter config for a specific language/extension
    pub fn get_language_config(&self, extension: &str) -> Option<&LanguageFormatterConfig> {
        self.language_settings.get(extension)
    }
}
