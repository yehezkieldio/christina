# Technical Specification

This document defines the technical standards, data formats, and supported specifications for Christina.

## Conventional Commits Support

Christina generates messages according to the [Conventional Commits v1.0.0](https://www.conventionalcommits.org/) specification.

### Canonical Types
The engine is biased toward the following types:
- `feat`: New user-facing capability.
- `fix`: Bug correction.
- `refactor`: Structural change without functional impact.
- `perf`: Measurable performance improvement.
- `chore`: Tooling, configuration, or dependency maintenance.
- `docs`: Documentation only.
- `test`: Adding or modifying tests.
- `build`: Build system or packaging changes.
- `ci`: CI pipeline configuration.

### Constraints
- **Subject Length**: Maximum 72 characters (default).
- **Format**: `type(scope): description`.
- **Casing**: Subject line is lowercase (except proper nouns).
- **Mood**: Imperative mood (e.g., "add", not "adds").

## Configuration Schema

Configuration is stored in TOML format. A formal JSON Schema is available at `config.schema.json`.

### Precedence
Precedence is applied strictly in this order (highest to lowest):
1.  Environment variables (e.g., `CHRISTINA_MODEL`).
2.  Active profile values.
3.  User config file (`~/.config/christina/config.toml`, or the platform equivalent).
4.  Hardcoded defaults.

## Git Diff Requirements

Christina interfaces with Git via the `git2` library.

- **Status Detection**: Supports Added, Modified, Deleted, Renamed, and Copied.
- **Binary Detection**:
  - Checks for NUL bytes in the first 8KB of content.
  - Matches against `BINARY_EXTENSIONS` (e.g., `.png`, `.exe`).
  - Respects Git's internal binary markers.
- **Size Limits**: Individual file diffs are capped at 1MB to prevent memory exhaustion. Total repository diff processing is capped at 10MB by default.

## Profile Schema

A Profile defines the connection to an LLM provider.

```rust
pub struct ProviderProfile {
    pub name: String,
    pub provider: ProviderKind, // e.g., "azure"
    pub model: ModelName,
    pub api_url: Option<Url>,
    pub api_key: Secret<S>,     // Encrypted/Redacted at runtime
    pub max_input_tokens: TokenCount,
    pub max_output_tokens: TokenCount,
    pub temperature: Option<f32>,
}
```

## Cross-References

- [Advanced Configuration](ADVANCED.md): In-depth guide to manual tweaking.
- [Design Philosophy](DESIGN.md): Motivation for these standards.
