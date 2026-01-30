# Diff Processor Documentation

The `DiffProcessor` is a core component of the `christina-git` crate designed to prepare Git diffs for consumption by Large Language Models (LLMs). Its primary goal is to transform raw, potentially massive diff strings into semantic, token-limited chunks that fit within an LLM's context window while preserving as much meaningful context as possible.

## 1. Overview

LLMs have finite context windows. Raw Git diffs can easily exceed these limits, especially when including lockfiles, large deletions, or binary data. The `DiffProcessor` handles this by:

1.  **Sanitizing:** Detecting and summarizing binary files.
2.  **Optimizing:** Truncating deletion-only changes (since LLMs don't need full context to understand a file was deleted).
3.  **Chunking:** Intelligently splitting large diffs into smaller, self-contained `DiffChunk` objects based on token counts.

## 2. Architecture

### Core Struct: `DiffProcessor`
Located in `src/diff_processor.rs`, this struct configures the processing behavior.

```rust
pub struct DiffProcessor<'a> {
    tokenizer: &'a dyn Tokenizer,  // Abstraction for counting tokens (e.g., BPE)
    token_limit: TokenCount,       // Max tokens per chunk
    ignore_files: Vec<String>,     // Patterns to treat specially (e.g., lockfiles)
    max_diff_size: usize,          // Hard safety limit for input size
}
```

### Key Dependencies
-   **`Tokenizer` Trait:** Allows the processor to be agnostic of the specific LLM provider (OpenAI, Anthropic, etc.).
-   **`buffer_pool`:** A thread-local pool of reusable buffers to minimize memory allocation overhead during heavy processing.

## 3. The Processing Pipeline

When `process(diff)` is called, the data flows through these stages:

### Stage 1: Safety & Sanity Checks
Before parsing, the processor checks if the diff is viable:
-   **Max Size:** If the diff exceeds `MAX_DIFF_SIZE` (default ~10MB), it is truncated or split immediately to avoid OOM (Out of Memory) issues.
-   **Binary Detection:** It scans content for NUL bytes (`\0`) or specific git headers (e.g., `Binary files ... differ`, `GIT binary patch`) to avoid sending garbage to the LLM. Binary files are replaced with a `[Binary file: <path>]` placeholder.

### Stage 2: Semantic Optimization
The processor analyzes the *nature* of the changes to save tokens:
-   **Deletion-Only Diffs:** If a diff contains *only* deletions (lines starting with `-` and no `+`), it is heavily truncated. The reasoning is that an LLM only needs to know *what* was deleted or that a file was removed; it doesn't need to read every deleted line to generate a commit message like "chore: remove unused assets".

### Stage 3: Parsing
The raw diff string is split into individual **File Diffs** using `parsing::split_by_files`.
-   It looks for the `diff --git` header.
-   It parses file paths, handling renames, copies, and quoted paths.
-   It ensures that headers are strictly at the start of lines to prevent injection attacks.

### Stage 4: Recursive Chunking (The Core Algorithm)
This is the most complex part, located in `src/chunking.rs`. The `split_recursive` function attempts to pack content into chunks using a "best-fit" strategy that degrades gracefully:

1.  **File Level:**
    -   It iterates through the list of file diffs.
    -   If a whole file's diff fits in the current `ChunkBuffer` (under `token_limit`), it is added.
    -   **Lockfile Handling:** If a file matches `ignore_files` (e.g., `Cargo.lock`), it is strictly capped (e.g., 100 tokens) to prevent auto-generated files from hogging context.

2.  **Hunk Level:**
    -   If a single file is too large to fit in a chunk, it is split by **Hunks** (`@@ ... @@` blocks).
    -   The processor preserves the file header (`diff --git ...`) for *every* chunk belonging to that file, ensuring the LLM always knows which file the context belongs to.

3.  **Line Level:**
    -   If a single hunk is still too large (rare, but possible with massive insertions), it falls back to splitting by **Lines**.

4.  **Token Truncation (Last Resort):**
    -   If a single line exceeds the limit (e.g., a minified JS file on one line), it uses `truncate_to_token_limit` to perform a hard cut based on token count, ensuring strict adherence to the limit.

## 4. Key Features

### Binary File Detection
The processor employs heuristics to detect binary files:
-   Checks for NUL bytes in the first 8KB.
-   Checks for known binary extensions in the file path (png, jpg, pdf, zip, etc.).
-   Checks for specific Git output markers.

### Smart Deletion Truncation
```rust
// Logic in src/parsing.rs
pub fn truncate_deletion_diff(content: &str, max_deletion_lines: usize) -> String {
    // Keeps metadata headers
    // Keeps the first N lines of deletions
    // Appends "[... deleted content truncated ...]"
}
```

### Memory Management
The module uses a custom `ChunkBuffer` and `buffer_pool` to reuse memory allocations. Since diff processing can involve creating thousands of small strings, this reduces allocator pressure significantly.

## 5. Usage Example

```rust
use christina_git::DiffProcessor;
use christina_core::types::TokenCount;

// 1. Initialize tokenizer (implementation specific)
let tokenizer = MyLLMTokenizer::new();

// 2. Create processor with a 4k token limit
let processor = DiffProcessor::new(&tokenizer, TokenCount::new(4096))
    .with_ignore_files(vec!["*.lock".to_string()]);

// 3. Process a raw git diff
let raw_diff = repo.get_staged_diff()?.to_string()?;
let chunks = processor.process(&raw_diff);

// 4. Use chunks
for chunk in chunks {
    println!("Chunk files: {:?}", chunk.files);
    println!("Token count: {}", chunk.token_count);
    // send chunk.content to LLM...
}
```
