<div align="center">
<img src=".github/assets/avatar.png" align="center" width="120px" height="120px" />
<h3>Christina</h3>
<p>Terminal commit assistant for staged Git changes.</p>

<a href="https://github.com/yehezkieldio/christina"><img src="https://img.shields.io/badge/rust-2024-C96329?style=flat&labelColor=1C2C2E&logo=Rust&logoColor=white"></a>
<a href="LICENSE-MIT"><img src="https://img.shields.io/badge/license-MIT-C96329?style=flat&labelColor=1C2C2E"></a>
<a href="LICENSE-APACHE"><img src="https://img.shields.io/badge/license-Apache--2.0-C96329?style=flat&labelColor=1C2C2E"></a>
</div>

---

Christina reads the staged diff in the current Git repository, asks an Azure OpenAI
chat model for a Conventional Commit message, and either creates the commit or lets
you inspect the proposed message first. It is built for ordinary command-line use:
stage changes, run `christina`, review the message, commit.

## Features

- **Staged-Diff Workflow**: Only staged changes are considered. If nothing is staged, Christina exits before contacting the model.
- **Conventional Commit Validation**: Generated messages are checked against configurable validation modes (`strict`, `soft`, or `disabled`) and a configurable subject length.
- **Large-Diff Chunking**: Oversized diffs are budgeted with local `tiktoken` counting, summarized concurrently, and reduced into one final message.
- **Commit History Context**: Recent commit subjects can be included so the generated message follows the local repository style.
- **Interactive Confirmation**: Accept, edit, regenerate, decline, or run with `--yes` for non-interactive use.
- **Secure Profile Storage**: Profiles support literal values, environment references (`env:NAME`), and keyring references (`keyring:NAME`) when the `keyring-support` feature is enabled.
- **Diagnostic Trace Mode**: `--trace` prints generation stages and summary stats for debugging token budgets, provider behavior, and commit creation.

## Pipeline

Each run follows a fixed sequence:

1. **Read Git state**: Open the current repository, verify staged changes, and build the staged diff.
2. **Load configuration**: Merge global config, safe local overrides from `./christina.toml`, active profile values, and `CHRISTINA_*` environment overrides.
3. **Prepare context**: Collect staged file names, optional user context, and recent commit history if enabled.
4. **Generate**: Send the diff directly for small inputs or split large diffs into chunks, summarize them concurrently, and synthesize a final message.
5. **Review**: Show the generated message and let the operator accept, edit, regenerate, or cancel.
6. **Commit**: Create the Git commit unless `--dry-run` was supplied.

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

1. Global config: `~/.config/christina/config.toml` on Linux and macOS, or the platform equivalent.
2. Local repository override: `./christina.toml` for safe repository-local settings.
3. Active profile values.
4. Environment variables.

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

A complete example lives in [`config.example.toml`](config.example.toml).

## Christina-specific

Christina only reads staged changes. It does not inspect unstaged work, and it
will exit before contacting the model when the index is empty.

The current provider surface is Azure OpenAI. The config model and profile CLI are
provider-shaped, but this checkout only has the Azure backend wired in.

Local repository config is intentionally narrower than global config. Use
`./christina.toml` for safe project-level behavior such as ignored files and
validation settings; keep provider credentials in the global config, environment,
or keyring.

Additional notes live under [`docs/`](docs/), including the generation pipeline,
design notes, benchmarks, advanced configuration, and contribution notes.

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
