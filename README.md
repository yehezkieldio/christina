<div align="center">
<img src=".github/assets/avatar.png" align="center" width="120px" height="120px" />
<h3>Christina</h3>
<p>Terminal interface for AI-powered conventional commit generation.</p>

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![License: Apache 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)
![Rust](https://img.shields.io/badge/rust-stable-brightgreen.svg)
</div>

---

Christina is a high-performance terminal utility designed to automate the generation of [Conventional Commits](https://www.conventionalcommits.org/). It employs advanced Large Language Models (LLMs) to analyze staged changes, utilizing a specialized Map-Reduce architecture to handle large-scale diffs while ensuring semantic consistency and adherence to version control standards.

## Technical Overview

Christina is engineered for speed, privacy, and precision. It leverages Rust's asynchronous runtime (`tokio`) for concurrent processing and the `tiktoken` library for efficient local token counting. Unlike simpler alternatives, it implements an intelligent diff analysis pipeline that maintains architectural intent across massive commits.

### Key Capabilities

*   **Recursive Diff Chunking**: Automatically fragments large diffs into semantically coherent units (File > Hunk > Line) to respect LLM context windows without losing change locality.
*   **Map-Reduce Orchestration**: Parallelizes chunk summarization before synthesizing a final intent-based commit message, supporting repositories with hundreds of modified files.
*   **Architectural Intent Extraction**: Groups atomic changes into high-level themes to generate descriptions that reflect "why" and "what" rather than just listing file names.
*   **Zero-Trust Security**: Native integration with OS Keyrings (via `keyring`) and environment variables ensures API keys are never stored in plaintext or leaked into telemetry.
*   **Profile System**: Supports multiple provider configurations (Azure OpenAI, OpenAI) with fine-grained control over model parameters and token budgets.
*   **Interactive TUI**: Provides a streamlined interface for message validation, regenerative refinement, and inline Emacs-style editing.

## Installation

### From Source

Requires the latest stable Rust toolchain.

```bash
git clone https://github.com/yehezkieldio/christina-vibe.git
cd christina-vibe
cargo install --path christina
```

### Development

The project uses `just` for workflow automation:

```bash
just all     # Execute formatting, linting, and tests
just test    # Run full test suite using nextest
```

## Configuration

Christina employs a layered configuration model with the following precedence (highest to lowest):

1.  **Environment Variables**: Prefixed with `CHRISTINA_` (e.g., `CHRISTINA_MODEL`).
2.  **Local Overrides**: `./christina.toml` located in the repository root (safe fields only).
3.  **Global Configuration**: `~/.config/christina/config.toml` (Linux/macOS) or `%APPDATA%\christina\config.toml` (Windows).

### Core Settings

| Key | Type | Default | Description |
|:--- |:--- |:--- |:--- |
| `active_profile` | String | `"default"` | The name of the LLM profile to use. |
| `commit_message_max_length` | Integer | `72` | Hard limit for the commit subject line. |
| `commit_message_validation_mode` | Enum | `"soft"` | `strict`, `soft`, or `disabled`. |
| `ignore_files` | List | `[]` | Glob patterns to exclude from analysis. |
| `max_concurrent_requests` | Integer | `4` | Concurrency limit for map-phase summarization. |
| `lockfile_token_limit` | Integer | `100` | Token cap for dependency lockfiles to preserve budget. |

### Profile Configuration

Profiles are defined in the `[profiles]` table.

```toml
[profiles.production]
provider = "azure"
model = "gpt-4o"
api_key = { keyring = "christina.azure.prod" }
api_url = "https://your-resource.openai.azure.com/"
max_input_tokens = 128000
max_output_tokens = 4096
temperature = 0.3
```

## Pipeline Architecture

Christina's processing pipeline is divided into two distinct crates to separate domain logic from interface concerns.

### christina-core

The headless engine responsible for:
*   **Token Management**: Local BPE tokenization and budget allocation.
*   **Chunking Logic**: Recursive algorithms for diff partitioning.
*   **Prompt Engineering**: Few-shot templates and anti-slop verbiage enforcement.
*   **Domain Models**: Immutable types for Git snapshots and commit messages.

### christina

The orchestrator and user interface responsible for:
*   **Concurrency**: Managing parallel LLM requests with rate limiting and exponential backoff.
*   **Git Integration**: Interfacing with `git2` for diff extraction and commit authoring.
*   **Secret Resolution**: Dynamic loading of credentials from secure stores.
*   **Telemetry**: Real-time progress events and diagnostic tracing.

## Advanced Usage

### Context Injection

Augment the AI's understanding by providing high-level context:
```bash
christina --context "Refactored the persistence layer to support S3"
```

### Diagnostic Tracing

Enable deep pipeline telemetry to analyze token usage, chunking efficiency, and provider latency:
```bash
christina --trace
```

### Dry Run

Preview the generated commit message without performing filesystem modifications:
```bash
christina --dry-run
```

## Security and Privacy

*   **Data Locality**: Changes are sent directly to your configured provider. No intermediate servers are involved.
*   **Redaction**: API keys and sensitive data are handled via `SecretString` wrappers that redact content in all debug and log outputs.
*   **Injection Prevention**: Diff headers are strictly parsed to prevent malicious content from overriding system prompt instructions.

## License

Dual-licensed under [MIT](LICENSE-MIT) and [Apache 2.0](LICENSE-APACHE).
