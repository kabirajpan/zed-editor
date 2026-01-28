pub mod config;
pub mod formatter;
pub mod providers;

pub use config::FormatterConfig;
pub use formatter::{FormatError, FormatResult, Formatter, FormatterProvider}; // ADD FormatError here
