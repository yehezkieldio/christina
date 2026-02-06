# Developer Guide: Design Patterns in Christina

This document explains the core Rust design patterns used throughout the Christina codebase.

## 1. "Correct by Construction" (Type Safety)
We use the type system to make invalid states unrepresentable.

### Examples:
- **`TokenCount`**: Instead of a raw `usize`, we use a `TokenCount` newtype. This prevents accidental math errors between tokens and bytes.
- **`Secret`**: Sensitive data is wrapped in a `Secret<T>` type that prevents accidental logging (implements `Debug` by masking) and ensures it's only accessible when needed.
- **`UsageTier`**: An enum that strictly defines the limits for different account types, preventing the system from ever attempting to send a request that would exceed provider limits.

## 2. Data-Oriented Design (DOD)
Christina avoids deep object hierarchies and complex inheritance (which Rust doesn't support anyway). Instead, we focus on:
- **Structs as Data**: Structs are simple containers for state.
- **Free Functions**: Many operations are implemented as free functions in modules rather than methods on structs, making the data flow more explicit.
- **Linear Pipelines**: The standard flow is `RawData -> ValidatedData -> TransformedData -> ConsumedData`.

## 3. Explicit Ownership and Lifetimes
We follow the rule: "Ownership moves forward in time, not sideways."
- **Moves by Default**: We prefer moving values into functions rather than taking long-lived references.
- **Scoped Borrows**: When borrowing is necessary (e.g., during diff processing), we keep the borrows extremely short-lived.
- **Avoid Interior Mutability**: `RefCell` and `Mutex` are used sparingly and only where absolutely necessary (e.g., in the TUI or global logging).

## 4. Error Handling as Domain Logic
Errors are not just strings; they are part of the API.
- `christina-core` defines a central `Error` enum using `thiserror`.
- Every variant is documented and carries necessary context (e.g., which file failed to open).
- We use `anyhow` in the CLI crate for easy error propagation and context wrapping.

## 5. Composition over Indirection
We avoid "Service" layers and "Manager" classes. If a component needs to do two things, we compose it from two simpler components rather than creating an abstract interface.
