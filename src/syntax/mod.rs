pub mod highlighter;
pub mod indent;
pub mod languages;
pub mod theme;

pub mod instant_highlighter;
pub use highlighter::{HighlightSpan, SyntaxHighlighter};
pub use indent::IndentCalculator;
pub use instant_highlighter::{Highlight, HighlightedRange, InstantHighlighter};
pub use languages::{LanguageConfig, LanguageId, LanguageRegistry};
pub use theme::SyntaxTheme;
