use std::fs::File;
use std::io::{BufReader, Read, Result};
use std::path::Path;

/// Progress callback for streaming operations
pub type ProgressCallback = Box<dyn FnMut(f32, &str) + Send>;

/// Streaming file loader with progress tracking
pub struct StreamingLoader {
    chunk_size: usize,
}

impl StreamingLoader {
    /// Create new streaming loader
    /// chunk_size: Size of chunks to read (default: 64KB)
    pub fn new(chunk_size: usize) -> Self {
        Self { chunk_size }
    }

    /// Default streaming loader (64KB chunks)
    pub fn default() -> Self {
        Self::new(64 * 1024)
    }

    /// Load file in chunks, calling callback for each chunk
    pub fn load_with_progress<P: AsRef<Path>>(
        &self,
        path: P,
        mut on_chunk: impl FnMut(&str) -> Result<()>,
        mut on_progress: impl FnMut(f32, &str),
    ) -> Result<()> {
        let file = File::open(path.as_ref())?;
        let file_size = file.metadata()?.len() as usize;
        let mut reader = BufReader::with_capacity(self.chunk_size, file);
        let mut buffer = vec![0u8; self.chunk_size];
        let mut bytes_read = 0;

        let filename = path
            .as_ref()
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("file");

        while let Ok(n) = reader.read(&mut buffer) {
            if n == 0 {
                break;
            }

            bytes_read += n;
            let progress = if file_size > 0 {
                (bytes_read as f32 / file_size as f32).min(1.0)
            } else {
                0.0
            };

            // Convert chunk to string (handling UTF-8)
            let chunk = String::from_utf8_lossy(&buffer[..n]);
            on_chunk(&chunk)?;

            // Report progress
            let msg = format!("Loading {} ({:.1}%)", filename, progress * 100.0);
            on_progress(progress, &msg);
        }

        Ok(())
    }

    /// Load file completely (for smaller files)
    pub fn load_complete<P: AsRef<Path>>(
        &self,
        path: P,
        on_progress: impl FnMut(f32, &str),
    ) -> Result<String> {
        let mut result = String::new();

        self.load_with_progress(
            path,
            |chunk| {
                result.push_str(chunk);
                Ok(())
            },
            on_progress,
        )?;

        Ok(result)
    }
}

/// Detect if file is text or binary (simple heuristic)
pub fn is_text_file<P: AsRef<Path>>(path: P) -> Result<bool> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    let mut buffer = [0u8; 512]; // Check first 512 bytes

    let n = reader.read(&mut buffer)?;
    if n == 0 {
        return Ok(true); // Empty file is text
    }

    // Check for null bytes (common in binary files)
    let null_count = buffer[..n].iter().filter(|&&b| b == 0).count();

    // If more than 1% null bytes, probably binary
    Ok((null_count as f32 / n as f32) < 0.01)
}

/// Get file info without loading it
pub struct FileInfo {
    pub size: u64,
    pub is_text: bool,
    pub line_count_estimate: Option<usize>,
}

impl FileInfo {
    pub fn from_path<P: AsRef<Path>>(path: P) -> Result<Self> {
        let file = File::open(path.as_ref())?;
        let metadata = file.metadata()?;
        let size = metadata.len();

        let is_text = is_text_file(path.as_ref())?;

        // Estimate line count by sampling
        let line_count_estimate = if is_text && size > 0 {
            Some(estimate_line_count(path.as_ref(), size)?)
        } else {
            None
        };

        Ok(Self {
            size,
            is_text,
            line_count_estimate,
        })
    }

    pub fn should_stream(&self) -> bool {
        // Stream files larger than 5MB
        self.size > 5 * 1024 * 1024
    }

    pub fn should_mmap(&self) -> bool {
        // Use mmap for files larger than 10MB
        self.size > 10 * 1024 * 1024
    }
}

/// Estimate line count by sampling the file
fn estimate_line_count<P: AsRef<Path>>(path: P, file_size: u64) -> Result<usize> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    let sample_size = (file_size as usize).min(10 * 1024); // Sample first 10KB
    let mut buffer = vec![0u8; sample_size];

    let n = reader.read(&mut buffer)?;
    if n == 0 {
        return Ok(0);
    }

    let sample = String::from_utf8_lossy(&buffer[..n]);
    let newlines_in_sample = sample.chars().filter(|&c| c == '\n').count();

    // Estimate based on sample
    if newlines_in_sample > 0 {
        let estimate = (file_size as f64 / n as f64 * newlines_in_sample as f64) as usize;
        Ok(estimate)
    } else {
        Ok(1) // At least 1 line
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_streaming_loader() {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        file.write_all(b"Hello, World!\nLine 2\nLine 3").unwrap();
        file.flush().unwrap();

        let loader = StreamingLoader::new(10); // Small chunks for testing
        let mut chunks = Vec::new();

        loader
            .load_with_progress(
                file.path(),
                |chunk| {
                    chunks.push(chunk.to_string());
                    Ok(())
                },
                |_progress, _msg| {},
            )
            .unwrap();

        let result = chunks.join("");
        assert_eq!(result, "Hello, World!\nLine 2\nLine 3");
    }

    #[test]
    fn test_is_text_file() {
        let mut text_file = tempfile::NamedTempFile::new().unwrap();
        text_file.write_all(b"Hello, World!").unwrap();
        text_file.flush().unwrap();
        assert!(is_text_file(text_file.path()).unwrap());

        let mut binary_file = tempfile::NamedTempFile::new().unwrap();
        binary_file.write_all(&[0, 1, 2, 3, 0, 0, 0]).unwrap();
        binary_file.flush().unwrap();
        assert!(!is_text_file(binary_file.path()).unwrap());
    }
}
