# Contributing to Christina

Thank you for your interest in contributing to Christina!

## Development Setup

1. **Install Rust and Cargo**: We use the latest stable version of Rust.
2. **Install Just**: [Just](https://github.com/casey/just) is used as a command runner.
3. **Configuration**: You'll need at least one LLM API key to test generation.

### Useful Commands

- `just check`: Check if the project compiles.
- `just clippy`: Run lints (we treat warnings as errors).
- `just test`: Run all tests.
- `just fmt`: Format the codebase.

## Project Structure

- `christina/`: The CLI application.
- `christina-core/`: The core logic library.

## Coding Standards

We follow strict Rust idioms as outlined in our [Architecture Documentation](ARCHITECTURE.md):

- No `unsafe` code.
- Prefer concrete types over complex trait abstractions unless polymorphic behavior is required.
- Maintain "correct by construction" invariants.
- Ensure all public APIs are documented.
- All new features should include corresponding tests.

## Pull Request Process

1. Fork the repository and create your branch from `main`.
2. Ensure `just clippy` and `just test` pass.
3. Update documentation if you're adding new features or changing existing behavior.
4. Submit a PR with a clear description of the changes.

## License

By contributing, you agree that your contributions will be licensed under the project's [MIT](LICENSE-MIT) and [Apache 2.0](LICENSE-APACHE) licenses.
