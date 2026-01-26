use std::fs::File;
use std::io::{self, Result};
use std::path::Path;

/// Memory-mapped file reader for instant access to large files
/// This is how VSCode/Zed handle large files - OS does the paging
pub struct MmapReader {
    #[cfg(unix)]
    mmap: memmap2::Mmap,
    #[cfg(not(unix))]
    _file: File,
    len: usize,
}

impl MmapReader {
    /// Open a file using memory mapping
    /// Returns instantly even for GB files - OS handles paging
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let file = File::open(path)?;
        let metadata = file.metadata()?;
        let len = metadata.len() as usize;

        #[cfg(unix)]
        {
            let mmap = unsafe { memmap2::Mmap::map(&file)? };
            Ok(Self { mmap, len })
        }

        #[cfg(not(unix))]
        {
            // Fallback for non-Unix systems
            Ok(Self { _file: file, len })
        }
    }

    /// Get file size
    pub fn len(&self) -> usize {
        self.len
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Get a chunk of bytes from the file
    /// This is lazy - only loads from disk when accessed
    #[cfg(unix)]
    pub fn chunk(&self, start: usize, len: usize) -> &[u8] {
        let end = (start + len).min(self.len);
        &self.mmap[start..end]
    }

    #[cfg(not(unix))]
    pub fn chunk(&self, _start: usize, _len: usize) -> &[u8] {
        &[]
    }

    /// Convert a chunk to string (with UTF-8 validation)
    pub fn chunk_as_str(&self, start: usize, len: usize) -> Result<String> {
        let bytes = self.chunk(start, len);
        String::from_utf8(bytes.to_vec()).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }

    /// Read entire file as string (for small files only!)
    #[cfg(unix)]
    pub fn as_str(&self) -> Result<&str> {
        std::str::from_utf8(&self.mmap).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }

    #[cfg(not(unix))]
    pub fn as_str(&self) -> Result<&str> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Memory mapping not supported on this platform",
        ))
    }
}

/// Streaming chunk iterator for large files
pub struct ChunkIterator<'a> {
    reader: &'a MmapReader,
    pos: usize,
    chunk_size: usize,
}

impl<'a> ChunkIterator<'a> {
    pub fn new(reader: &'a MmapReader, chunk_size: usize) -> Self {
        Self {
            reader,
            pos: 0,
            chunk_size,
        }
    }
}

impl<'a> Iterator for ChunkIterator<'a> {
    type Item = &'a [u8];

    fn next(&mut self) -> Option<Self::Item> {
        if self.pos >= self.reader.len() {
            return None;
        }

        let chunk = self.reader.chunk(self.pos, self.chunk_size);
        self.pos += chunk.len();

        if chunk.is_empty() {
            None
        } else {
            Some(chunk)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    #[cfg(unix)]
    fn test_mmap_basic() {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        file.write_all(b"Hello, World!").unwrap();
        file.flush().unwrap();

        let reader = MmapReader::open(file.path()).unwrap();
        assert_eq!(reader.len(), 13);
        assert_eq!(reader.as_str().unwrap(), "Hello, World!");
    }

    #[test]
    #[cfg(unix)]
    fn test_mmap_chunked() {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        file.write_all(b"0123456789").unwrap();
        file.flush().unwrap();

        let reader = MmapReader::open(file.path()).unwrap();
        let chunk = reader.chunk(0, 5);
        assert_eq!(chunk, b"01234");

        let chunk = reader.chunk(5, 5);
        assert_eq!(chunk, b"56789");
    }
}
