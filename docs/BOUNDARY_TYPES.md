# Boundary Types

Christina should add domain types only where a primitive crosses a module
boundary with ambiguous meaning or invalid states.

## Current Covered Boundaries

- `TokenCount` separates token budgets from byte counts, line counts, and raw
  lengths.
- `FilePath` keeps repository paths relative and prevents absolute-path drift.
- `CommitMessage` seals Conventional Commit validation before Git writes.
- `ModelName` keeps provider model identifiers cheap to clone without exposing
  provider internals.
- `GenerationId` already exists for request identity in `christina-core`.

## Audited Primitive Boundaries

- Diff bytes, line counts, staged file counts, and trace counters stay as
  primitives because they are local telemetry values and do not drive domain
  decisions.
- Retry attempts stay as `u32` inside the retry loop because they are scoped to
  one function and never cross the provider boundary.
- Commit SHAs stay as strings in CLI history formatting because Christina only
  displays abbreviated values and never accepts them back as identifiers.
- Provider concurrency stays as `usize` because `RuntimeConfig` clamps it before
  orchestration receives it.

## Rule

Add a newtype when the value is passed across modules and the type can prevent a
real bug. Do not add marker traits or wrapper types for symmetry alone.
