<div align="center">
<img src=".github/assets/avatar.png" align="center" width="120px" height="120px" />
<h3>Christina</h3>
<p>Terminal commit assistant for staged Git changes.</p>

<a href="https://github.com/yehezkieldio/christina"><img src="https://img.shields.io/badge/rust-2024-C96329?style=flat&labelColor=1C2C2E&logo=Rust&logoColor=white"></a>
<a href="LICENSE-MIT"><img src="https://img.shields.io/badge/license-MIT-C96329?style=flat&labelColor=1C2C2E"></a>
<a href="LICENSE-APACHE"><img src="https://img.shields.io/badge/license-Apache--2.0-C96329?style=flat&labelColor=1C2C2E"></a>
</div>

---

Christina is a terminal commit assistant for Git repositories. Stage your changes,
run `christina`, and it turns the staged diff into a Conventional Commit message
using your configured Azure OpenAI deployment. The generated message is shown for
review before Git writes anything, with inline editing, regeneration, dry-run, and
non-interactive modes available for different workflows.

Large commits are handled through the same flow: Christina budgets the diff
locally, chunks oversized inputs when needed, includes recent commit history as
style context, and reduces chunk summaries into one final message. It sends data
directly to the configured provider; there is no Christina-hosted service in the
middle.

## Features

- **Staged-Diff Workflow**: Only staged changes are considered, so the proposed message describes exactly what Git is about to commit. If the index is empty, Christina exits before contacting the model.
- **Conventional Commit Validation**: Generated messages are cleaned and checked against configurable validation modes (`strict`, `soft`, or `disabled`). Subject length can be bounded so the result stays compatible with conventional tooling and local commit style.
- **Large-Diff Chunking**: Oversized diffs are budgeted with local `tiktoken` counting, split into manageable chunks, summarized concurrently, and reduced into one final message. Lockfiles can be capped separately so dependency churn does not consume the whole prompt.
- **Commit History Context**: Recent commit subjects can be included as style context for the generator. History is trimmed against the token budget, which keeps it useful without crowding out the staged diff.
- **Interactive Confirmation**: The generated message is shown before commit creation, with options to accept, edit inline, regenerate, or decline. `--yes` keeps the same pipeline available for non-interactive use.
- **Secure Profile Storage**: Provider profiles support literal values and environment references (`env:NAME`). Secrets are resolved at runtime instead of being printed through normal config output.
- **Diagnostic Trace Mode**: `--trace` prints generation stages, budget warnings, repository stats, and commit outcome details. It is meant for understanding slow generations, provider failures, and unexpected prompt-budget behavior.

## Pipeline

Each `christina` invocation runs through a fixed sequence of stages:

1. **Read**: Christina opens the current Git repository, verifies that the index
   contains staged changes, and builds a staged diff. Unstaged files are ignored
   completely, so the generated message matches exactly what Git is about to
   commit.
2. **Configure**: Runtime settings are loaded from the global config, safe local
   overrides, the active profile, and environment variables. Provider credentials
   are resolved from literal values or environment references before any model
   request is made.
3. **Contextualize**: Staged file names, optional `--context` text, and recent
   commit subjects are prepared as prompt context. User context and history are
   trimmed against the token budget instead of being allowed to crowd out the
   staged diff.
4. **Analyze**: Small diffs are sent through the direct prompt path. Large diffs
   are split into budgeted chunks, summarized concurrently, and reduced into a
   final intent summary before message generation.
5. **Validate**: The generated message is cleaned, checked against the configured
   Conventional Commit validation mode, and constrained by the configured subject
   length. Trace mode reports budget warnings and generation stages when the
   pipeline needs inspection.
6. **Commit**: The proposed message is shown for confirmation. The operator can
   accept, edit inline, regenerate, decline, or use `--dry-run` to stop before
   Git writes the commit.

## Building from Source

Christina currently ships from source.

### Prerequisites

- [Rust](https://rustup.rs/) stable toolchain with edition 2024 support
- [`just`](https://github.com/casey/just) for development commands
- An Azure OpenAI chat deployment

### Build

```sh
git clone https://github.com/yehezkieldio/christina.git
cd christina
cargo build --release -p christina
```

The binary is placed at `./target/release/christina`.

To install from this checkout:

```sh
cargo install --path christina
```

## Quick Start

### 1. Configure a provider profile

Christina currently supports Azure OpenAI.

```sh
christina profile create azure \
  --provider azure \
  --model gpt-4o \
  --api-key env:AZURE_OPENAI_API_KEY \
  --api-url https://your-resource.openai.azure.com/openai/deployments/gpt-4o/chat/completions \
  --azure-api-version 2024-12-01-preview \
  --azure-deployment-id gpt-4o

christina profile switch azure
```

Then provide the secret through the referenced environment variable:

```sh
export AZURE_OPENAI_API_KEY=your-api-key
```

You can see the global config path with:

```sh
christina config path
```

### 2. Stage changes

```sh
git add path/to/file.rs
```

### 3. Generate a commit

```sh
christina
```

Useful variants:

```sh
christina --dry-run
christina --yes
christina --context "rename the parser state machine"
christina --trace
```

## Configuration

Configuration is loaded in ascending priority:

1. User config: `~/.config/christina/config.toml` on Linux and macOS, or the platform equivalent.
2. Active profile values.
3. Environment variables.

Selected environment overrides:

| Variable | Meaning |
| --- | --- |
| `CHRISTINA_MODEL_PROVIDER` | Provider kind. Currently `azure`. |
| `CHRISTINA_MODEL` | Model name sent to the provider. |
| `CHRISTINA_MODEL_API_KEY` | API key value. |
| `CHRISTINA_MODEL_API_URL` | Azure chat completions URL. |
| `CHRISTINA_AZURE_API_VERSION` | Azure API version. |
| `CHRISTINA_AZURE_DEPLOYMENT_ID` | Azure deployment id. |
| `CHRISTINA_TOKENS_MAX_INPUT` | Maximum input token budget. |
| `CHRISTINA_TOKENS_OUTPUT` | Maximum output token budget. |
| `CHRISTINA_MODEL_TEMPERATURE` | Sampling temperature. |
| `CHRISTINA_REASONING_EFFORT` | Optional provider reasoning effort. |
| `CHRISTINA_USE_COMMIT_HISTORY` | Enable or disable recent commit history context. |
| `CHRISTINA_COMMIT_HISTORY_DEPTH` | Number of recent commits to inspect, clamped to 0-50. |
| `CHRISTINA_CONCURRENCY_LIMIT` | Concurrent map requests, clamped to 1-20. |
| `CHRISTINA_MAX_FAILURE_RATE` | Allowed partial map failure rate before aborting. |

## Commit Workflow

Christina only reads staged changes. It does not inspect unstaged work, and it
will exit before contacting the model when the index is empty.

The current provider surface is Azure OpenAI. The config model and profile CLI are
provider-shaped, but this checkout only has the Azure backend wired in.

Christina does not read repository-local config. Keep persistent settings in the
user config path printed by `christina config path`, and use environment
variables for invocation-specific overrides.

## Development

The workspace contains:

- `christina`: CLI, Git integration, configuration loading, Azure provider calls, orchestration, and terminal UI.
- `christina-core`: provider-agnostic domain types, config file shapes, tokenizer utilities, prompt construction, chunking, and tests.
- `ui-extractable`: small terminal UI extraction experiment used by the workspace.

Common commands:

```sh
just check
just clippy
just test
just fmt
```

`just clippy` runs with `-D warnings`. `just test` expects `cargo nextest` to be installed.

## License

Dual-licensed under [MIT](LICENSE-MIT) and [Apache 2.0](LICENSE-APACHE).
