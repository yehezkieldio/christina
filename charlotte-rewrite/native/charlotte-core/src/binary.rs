//! Binary content detection, ported from Christina's
//! `christina/src/git/diff_processor.rs` (`DiffProcessor::is_binary_content`)
//! and the extension list in `christina-core/src/git/stage.rs`
//! (`BINARY_EXTENSIONS`). The FFI signature in `02-native-core-and-ffi.md`
//! is `is_binary_content(bytes, path) -> boolean`: raw file bytes plus an
//! explicit path, matching `05-diff-processing-and-chunking.md`'s 8 KB
//! sample bound.

/// Matches Christina's `BINARY_EXTENSIONS`.
const BINARY_EXTENSIONS: &[&str] = &[
    ".png", ".jpg", ".jpeg", ".gif", ".bmp", ".ico", ".svg", ".pdf", ".zip", ".tar", ".gz", ".rar", ".7z", ".exe",
    ".dll", ".so", ".dylib", ".wasm", ".pyc", ".class", ".ttf", ".otf", ".woff", ".woff2", ".mp3", ".mp4", ".avi",
    ".mov", ".mkv", ".db", ".sqlite", ".bin",
];

/// The size of the leading sample scanned for a NUL byte. A NUL byte
/// anywhere in the first 8 KB of a file is a reliable binary signal;
/// scanning further only costs time on files this check already can't help
/// (see `05-diff-processing-and-chunking.md`'s "same 8 KB sample bound").
const SAMPLE_SIZE: usize = 8 * 1024;

fn has_binary_extension(path: &str) -> bool {
    let path_lower = path.to_ascii_lowercase();
    BINARY_EXTENSIONS.iter().any(|ext| path_lower.ends_with(ext))
}

fn has_nul_byte_in_sample(bytes: &[u8]) -> bool {
    let sample_len = bytes.len().min(SAMPLE_SIZE);
    bytes[..sample_len].contains(&0)
}

/// Detects binary content by sampling up to 8 KB for a NUL byte, then
/// falling back to a known-extension check on `path`.
pub fn is_binary_content(bytes: &[u8], path: &str) -> bool {
    if bytes.is_empty() {
        return has_binary_extension(path);
    }

    has_nul_byte_in_sample(bytes) || has_binary_extension(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_nul_byte_at_start_of_sample() {
        let mut bytes = vec![0u8];
        bytes.extend_from_slice(b"rest of content");
        assert!(is_binary_content(&bytes, "file.dat"));
    }

    #[test]
    fn detects_nul_byte_within_8kb_sample() {
        let mut bytes = vec![b'a'; 4000];
        bytes.push(0);
        bytes.extend(vec![b'b'; 4000]);
        assert!(is_binary_content(&bytes, "file.dat"));
    }

    #[test]
    fn ignores_nul_byte_beyond_8kb_sample() {
        let mut bytes = vec![b'a'; SAMPLE_SIZE + 100];
        bytes.push(0);
        assert!(!is_binary_content(&bytes, "file.rs"));
    }

    #[test]
    fn detects_known_binary_extension_with_clean_bytes() {
        let bytes = b"not actually binary content".to_vec();
        assert!(is_binary_content(&bytes, "src/assets/image.png"));
    }

    #[test]
    fn text_file_without_nul_or_binary_extension_is_not_binary() {
        let bytes = b"fn main() {\n    println!(\"hello\");\n}\n".to_vec();
        assert!(!is_binary_content(&bytes, "src/main.rs"));
    }

    #[test]
    fn empty_bytes_with_text_extension_is_not_binary() {
        assert!(!is_binary_content(&[], "readme.txt"));
    }

    #[test]
    fn empty_bytes_with_binary_extension_is_binary() {
        assert!(is_binary_content(&[], "logo.png"));
    }

    #[test]
    fn extension_check_is_case_insensitive() {
        let bytes = b"clean content".to_vec();
        assert!(is_binary_content(&bytes, "IMAGE.PNG"));
    }

    #[test]
    fn large_clean_text_file_is_not_binary() {
        let bytes = "function test() { return 'hello world'; }\n".repeat(100_000).into_bytes();
        assert!(!is_binary_content(&bytes, "bundle.js"));
    }
}
