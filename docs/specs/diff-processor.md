# Component Spec: Diff Processor

## Overview
The `DiffProcessor` (located in `christina/src/io/git/diff_processor.rs`) is responsible for transforming a raw Git diff into a "compressed" version that fits within LLM context limits while retaining maximum semantic value.

## Processing Pipeline

### Stage 1: Sanitization
- Remove `` (carriage returns).
- Trim trailing whitespace.
- Normalize line endings to `
`.
- Collapse multiple empty lines into a single one.

### Stage 2: Tokenization
- Uses the `tiktoken-rs` library with the `cl100k_base` encoding (standard for GPT-3.5/4/4o).
- Calculates the token count for each file in the diff.

### Stage 3: Chunking Strategy
If the total token count exceeds the `max_input_tokens`:
1. **File Filter**: It attempts to include as many whole files as possible, sorted by importance (heuristic: modified > added > deleted).
2. **Hunk Truncation**: If a single file is too large, it truncates the diff, keeping the start of the diff (where changes are usually most significant in Git's output).

## Configuration
- `max_input_tokens`: Set per profile (defaults to 128k for modern models).
- `reserved_tokens`: Space kept for system prompts and user instructions.

## Design Decisions
- **Immutable Input**: The processor takes a `&str` or `&Diff` and returns an owned `ProcessedDiff`.
- **Zero-Allocation (where possible)**: Uses efficient string slices during the sanitization phase to minimize memory pressure.
