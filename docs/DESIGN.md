# Design Philosophy

This document outlines the technical philosophy and architectural decisions behind Christina.

## Data-Oriented Design

Christina is designed around the transformation of data, not the encapsulation of objects. We avoid deep class hierarchies and complex trait indirection in favor of linear pipelines.

- **Explicit State**: System state is represented by Enums (e.g., `PipelineState`) that clearly define the progression of work.
- **Batch Processing**: Instead of processing file-by-file in a tightly coupled loop, we use a Map-Reduce flow that treats chunks as independent units of work.
- **Composition over Inheritance**: We build complex behaviors by composing small, specialized modules rather than extending base classes.

## Correct by Construction

We leverage Rust's type system to eliminate entire classes of bugs.

- **Validated Newtypes**: A `CommitMessage` cannot be instantiated without passing through a validation boundary. Once a component receives a `CommitMessage` type, it can assume the content is non-empty, single-line, and follows Conventional Commit standards.
- **Invariant Enforcement**: Types like `TokenCount` (using `NonZeroU32`) and `RepoRoot` (requiring absolute paths) ensure that core logic never has to perform defensive null-checks or path resolution.
- **Ownership moves forward**: APIs are designed to consume inputs and produce new outputs, following a clear lifecycle from construction to consumption.

## Map-Reduce for Large-Scale Diffs

The decision to use a Map-Reduce architecture was driven by the inherent limitations of LLM context windows and the need for semantic consistency.

- **The Problem**: A single massive diff (e.g., a 500-file refactor) loses context when truncated and confuses LLMs when processed sequentially.
- **The Solution**: 
  - **Map**: Partition the diff into coherent chunks. Summarize each chunk in parallel to extract atomic "facts".
  - **Reduce**: Synthesize these facts into high-level intent.
- **Benefit**: This approach scales linearly with the number of files and produces high-quality messages even when changes span unrelated modules.

## Zero-Trust Input

Security is a design-time property. Christina assumes all external data is potentially malicious.

- **Diff Parsing**: We use a strict header-first parser for Git diffs. This prevents "injection" attacks where diff content might attempt to mimic a header to redirect the AI's attention.
- **Context Delimiters**: User-provided hints are wrapped in strong, unique delimiters and the system prompt includes explicit instructions to treat this content as untrusted data.

## Cross-References

- [Architecture](ARCHITECTURE.md): Structural overview of the workspace.
- [Generation Pipeline](GENERATION_PIPELINE.md): Detailed data transformation steps.
