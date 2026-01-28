pub mod buffer;
pub mod editor;
pub mod formatter;
pub mod gui;
pub mod history;
pub mod io;
pub mod rope;
pub mod syntax; // ADD THIS
pub mod tree;
pub mod ui;
pub mod util;

// Re-export commonly used types
pub use buffer::{Buffer, Offset, Point};
pub use editor::{Editor, Selection};
pub use formatter::{FormatResult, Formatter, FormatterConfig, FormatterProvider};
pub use gui::GuiApp;
pub use history::{History, Transaction};
pub use io::{read_file, write_file};
pub use rope::{Chunk, Rope, TextMetrics};
pub use syntax::{IndentCalculator, SyntaxHighlighter, SyntaxTheme}; // ADD THIS
pub use tree::{Count, Item, SumTree, Summary, TextSummary};
pub use ui::{render, App};
