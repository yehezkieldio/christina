# Contributing to Christina

We maintain high standards for code quality, performance, and type safety. This guide details the technical requirements for contributors.

## Development Environment

1.  **Toolchain**: Latest stable Rust.
2.  **Workflow**: We use `just` for automated checks.
3.  **Tests**: We use `nextest` for faster, parallel test execution.

```bash
cargo install cargo-nextest
```

### Essential Commands

| Command | Purpose |
|:--- |:--- |
| `just all` | Run formatter, linter, and all tests. |
| `just clippy` | Execute strict linting (warnings are errors). |
| `just test` | Run the unit and integration test suite. |
| `just check` | Verify compilation across the workspace. |

## Coding Standards

### 1. Ownership & Safety
- **Zero Unsafe**: The use of `unsafe` is prohibited.
- **Explicit Ownership**: Design with ownership semantics. Mutation should happen through replacement rather than aliasing where possible.
- **Short-lived Borrows**: References should be transient and local. Avoid storing references in long-lived structs.

### 2. Type System as Invariants
- Use the **Correct by Construction** pattern.
- Avoid primitive obsession. Use newtypes (e.g., `TokenCount`, `GenerationId`) to enforce domain constraints at the type level.
- Handle recoverable failures with `Result`. Programming errors and invariant violations should `panic!`.

### 3. Performance
- Performance is a design-time property. Identify hot paths (tokenization, chunking) and optimize for throughput.
- Minimize allocations in the generation pipeline.
- Use the benchmark suite (`cargo bench`) to verify that changes do not regress performance.

## Pull Request Process

1.  **Fork and Branch**: Create a feature branch from `main`.
2.  **Verified Commits**: We recommend using `christina` to generate your own commit messages.
3.  **Quality Gate**: Ensure `just all` passes with zero warnings and zero failures.
4.  **Documentation**: Update relevant `.md` files if you modify the pipeline or configuration schema.
5.  **Review**: PRs require technical review. Focus on clarity, performance, and idiomatic Rust.

## License

By contributing, you agree that your contributions will be licensed under the project's [MIT](LICENSE-MIT) and [Apache 2.0](LICENSE-APACHE) licenses.
