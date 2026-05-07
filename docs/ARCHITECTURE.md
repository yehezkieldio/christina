# Christina Architecture

Christina is engineered as a high-throughput, semantically aware commit generation engine. This document details the system decomposition and the Map-Reduce pipeline used to manage large-scale Git diffs.

## System Decomposition

The project is structured as a Rust workspace to maintain a strict separation between core domain logic and the user-facing interface.

### 1. `christina-core` (The Engine)
A headless, zero-IO library that defines the stable domain model and the atomic processing primitives.
- **Processing Pipeline**: Implementation of recursive diff chunking and BPE tokenization.
- **Domain Models**: Validated newtypes for `CommitMessage`, `FilePath`, and `TokenCount` that enforce system invariants.
- **Prompt Templates**: Few-shot LLM templates with strict "anti-slop" verbiage enforcement.
- **Token Management**: Local token counting via `tiktoken` to ensure context window compliance before network egress.

### 2. `christina` (The Orchestrator)
The user-facing CLI and runtime orchestrator.
- **Git Adapter**: Integration with `git2` for staged change extraction and commit authoring.
- **AI Orchestrator**: Multi-threaded Map-Reduce implementation using `tokio`.
- **Secret Provider**: Resolution of credentials from environment variables or OS keyrings.
- **Telemetry**: Diagnostic tracing and real-time progress events for the TUI.

## The Generation Pipeline

Christina employs a Map-Reduce architecture to overcome LLM context window limitations and maintain high semantic coherence across large commits.

1.  **Ingestion**: Staged changes are extracted via `git2` into a `RepoSnapshot`.
2.  **Chunking**: The diff is recursively partitioned into semantically coherent chunks (File > Hunk > Line).
3.  **Map Phase**: Chunks are processed in parallel. Each chunk is summarized by the LLM to extract its primary technical change.
4.  **Intent Extraction**: Atomic summaries are grouped into architectural themes.
5.  **Reduce Phase**: Themes are synthesized into a final single-line Conventional Commit message.
6.  **Validation**: The output is verified against the Conventional Commits specification before presentation.

## Design Principles

- **Data-Oriented Design**: We prioritize data flow over object-oriented hierarchies. State transitions are explicit and linear.
- **Correct by Construction**: We leverage Rust's type system to eliminate invalid states. A `CommitMessage` cannot exist unless it satisfies formatting invariants.
- **Performance as a Design-Time Property**: Performance is not a cleanup phase. We use buffer pooling in chunking and LRU caching in tokenization to minimize overhead.
- **Zero-Trust Input**: All external data (diffs and user context) is treated as untrusted and strictly delimited to prevent prompt injection.

## Cross-References

- [Design Philosophy](DESIGN.md): Deep dive into technical choices.
- [Generation Pipeline](GENERATION_PIPELINE.md): Detailed step-by-step data transformation.
- [Performance Optimization](PERFORMANCE_OPTIMIZATION.md): Documented hot-path optimizations.
- [Technical Specification](SPECIFICATION.md): Wire formats and schema definitions.
