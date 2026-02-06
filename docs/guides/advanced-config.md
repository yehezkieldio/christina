# Advanced Configuration Guide

This guide covers configuration, profiles, and advanced operational tuning for Christina.

## Configuration Files

Christina loads configuration in this order (highest to lowest priority):
- Environment variables (`CHRISTINA_*`)
- Local config: `./christina.toml` (safe fields only)
- Global config: `~/.config/christina/config.toml` (Linux/macOS) or `%APPDATA%\\christina\\config.toml` (Windows)
- Built-in defaults

The full reference file lives at `config.example.toml`.

## Global Config Structure

Global config is grouped into sections:
- `[standard]` for day-to-day settings
- `[advanced]` for performance and budget controls
- `[experimental]` for opt-in features
- `[profiles.*]` for provider profiles

Example:
```toml
schema_version = 2

[standard]
active_profile = "default"
commit_message_validation_mode = "soft"
ignore_files = ["Cargo.lock", "vendor/"]

[advanced]
lockfile_token_limit = 100
use_commit_history = true
commit_history_depth = 5
max_concurrent_requests = 4
max_partial_failure_rate = 0.10
prompt_failure_rate_threshold = 0.05

[profiles.default]
name = "default"
provider = "openai"
model = "gpt-4.1-mini"
api_key = { env = "OPENAI_API_KEY" }
max_input_tokens = 128000
max_output_tokens = 4096
temperature = 0.3
```

## Local Config Overrides (`./christina.toml`)

Local config is intentionally limited to safe fields so a repo cannot override secrets or providers. Allowed keys:
- `ignore_files`
- `lockfile_token_limit`
- `commit_message_max_length`
- `commit_message_validation_mode`
- `use_commit_history`
- `commit_history_depth`

Example:
```toml
ignore_files = ["Cargo.lock", "*.lock", "vendor/"]
lockfile_token_limit = 50
commit_message_validation_mode = "strict"
commit_history_depth = 3
```

## Environment Variable Overrides

Environment variables override everything, including profiles. Supported keys:

| Variable | Description |
| --- | --- |
| `CHRISTINA_TOKENS_MAX_INPUT` | Override max input tokens |
| `CHRISTINA_TOKENS_OUTPUT` | Override max output tokens |
| `CHRISTINA_MODEL_PROVIDER` | Provider override (`openai`, `azure`, `groq`) |
| `CHRISTINA_MODEL` | Model name override |
| `CHRISTINA_MODEL_API_KEY` | Inline API key override |
| `CHRISTINA_MODEL_API_URL` | Base URL override |
| `CHRISTINA_AZURE_API_VERSION` | Azure API version override |
| `CHRISTINA_AZURE_DEPLOYMENT_ID` | Azure deployment ID override |
| `CHRISTINA_MODEL_TEMPERATURE` | Temperature override |
| `CHRISTINA_USE_COMMIT_HISTORY` | Override commit history usage |
| `CHRISTINA_COMMIT_HISTORY_DEPTH` | Override history depth |
| `CHRISTINA_CONCURRENCY_LIMIT` | Override max concurrent requests |
| `CHRISTINA_MAX_FAILURE_RATE` | Override max partial failure rate |

## Config CLI

Use `christina config` for quick inspection and edits:
```bash
christina config list
christina config get max_input_tokens
christina config set commit_message_validation_mode strict
christina config path
```

Supported `config set` keys:
- `max_input_tokens`
- `max_output_tokens`
- `model_provider`
- `model`
- `api_key`
- `model_api_url`
- `azure_api_version`
- `azure_deployment_id`
- `model_temperature`
- `ignore_files`
- `lockfile_token_limit`
- `usage_tier`
- `use_experimental`
- `free_tier_max_input_tokens`
- `free_tier_max_output_tokens`
- `free_tier_max_concurrent_requests`
- `free_tier_commit_history_depth`
- `commit_message_max_length`
- `commit_message_validation_mode`
- `use_commit_history`
- `commit_history_depth`
- `max_concurrent_requests`
- `max_partial_failure_rate`
- `prompt_failure_rate_threshold`

Notes:
- Token values are clamped to hard limits.
- `max_partial_failure_rate` is clamped to the recommended range `0.01` to `0.50`.
- `config set` updates the active profile for provider-specific fields.

## Profiles

Profiles define how Christina talks to an LLM. Only these providers are supported:
- `openai`
- `azure`
- `groq`

### Active Profile

Christina chooses the active profile in this order:
- `CHRISTINA_MODEL_*` env overrides (highest priority)
- `standard.active_profile` in `config.toml`
- `profiles.active` in `config.toml` (legacy; still supported)

### Profile CLI

```bash
christina profile list
christina profile show default
christina profile create my-openai --provider openai --model gpt-4.1-mini --api-key env:OPENAI_API_KEY
christina profile edit my-openai --model gpt-4.1-mini --max-output-tokens 1024
christina profile duplicate my-openai my-openai-fast
christina profile switch my-openai
christina profile delete my-openai
```

### API Key Formats

You can provide API keys in three ways:
- Plaintext: `YOUR_KEY`
- Environment reference: `env:OPENAI_API_KEY`
- Keyring reference: `keyring:christina.openai`

Plaintext keys require `--allow-plaintext` when creating or editing profiles.

## Token Budgets and Limits

Important knobs and what they do:
- `max_input_tokens` and `max_output_tokens` control the context budget.
- `lockfile_token_limit` caps how much of large lockfiles is included.
- `use_commit_history` and `commit_history_depth` control style context.
- `max_concurrent_requests` affects throughput during map-reduce generation.
- `max_partial_failure_rate` controls tolerance for chunk failures.

## Experimental Usage Tier

Free-tier limits only apply when both of these are true:
- `experimental.use_experimental = true`
- `experimental.usage_tier = "free"`

## Diagnostics

Trace and logging options:
- `RUST_LOG=debug christina` for verbose file-based logging.
- `christina --trace` for full pipeline tracing to stderr with telemetry summary.
