# Christina Architecture

This document describes the high-level architecture of Christina and the design principles guiding its development.

## Design Philosophy

Christina is built with the following principles (as defined in [AGENTS.md](AGENTS.md)):

- **Data-Oriented Design**: Focus on data flow and state transitions rather than object-oriented hierarchies.
- **Explicit Ownership**: Leveraging Rust's ownership model for safety and performance without over-reliance on interior mutability.
- **"Correct by Construction"**: Using the type system to enforce invariants (e.g., specific types for commit messages, file paths).
- **Performance First**: Minimal allocations, high-performance token counting, and efficient diff processing.

## Codebase Structure

The project is organized as a Rust workspace with two primary crates:

### 1. `christina-core`

The logic heart of the application. It is intentionally headless and could be used by other frontends (e.g., a GUI or a web service).

- **`git/`**: Wraps `git2-rs` to provide high-level operations like staging checks, diff generation, and commit execution.
- **`llm/`**: A provider-agnostic interface for AI services. Currently supports Azure OpenAI.
- **`types/`**: Core domain models that ensure data validity across the system.
- **`config/`**: Handles configuration resolution, profile management, and secure secret storage using system keyrings.
- **`tokenizer.rs`**: High-performance token counting powered by `tiktoken` to ensure diffs fit within model context windows.

### 2. `christina` (CLI)

The user-facing CLI application.

- **`cli.rs`**: Argument parsing using `clap`.
- **`generate.rs`**: The main orchestration logic that ties Git changes, LLM generation, and user confirmation together.
- **`ui/`**: (Coming soon) Components for terminal interaction, spinners, and progress reporting.
- **`events.rs`**: Internal event system for tracking long-running tasks like AI generation.

## Data Flow

1. **Analysis**: `christina` asks `christina-core` to inspect the current Git repository for staged changes.
2. **Context Compression**: The diff is processed to ensure it fits within the selected LLM's context window.
3. **Generation**: `christina` sends the compressed diff (and optional user context) to `christina-core::llm`, which communicates with the configured AI provider.
4. **Validation**: The generated message is parsed and validated against Conventional Commit standards.
5. **Execution**: Upon user confirmation, `christina` calls `christina-core::git` to perform the actual commit.

## Profile System

Christina supports multiple "Profiles". A profile defines:
- The AI Provider (Azure, etc.)
- The Model (e.g., `gpt-4o`)
- Configuration parameters (Temperature, Max Tokens)
- Secrets (API Keys, stored securely)

Profiles are stored in `~/.config/christina/profiles.json` (or OS equivalent), with sensitive data moved to the system keyring.

## Error Handling

- **Domain Errors**: Defined in `christina-core/src/error.rs` using `thiserror`.
- **CLI Errors**: Handled in `christina` using `anyhow` for flexible reporting.
- **Panics**: Used strictly for invariant violations and unrecoverable programming errors.

## Prompt Safety

- User-provided context is treated as untrusted data and wrapped in explicit delimiters.
- Context length is capped to 500 bytes to prevent prompt overflows and injection attempts.

## Performance Considerations

- **Memory**: Uses `mimalloc` for better allocation performance in multi-threaded environments.
- **Concurrency**: LLM requests are handled asynchronously via `tokio`.
- **Tokenization**: Efficiently handles large diffs by pre-calculating tokens before sending requests.

## Detailed Documentation

For more in-depth information, please refer to the following resources:

### Architecture Decision Records (ADRs)
- [ADR 001: Crate Split](docs/adrs/001-crate-split.md)
- [ADR 002: Diff Chunking Strategy](docs/adrs/002-diff-chunking-strategy.md)
- [ADR 003: Provider-Agnostic Interface](docs/adrs/003-provider-agnostic-interface.md)

### Component Specifications
- [AI Orchestrator](docs/specs/ai-orchestrator.md)
- [Diff Processor](docs/specs/diff-processor.md)

### Developer Guides
- [Rust Design Patterns](docs/guides/design-patterns.md)
- [Provider Implementation Guide](docs/guides/provider-implementation.md)
- [Advanced Configuration Guide](docs/guides/advanced-config.md)
