use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::Path;

/// 🚀 LEGACY: Write contents to file from string (for backward compatibility)
pub fn write_file<P: AsRef<Path>>(path: P, contents: &str) -> io::Result<()> {
    std::fs::write(path, contents)
}

/// 🚀 ULTIMATE OPTIMIZED: Write from Rope chunk-by-chunk (ZERO string conversion!)
pub fn write_file_from_rope<P: AsRef<Path>>(path: P, rope: &crate::rope::Rope) -> io::Result<()> {
    let file = File::create(path)?;
    let mut writer = BufWriter::new(file);

    // 🚀 Write each chunk directly - NO full string allocation!
    rope.for_each_chunk(|chunk| {
        writer.write_all(chunk.as_bytes()).unwrap();
    });

    writer.flush()?;
    Ok(())
}
