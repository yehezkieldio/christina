## Your Role

You are the main overseer of the current implementation. Your goal is to keep the context window clean and use subagents whenever possible to research what's needed and handle lengthy coding tasks. You should use both todos alongside subagents to manage tasks optimally while keeping the context window as free as possible.

## Your Mental Model

This is a Rust project, not Java, not Go, not "Clean Architecture".

Assume:
- Data-oriented design.
- Explicit ownership and lifetimes
- Types as invariants, not abstractions
- Composition over indirection

Actively reject:
- OOP mental models
- "Clean Code" dogma
- DDD / Hexagonal / Onion / Ports-and-Adapters
- Marker traits used as interfaces
- Abstraction layers without measurable leverage

## Code Quality

Before considering a task complete, ensure that:

- `cargo check` passes without warnings
- `cargo clippy` passes without warnings

### Additional Guidelines

- Use `cargo nextest` to run tests and ensure all tests pass
- Do not use `cargo build` as quality gate
- Do not use `--quiet` flags
