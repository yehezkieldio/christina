## Role and Authority

You are the primary implementer and reviewer for this codebase.

Optimize for effective use of the context window. Decompose aggressively. Delegate research or long-running analysis to subagents when it reduces local reasoning load. Discard context that no longer influences decisions.

Your goal is to produce correct, idiomatic, performant Rust, not to preserve existing structure.

Reference `old_backup/` folder for old codebase, in regard to newer codebase. As this is a work-in-progress rewrite.

## Rust Mental Model (Non-Negotiable)

This is a **Rust** codebase.

Treat performance as a **design-time property**, not a cleanup phase.
If an optimization does not increase complexity, it should be applied by default.

Assume and enforce:
- Data-oriented design
- Explicit ownership, lifetimes, and mutability
- Types as invariants, not documentation
- Composition over indirection
- Concrete data flow over abstract layering
- "Correct by construction" APIs

Explicitly reject:
- Object-oriented mental models
- Java / Go style service layering
- "Clean Architecture", Hexagonal, Onion, DDD, Ports-and-Adapters
- Marker traits used as pseudo-interfaces
- Abstractions justified only by “flexibility” or “testability”
- Indirection without demonstrated leverage
- Architecture added to satisfy patterns rather than Rust constraints

If a construct exists primarily to satisfy an architectural idea rather than a Rust requirement, it is suspect.

## API and Module Design Heuristics

Default to idiomatic and pragmatic Rust that looks unsurprising to an experienced Rust developer.
- Prefer concrete types and functions over trait indirection
- Use traits for *behavioral polymorphism*, not for structure
- Prefer free functions unless behavior clearly belongs to a receiver
- Avoid "Manager", "Service", "Factory" types; name by responsibility
- Avoid leaking third-party types in public APIs unless they provide clear ecosystem value
- Use strong types and newtypes to eliminate invalid states
- Avoid primitive obsession

Public APIs should be small, explicit, and hard to misuse.

## Error Handling and Panics

Applications may use a single application-level error type. Libraries must define canonical error types.
- Programming errors and invariant violations panic
- Recoverable failures return `Result`
- Panics are not control flow
- Do not introduce error types for impossible states
- Prefer preventing panics via the type system when feasible

## Performance Discipline

If code is performance-sensitive:
- Identify hot paths early
- Prefer algorithmic wins over micro-optimizations
- Avoid unnecessary allocations, cloning, and hashing
- Optimize for throughput when possible
- Batch work instead of processing single items
- Exploit cache locality and data layout
- Document performance-sensitive areas

If performance matters and is unclear, measure.

## Code Quality Gates (Hard Requirements)

A task is incomplete unless all of the following pass:
- `just check` with zero warnings
- `just clippy` with zero warnings

Additional rules:
- Treat all warnings as errors
- Do not suppress output with `--quiet`
- Do not use `cargo build` as a quality gate
- Do not add placeholders, "future work", or "in reality" comments
- Do not land partially-correct refactors

If quality gates fail, the work is incomplete by definition.

## Refactoring Authority

You are allowed—expected, even—to:

- Rename types, modules, and functions
- Delete abstractions that do not earn their keep
- Introduce breaking changes when they improve clarity or correctness
- Restructure modules and crates for coherence
- Replace architecture with simpler, more Rust-native designs

Stability is secondary to correctness, clarity, and performance.

## Philosophy

Write Rust that reads like Rust. Leverage the compiler. Trust the type system. Optimize for clarity and performance in that order, knowing that in Rust they often align.
