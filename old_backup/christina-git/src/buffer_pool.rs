use std::cell::RefCell;

use christina_core::types::FilePath;

/// A reusable string buffer for building chunk content.
pub(crate) struct ChunkBuffer {
    content: String,
    file_paths: Vec<FilePath>,
}

impl ChunkBuffer {
    fn new() -> Self {
        Self {
            // Pre-allocate 4KB for content, typical chunk size
            content: String::with_capacity(4096),
            file_paths: Vec::with_capacity(4),
        }
    }

    /// Clear the buffer for reuse without deallocating.
    pub(crate) fn clear(&mut self) {
        self.content.clear();
        self.file_paths.clear();
    }

    /// Get mutable access to the content buffer.
    pub(crate) fn content_mut(&mut self) -> &mut String {
        &mut self.content
    }

    /// Get mutable access to the file paths buffer.
    pub(crate) fn file_paths_mut(&mut self) -> &mut Vec<FilePath> {
        &mut self.file_paths
    }

    /// Take ownership of the content, replacing it with a new buffer.
    pub(crate) fn take_content(&mut self) -> String {
        std::mem::replace(&mut self.content, String::with_capacity(4096))
    }

    /// Take ownership of the file paths, replacing it with a new buffer.
    pub(crate) fn take_file_paths(&mut self) -> Vec<FilePath> {
        std::mem::replace(&mut self.file_paths, Vec::with_capacity(4))
    }

    /// Check if the content buffer is empty.
    pub(crate) fn is_empty(&self) -> bool {
        self.content.is_empty()
    }
}

// Thread-local buffer pool for chunking operations.
thread_local! {
    static BUFFER_POOL: RefCell<Vec<ChunkBuffer>> = const { RefCell::new(Vec::new()) };
}

pub(crate) fn acquire_buffer() -> ChunkBuffer {
    BUFFER_POOL.with(|pool| {
        let mut pool = pool.borrow_mut();
        match pool.pop() {
            Some(mut buffer) => {
                buffer.clear();
                buffer
            }
            None => ChunkBuffer::new(),
        }
    })
}

pub(crate) fn release_buffer(buffer: ChunkBuffer) {
    BUFFER_POOL.with(|pool| {
        let mut pool = pool.borrow_mut();
        // Limit pool size to prevent unbounded growth
        if pool.len() < 16 {
            pool.push(buffer);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buffer_reuse() {
        let mut buf1 = acquire_buffer();
        buf1.content_mut().push_str("test content");
        let addr1 = buf1.content_mut().as_ptr();
        release_buffer(buf1);

        let mut buf2 = acquire_buffer();
        let addr2 = buf2.content_mut().as_ptr();
        release_buffer(buf2);

        // Should reuse the same buffer allocation
        assert_eq!(addr1, addr2);
    }

    #[test]
    fn buffer_cleared_on_acquire() {
        let mut buf = acquire_buffer();
        buf.content_mut().push_str("old content");
        buf.file_paths_mut().push(FilePath::from("old_file.txt"));
        release_buffer(buf);

        let mut buf = acquire_buffer();
        assert!(buf.is_empty());
        assert!(buf.file_paths_mut().is_empty());
        release_buffer(buf);
    }

    #[test]
    fn buffer_pool_limit() {
        // Fill pool beyond limit
        for _ in 0..20 {
            let buf = acquire_buffer();
            release_buffer(buf);
        }

        // Pool should not exceed limit
        BUFFER_POOL.with(|pool| {
            assert!(pool.borrow().len() <= 16);
        });
    }
}
