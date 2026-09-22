# Testing

## Native core tests

The native core (`02-native-core-and-ffi.md`, `05-diff-processing-and-chunking.md`)
carries Christina's existing Rust test suite forward with minimal change:
the chunking tests, the binary-detection tests, and the truncation tests in
`christina-core/src/processing/chunking.rs` and
`christina/src/git/diff_processor.rs`. These stay as Rust unit tests inside
`native/charlotte-core`, run with `cargo test`, independent of the
TypeScript test runner. This is the strongest source of confidence in the
whole project, because it is inherited test coverage, not new coverage
written against a guess.

## Orchestrator tests

Christina tests the orchestrator against a mock `Provider` (`Provider::mock`,
`Provider::mock_sequence`, `Provider::mock_sequence_with_delay`). It also
tests against a mock commit history provider
(`MockCommitHistoryProvider`, `FailingCommitHistoryProvider`). Charlotte
needs the same two seams in `packages/orchestrator`. The first is a fake AI
SDK model. The AI SDK ships a `MockLanguageModel` for exactly this purpose.
The second is a fake commit history provider matching the interface in
`04-git-integration.md`. Every orchestrator behavior Christina's tests
cover, including the partial-failure and fallback paths named in
`07-orchestrator-pipeline.md`, gets a matching Bun test.

## Config and profile tests

Christina's config tests cover environment overlay precedence, profile
resolution, and secret redaction. Charlotte writes the equivalent tests
against `packages/config`. This means schema tests for the Zod
definitions, plus a small set of precedence tests that load a temp config
directory.

## Retry and throttle tests

Christina seeds its jitter with a fixed value for deterministic retry-delay
tests (`calculate_delay_with_seed`, `rand_jitter_with_seed`). Charlotte's
`packages/providers` retry and throttle tests use the same seeded-random
pattern, so retry timing tests stay deterministic instead of depending on
wall-clock timing.

## Native boundary tests

`packages/native-core` gets its own test file, separate from both the Rust
tests and the orchestrator tests. It calls every exported native function
directly through `bun:ffi` and checks the returned shape against the
signatures in `02-native-core-and-ffi.md`. This test catches a mismatch
between the Rust side and the TypeScript side early. Otherwise, that
mismatch shows up later as a confusing failure somewhere else in the
stack.

## Scenario checklist

Before a release, run these scenarios against a real repository. They are
not a comparison against Christina. They are the behaviors Charlotte
itself must get right.

1. A small diff under the direct-generation threshold produces a
   Conventional Commit message.
2. A large diff that forces chunking produces chunk boundaries that
   respect the configured token limit.
3. A binary-only diff produces a binary notice instead of a garbled
   prompt.
4. A diff against a lockfile respects the separate lockfile token cap.
5. An empty staged index exits before any model call.
6. A deletion-only diff uses the smaller truncation limit.
7. Every `config` and `profile` subcommand produces the expected output
   for a known config file.
8. A forced provider error triggers the expected retry count and the
   expected partial-failure fallback behavior.

## Open decision

Charlotte's token counts can match Christina's for the same input and the
same model encoding, since both wrap the same `tiktoken-rs` crate. Treat
this as a useful sanity check, not a requirement. Charlotte's native core
can diverge from Christina's later, and the token count can diverge with
it, as long as the divergence is a deliberate choice.
