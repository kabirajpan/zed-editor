pub mod mmap_reader;
pub mod reader;
pub mod streaming;
pub mod writer;

pub use mmap_reader::MmapReader;
pub use reader::{read_file, read_file_chunked};
pub use streaming::{FileInfo, StreamingLoader};
pub use writer::{write_file, write_file_from_rope}; // 🚀 NEW: Export efficient rope writer
