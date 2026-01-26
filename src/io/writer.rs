use std::fs;
use std::io;
use std::path::Path;

/// Write contents to file
pub fn write_file<P: AsRef<Path>>(path: P, contents: &str) -> io::Result<()> {
    fs::write(path, contents)
}
