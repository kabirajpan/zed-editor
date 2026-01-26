use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::path::Path;

/// Read file contents
pub fn read_file<P: AsRef<Path>>(path: P) -> io::Result<String> {
    std::fs::read_to_string(path)
}

/// Read large file line by line (for huge files)
pub fn read_file_chunked<P: AsRef<Path>>(path: P, max_size: usize) -> io::Result<String> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);

    let mut result = String::new();
    let mut total_size = 0;

    for line in reader.lines() {
        let line = line?;

        // Check if we've hit max size
        if total_size + line.len() > max_size {
            result.push_str(&format!("\n... [File truncated - {} bytes max]", max_size));
            break;
        }

        result.push_str(&line);
        result.push('\n');
        total_size += line.len() + 1;
    }

    Ok(result)
}
