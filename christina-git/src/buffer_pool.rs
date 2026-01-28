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
