pub mod highlighter;
pub mod indent;
pub mod languages;
pub mod theme;

pub use highlighter::{HighlightSpan, SyntaxHighlighter};
pub use indent::IndentCalculator;
pub use languages::{LanguageConfig, LanguageId, LanguageRegistry};
pub use theme::SyntaxTheme;
