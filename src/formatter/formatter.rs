use std::path::Path;

#[derive(Debug, Clone)]
pub enum FormatError {
    ExecutionFailed(String),
    NotFound(String),
    InvalidOutput(String),
    UnsupportedLanguage(String),
}

pub type FormatResult = Result<String, FormatError>;

/// Trait that all formatter providers must implement
pub trait FormatterProvider: Send + Sync {
    /// Name of the formatter (e.g., "rustfmt", "prettier")
    fn name(&self) -> &str;

    /// File extensions this formatter supports (e.g., ["rs"], ["js", "ts"])
    fn supported_extensions(&self) -> &[&str];

    /// Check if the formatter binary is available
    fn is_available(&self) -> bool;

    /// Format the given text
    fn format(&self, text: &str, file_path: Option<&Path>) -> FormatResult;
}

/// Main formatter manager
pub struct Formatter {
    providers: Vec<Box<dyn FormatterProvider>>,
}

impl Formatter {
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
        }
    }

    /// Register a formatter provider
    pub fn register(&mut self, provider: Box<dyn FormatterProvider>) {
        self.providers.push(provider);
    }

    /// Find appropriate formatter for a file
    pub fn find_provider(&self, file_path: &Path) -> Option<&dyn FormatterProvider> {
        let extension = file_path.extension()?.to_str()?;

        self.providers
            .iter()
            .find(|p| p.supported_extensions().contains(&extension))
            .map(|p| p.as_ref())
    }

    /// Format text using the appropriate provider
    pub fn format_text(&self, text: &str, file_path: Option<&Path>) -> FormatResult {
        if let Some(path) = file_path {
            if let Some(provider) = self.find_provider(path) {
                if !provider.is_available() {
                    return Err(FormatError::NotFound(format!(
                        "{} is not installed or not in PATH",
                        provider.name()
                    )));
                }
                return provider.format(text, Some(path));
            }
            return Err(FormatError::UnsupportedLanguage(format!(
                "No formatter found for {:?}",
                path.extension()
            )));
        }

        Err(FormatError::UnsupportedLanguage(
            "Cannot determine language without file path".to_string(),
        ))
    }

    /// Get list of available formatters
    pub fn available_formatters(&self) -> Vec<&str> {
        self.providers
            .iter()
            .filter(|p| p.is_available())
            .map(|p| p.name())
            .collect()
    }
}

impl Default for Formatter {
    fn default() -> Self {
        Self::new()
    }
}
