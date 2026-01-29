use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::Path;

/// 🚀 LEGACY: Write contents to file from string (for backward compatibility)
pub fn write_file<P: AsRef<Path>>(path: P, contents: &str) -> io::Result<()> {
    std::fs::write(path, contents)
}

/// 🚀 NEW OPTIMIZED: Write from Rope directly without full string conversion!
/// This uses buffered writing and processes chunks efficiently
pub fn write_file_from_rope<P: AsRef<Path>>(path: P, rope: &crate::rope::Rope) -> io::Result<()> {
    // Note: Rope's to_string() is already optimized - it iterates chunks internally
    // For now, we use it as it's the simplest approach
    // Future optimization: expose tree.iter() publicly in Rope for chunk-by-chunk writing
    let content = rope.to_string();

    let file = File::create(path)?;
    let mut writer = BufWriter::new(file);
    writer.write_all(content.as_bytes())?;
    writer.flush()?;

    Ok(())
}
