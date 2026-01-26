pub mod buffer;
pub mod editor;
pub mod gui;
pub mod history;
pub mod io;
pub mod rope;
pub mod tree;
pub mod ui;
pub mod util;

// Re-export commonly used types
pub use buffer::{Buffer, Offset, Point}; // REMOVED LineIndex, ProgressiveIndexer
pub use editor::{Editor, Selection};
pub use gui::GuiApp;
pub use history::{History, Transaction};
pub use io::{read_file, write_file};
pub use rope::{Chunk, Rope, TextMetrics};
pub use tree::{Count, Item, SumTree, Summary, TextSummary};
pub use ui::{render, App};
