# ADR 001: Crate Split (Core vs. CLI)

## Status
Accepted

## Context
Christina started as a monolithic CLI tool. As the project grew, the need for a headless core became apparent to support potential future frontends (TUI, GUI, Web) and to allow the core logic to be used as a library by other Rust applications.

## Decision
We split the project into a Rust workspace with two primary crates:
1. `christina-core`: Contains all domain logic, Git abstractions, LLM provider interfaces, and configuration resolution. It is strictly headless and has no dependency on CLI-specific libraries like `clap`.
2. `christina`: The CLI entry point. Handles argument parsing, user interaction, terminal UI (when present), and high-level orchestration of the core components.

## Consequences
- **Pros**:
    - Separation of concerns: Logic is decoupled from presentation.
    - Testability: Core logic can be unit tested without involving CLI machinery.
    - Reusability: `christina-core` can be integrated into other tools.
    - Build Times: Changes in the CLI don't require recompiling the core, and vice versa.
- **Cons**:
    - Increased complexity in workspace management and dependency sharing.
    - Boilerplate for passing data between the CLI layer and the core.
