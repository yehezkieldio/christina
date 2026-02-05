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
   christina profile add my-openai --provider openai --model gpt-4o --api-key YOUR_KEY
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

### Example `config.toml`

```toml
[diff]
# Choose your preferred diff tool: git, delta, difftastic, or basic
diff_tool = "git"
# Always show a preview of changes before generating message
diff_show_preview = true

[generation]
# Default temperature for LLM sampling
temperature = 0.7
# Maximum tokens for generated message
max_tokens = 500
```

### Profile Management

Profiles allow you to manage multiple LLM configurations.

```bash
# List all profiles
christina profile list

# Switch to a different profile
christina profile switch my-groq-profile

# View detailed config for a profile
christina profile show my-openai
```

Keys are securely stored using your system's native keyring (via the `keyring` crate).

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
