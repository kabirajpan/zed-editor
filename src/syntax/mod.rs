pub mod context;
pub mod highlighter;
pub mod indent;
pub mod languages;
pub mod theme;

pub mod instant_highlighter;
pub mod delta_logger;
pub use context::CodeContext;
pub use highlighter::{HighlightSpan, SyntaxHighlighter};
pub use indent::IndentCalculator;
pub use instant_highlighter::{Highlight, HighlightedRange, InstantHighlighter};
pub use languages::{LanguageConfig, LanguageId, LanguageRegistry};
pub use theme::SyntaxTheme;
