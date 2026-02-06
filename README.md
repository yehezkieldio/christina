test change

<div align="center">
<img src=".github/assets/avatar.png" align="center" width="120px" height="120px" />
<h3>Christina</h3>
<p>Automated Conventional Commit Generator Powered By LLMs</p>
</div>

---

Christina is a personal Terminal User Interface (TUI) tool designed to streamline version control workflows. It analyzes your staged Git changes and uses AI to automatically generate meaningful, conventional commit messages, saving you time and ensuring consistency across your project history, *most of the time.*

## Features

- **Automated Conventional Commits**: Analyzes staged changes and generates high-quality commit messages conforming to [Conventional Commits](https://www.conventionalcommits.org/).
- **Multi-Provider Support**: Seamlessly switch between OpenAI, Azure, and Groq backends.
- **Smart Diff Processing**: Only sends relevant parts of your diff to the LLM, managing token limits efficiently.
- **Context Injection**: Provide optional hints to the AI to guide message generation.
- **Dry Run Mode**: Preview generated messages without committing.
- **High-Performance CLI**: Built in Rust with focus on speed and minimal resource usage.
- **Profile System**: Configure and switch between multiple LLM profiles for different projects or environments.
- **Integrated Logging**: Rolling diagnostic logs for easy troubleshooting.

## Getting Started

### Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) (latest stable)
- [Just](https://github.com/casey/just) (optional, but recommended for build shortcuts)

### Installation

```bash
# Clone the repository
git clone https://github.com/yehezkieldio/christina-vibe.git
cd christina-vibe

# Build and install
cargo install --path christina
```

### Usage

1. **Configure your first profile:**
   ```bash
   # Add a new profile (OpenAI example)
   # Plaintext keys are accepted by default (you'll get a warning)
   christina profile create my-openai --provider openai --model gpt-4o --api-key YOUR_KEY
   # Environment or keyring references are recommended for security
   christina profile create my-openai --provider openai --model gpt-4o --api-key env:OPENAI_API_KEY
   christina profile create my-openai --provider openai --model gpt-4o --api-key keyring:christina.openai
   ```

2. **Generate a commit message:**
   ```bash
   # Stage some changes
   git add .

   # Run christina
   christina
   ```

3. **Options:**
   ```bash
   # Dry run (don't actually commit)
   christina --dry-run

   # Provide context
   christina --context "Fixed a critical memory leak in the tokenizer"
   ```

## Configuration

Christina stores configuration and profiles in your OS-standard config directory (e.g., `~/.config/christina/` on Linux).
See `config.example.toml` for the full reference and `docs/guides/advanced-config.md` for deep usage patterns.

Quick tips:
- Global config: `~/.config/christina/config.toml` (Linux/macOS) or `%APPDATA%\\christina\\config.toml` (Windows)
- Local overrides: `./christina.toml` (safe fields only, per-repo)
- CLI helpers: `christina config list|get|set|path`

### Example `config.toml`

```toml
schema_version = 2

[standard]
active_profile = "default"
commit_message_max_length = 72
commit_message_validation_mode = "soft"
ignore_files = []

[advanced]
lockfile_token_limit = 100
use_commit_history = true
commit_history_depth = 5
max_concurrent_requests = 4
max_partial_failure_rate = 0.10
prompt_failure_rate_threshold = 0.05

[experimental]
use_experimental = false
usage_tier = "standard"

[experimental.free_tier]
max_input_tokens = 16000
max_output_tokens = 512
max_concurrent_requests = 1
commit_history_depth = 3

[profiles.default]
name = "default"
provider = "openai"
model = "gpt-4.1-mini"
api_key = { value = "YOUR_KEY" }
max_input_tokens = 128000
max_output_tokens = 4096
temperature = 0.3
```

### Profile Management

Profiles allow you to manage multiple LLM configurations.

```bash
# List all profiles
christina profile list

# Create a profile (OpenAI example)
christina profile create my-openai --provider openai --model gpt-4.1-mini --api-key env:OPENAI_API_KEY

# Switch to a different profile
christina profile switch my-groq-profile

# View detailed config for a profile
christina profile show my-openai
```

### API Keys
You can provide API keys in three ways:
- Plaintext (default): `api_key = { value = "YOUR_KEY" }`
- Environment variable: `api_key = { env = "OPENAI_API_KEY" }`
- Keyring reference: `api_key = { keyring = "christina.openai" }`

Plaintext is accepted by default (with a warning). For security, prefer env or keyring.

## Architecture

Christina follows a modular design split into two main crates:
- `christina-core`: Headless logic, Git integration, LLM orchestrator, and domain models.
- `christina`: CLI interface, orchestration, and user interaction layer.

For more details, see [ARCHITECTURE.md](ARCHITECTURE.md).

## TUI Status

The TUI mode is currently **temporarily removed** to focus on core stability. See [TUI_INTEGRATION.md](TUI_INTEGRATION.md) for the roadmap and status.

## License

Licensed under either the MIT License or the Apache License 2.0, at your option

For detailed license information, please refer to the [LICENSE-MIT](LICENSE-MIT) and [LICENSE-APACHE](LICENSE-APACHE) files.
