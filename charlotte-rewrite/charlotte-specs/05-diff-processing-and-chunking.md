# Diff processing and chunking

## Why this module stays in Rust

This module holds the busiest functions in Christina's own codebase graph. `TokenCount::new_at_least_one` and `ChunkBuffer::is_empty` each show up in 64 call sites. The chunking and diff-processing functions around them carry the largest test suite in the project: over 20 dedicated tests for chunking alone, plus a further 30 for binary detection and truncation in `diff_processor.rs`. This is the strongest case in the whole codebase for keeping code in Rust rather than moving it to TypeScript.

## Binary detection

Christina's `DiffProcessor::is_binary_content` (`christina/src/git/diff_processor.rs`) samples up to 8 KB of file content and scans for a null byte. It also checks the file extension against a known binary list: images, archives, fonts, and video. Charlotte's native `is_binary_content` keeps both checks and the same 8 KB sample bound.

## Truncation

Christina truncates an oversized diff to a token budget before chunking (`process_borrowed`, `truncate_to_token_limit`, `truncate_to_token_limit_fallback`). A deletion-only diff gets a smaller limit. Every other diff gets a larger one. Charlotte's native chunker keeps this two-tier limit.

## Chunking algorithm

Christina's `split_recursive` in `christina-core/src/processing/chunking.rs` tries three strategies in order.

1. Split by diff hunk. Each hunk becomes a candidate chunk boundary.
2. When a single hunk still exceeds the token limit, split by line.
3. When a single line still exceeds the token limit, split by raw token span (`split_oversized_line_by_tokens`, with a search-based fallback in `split_oversized_line_by_search`).

Lockfiles get a separate, smaller token cap (`LOCKFILE_TOKEN_LIMIT`), so dependency-lock churn does not crowd out the rest of the prompt budget. Charlotte's native `chunk_diff` keeps this three-tier fallback and the lockfile cap, unchanged in shape from Christina.

## Tokenization

Christina counts tokens with `tiktoken-rs` (`christina-core/src/processing/tokenizer.rs`). `TokenCount` (`christina-core/src/types/tokens.rs`) is a newtype (a wrapper type that gives a raw number a named, checked meaning). It rejects a zero count where a caller needs at least one token. Charlotte's native `count_tokens` wraps the same `tiktoken-rs` crate, so token counts match Christina's exactly for the same input and the same model encoding.

## TypeScript-side responsibility

Once a chunk list crosses the FFI boundary into TypeScript, all downstream work is orchestration, not computation. This covers concurrent summarization, prompt assembly, and the AI SDK call, and it stays in TypeScript per `01-architecture.md`.

## Open decision

Christina's `ChunkBuffer` uses a buffer pool (`acquire_buffer`, `release_buffer`) to cut allocation overhead during recursive splitting. Charlotte needs a decision on whether this pool carries over into the native core unchanged. The FFI boundary's own serialization cost can already dominate the runtime. If so, the buffer pool becomes a needless complication. Settle this with a benchmark on a representative large diff before writing the pooling code.
