# ADR 002: Diff Chunking and Context Compression

## Status
Accepted

## Context
Git diffs can be extremely large, often exceeding the context window of LLMs (especially smaller models or when history is included). Sending raw diffs is also expensive and often contains "noise" (like imports or boilerplate changes) that doesn't contribute much to a commit message's quality.

## Decision
We implemented a multi-stage "Context Compression" strategy in the `DiffProcessor`:
1. **Cleaning**: Remove noise such as carriage returns, trailing whitespace, and excessive empty lines.
2. **Chunking**: If a diff exceeds the token budget, it is split into chunks. We prioritize:
    - File-based chunking first.
    - If a single file's diff is too large, we perform line-based chunking within that file.
3. **Budgeting**: We use a strict token budget calculated based on the model's limits, reserved space for the system prompt, and user context.

## Consequences
- **Pros**:
    - Reliability: Prevents "Context Window Exceeded" errors from LLM providers.
    - Cost Efficiency: Reduces the number of tokens sent, lowering API costs.
    - Speed: Smaller payloads result in faster LLM responses.
- **Cons**:
    - Information Loss: In extreme cases, crucial context might be truncated.
    - Complexity: The `DiffProcessor` logic is complex and must be highly performant (using `tiktoken`).
