# Advanced Configuration Guide

This guide covers advanced configuration scenarios for Christina.

## Profile Management
Christina uses a profile-based system. Profiles allow you to quickly switch between different models and providers.

### Switching Profiles
```bash
christina profile switch production-gpt4
```

### Environment Overrides
You can override any configuration value using environment variables with the `CHRISTINA_` prefix:
- `CHRISTINA_ACTIVE_PROFILE=groq`
- `CHRISTINA_TEMPERATURE=0.7`

## API Keys
Plaintext API keys are accepted by default (with a warning). For security, prefer environment or keyring references.

### Plaintext (default)
```toml
api_key = { value = "YOUR_KEY" }
```

### The Keyring
You can store keys in your OS's secure keyring:
- **Linux**: Secret Service (libsecret) or KWallet.
- **macOS**: Keychain.
- **Windows**: Credential Manager.

```toml
api_key = { keyring = "christina.openai" }
```

### Environment Variable References
You can also reference environment variables in your config:
```toml
api_key = { env = "MY_SECRET_KEY" }
```

### CLI Examples
```bash
# Plaintext (default, warning emitted)
christina profile create my-openai --provider openai --model gpt-4o --api-key YOUR_KEY

# Environment variable
christina profile create my-openai --provider openai --model gpt-4o --api-key env:OPENAI_API_KEY

# Keyring reference
christina profile create my-openai --provider openai --model gpt-4o --api-key keyring:christina.openai
```

## Customizing Token Budgets
If you are using a model with a very large context window (e.g., Claude 3.5 Sonnet with 200k tokens), you can increase the budget in your profile:

```bash
christina profile update my-profile --max-input-tokens 200000
```

## Log Management
Christina maintains rolling logs for diagnostics.
- **Location**: `~/.cache/christina/logs/` (or OS equivalent).
- **Levels**: Controlled via `RUST_LOG=debug christina`.

## Custom Commit Formats
While Christina defaults to Conventional Commits, you can influence the style using the `--context` flag or by setting a `system_prompt_override` (experimental) in the profile config.
