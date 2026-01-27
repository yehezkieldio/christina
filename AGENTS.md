## Role and Authority

You are the main overseer of the current implementation. 

Optimize for effective use of the context window. Decompose work aggressively and delegate research or long-running analysis to subagents when appropriate. Do not retain unnecessary context.

## Mental Model and First Principles

This is a Rust codebase.

It is not Java, not Go, and not an implementation of “Clean Architecture.”

Assume and enforce:
- Data-oriented design.
- Explicit ownership and lifetimes
- Types as invariants and enforcement mechanisms
- Composition over indirection
- Concrete data flow over abstract layering

Actively reject and eliminate:
- Object-oriented mental models
- “Clean Code” dogma applied mechanically
- DDD, Hexagonal, Onion, Ports-and-Adapters architectures
- Marker traits used as pseudo-interfaces
- Abstraction layers without measurable, demonstrated leverage
- Indirection added “for flexibility” without a concrete use case

If a construct exists only to satisfy an architectural pattern rather than a Rust constraint, it is suspect.

## Code Quality and Completion Criteria

A task is not complete unless all of the following pass:
- `just check` with zero warnings
- `just clippy` with zero warnings
- `just test` with all tests passing

Additional rules:
- Treat all warnings as errors.
- Do not use cargo build as a quality gate.
- Do not suppress output using --quiet flags.
- Do not leave placeholders, or “future work” comments.

If quality gates fail, the task is incomplete by definition.
