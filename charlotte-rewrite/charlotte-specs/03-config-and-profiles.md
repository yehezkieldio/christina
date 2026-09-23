# Config and profiles

## Config file

Charlotte reads a global TOML config file, the same file format Christina uses today. The file path follows the OS-appropriate config directory (`~/.config/charlotte/config.toml` on Linux). Charlotte reuses this format because it is a good format, not to stay compatible with an existing Christina config file.

Christina's `Config` struct (`christina/src/config/settings.rs`) defines the fields. These cover token limits, model provider, model name, API key, API URL, Azure API version and deployment ID, temperature, and reasoning effort. They also cover ignore file patterns and the lockfile token cap. They also cover commit message length and validation mode, commit history depth, the concurrency limit, and the partial-failure rate threshold. Charlotte keeps this field set and defines it as a Zod schema instead of a `schemars`-derived JSON Schema. The generated JSON Schema file becomes a build output of the Zod schema, not a hand-maintained file.

## Layered loading

Christina resolves config in a fixed order, and Charlotte keeps the same order. The order is: built-in defaults, the global config file, a profile overlay, then an environment variable overlay. A named profile must be active for the profile overlay to apply. Later sources win over earlier ones.

Christina supports two environment variable naming schemes at once: a flat legacy set and a nested `CHRISTINA_*` set (`LEGACY_ENV_BINDINGS` in `settings.rs`). Charlotte drops the legacy naming scheme and keeps a single nested `CHARLOTTE_*` scheme, because Charlotte has no installed base that depends on the old names.

## Profiles

Christina's `ProviderProfile` (`christina-core/src/profile.rs`) names a provider, model, API URL, API key, token limits, and provider-specific fields such as the Azure deployment ID. `Profiles` stores named profile definitions plus which one is active. Charlotte keeps this model and widens `provider` beyond Azure to match the provider list in `06-providers-and-ai-sdk.md`.

## Secrets

Christina's `Secret` and `SecretRef` types (`christina-core/src/config/secret.rs`) let a config value be a literal string or an `env:NAME` reference resolved at runtime. `SecretString` redacts the value in debug output. Charlotte keeps the `env:` syntax unchanged and applies the same redaction rule: a secret value never appears in a printed config, a log line, or a session transcript event. `08-session-storage-and-stats.md` states the same rule for the session log.

## CLI surface

Christina's CLI (`christina/src/cli/mod.rs`) exposes four `config` subcommands: `get`, `set`, `list`, and `path`. It also exposes a `profile` subcommand group with seven actions: `create`, `edit`, `delete`, `switch`, `duplicate`, `show`, and `list`. Charlotte keeps this command surface as its starting point. It is a stable and well-tested shape, not a promise of a familiar interface for a Christina user.

## Open decision

Christina clamps `max_concurrent_requests` and the partial-failure rate between hard bounds (`MIN_PARTIAL_FAILURE_RATE`, `MAX_PARTIAL_FAILURE_RATE`, `MAX_CONFIGURED_CONCURRENT_REQUESTS`). A recent Christina change (see git history, commit `9dcf41c`) stopped clamping max token limits to a hard maximum. Charlotte needs one written policy for which numeric config fields get a hard clamp. It must not decide this field by field during implementation.
