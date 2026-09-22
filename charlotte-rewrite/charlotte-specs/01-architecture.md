# Architecture

## Build system

Charlotte uses a Bun workspace for package management and Turborepo for
task orchestration (build, test, lint, and type-check, run across packages
with dependency-aware caching). A `turbo.json` at the repository root
defines the task graph. Each package's `build` task depends on the `build` task of every package
it imports, through `"dependsOn": ["^build"]`. Turborepo then builds
`native-core` before anything that imports it, and skips rebuilding a
package whose inputs have not changed.

## Workspace layout

Charlotte splits into one app and several small packages. No package holds
more than one responsibility.

- `apps/cli`: the executable. Argument parsing, process wiring, and the
  top-level command handlers. This is the only package that produces a
  binary through `bun build --compile`.
- `packages/ui`: the interactive terminal UI, styled output helpers, and
  spinners. `apps/cli` depends on it. Nothing else does.
- `packages/config`: the config file loader, profile store, and secret
  resolution.
- `packages/schemas`: the Zod schemas for structured model output
  (summary, theme, commit message) and for session events. Both
  `packages/providers` and `packages/session` depend on this package, so
  the schemas live in one place instead of two.
- `packages/providers`: the AI SDK integration. Model calls, provider
  selection, and retry and throttle logic.
- `packages/orchestrator`: the map-reduce generation pipeline: chunk
  summarization, hierarchical intent extraction, and message reduction. It
  depends on `packages/providers` and `packages/schemas`, not on
  `apps/cli` or `packages/ui`.
- `packages/session`: the session transcript writer and reader, and the
  stats aggregation.
- `packages/native-core`: the TypeScript bindings for the Rust native
  core, loaded through `bun:ffi`. Every other package that needs a native
  call (git reading, chunking, tokenization) depends on this package,
  never on the native library directly.
- `native/charlotte-core`: the Rust crate that compiles to a native
  library and backs `packages/native-core`. This is not a Bun workspace
  package. Turborepo invokes its `cargo build` through a task defined in
  `packages/native-core`.

A dependency only ever points from a feature package toward
`packages/native-core`, `packages/schemas`, or `packages/config`, never the
other way. This keeps the dependency graph a tree, not a web, and it is
why `packages/ui` and `packages/orchestrator` do not depend on each other:
`apps/cli` is the only place that wires them together.

## Language split

TypeScript owns the config layer, the CLI, the interactive UI, the
orchestrator control flow, provider integration, and session storage. Rust
owns diff chunking, tokenization, binary detection, and staged diff
extraction through `git2`.

The split follows the hot-path evidence from Christina's own codebase graph.
The graph's ten busiest functions by inbound call count are dominated by
`TokenCount::new_at_least_one`, `ChunkBuffer::is_empty`, and the chunking and
diff-processing functions around them. These stay in Rust. Orchestration
functions such as `AIOrchestrator::new` and `RetryPolicy::new` show up in
the same hotspot list for a different reason. Christina calls them from
many test cases, not because they do heavy computation. These move to
TypeScript.

## Data flow

A generation run in Charlotte follows six stages.

1. Read: `packages/native-core` opens the repository. It makes sure that
   the index holds staged changes, then returns the staged diff and
   requested commit history.
2. Configure: `packages/config` loads the global config, profile, and
   environment overlay, and resolves the API key.
3. Contextualize: `packages/orchestrator` builds prompt context from file
   names, user-supplied context text, and trimmed commit history.
4. Analyze: small diffs go through a direct prompt. Large diffs go through
   `packages/native-core`'s chunker, then through concurrent summarization
   and hierarchical intent extraction in `packages/orchestrator`, using AI
   SDK calls from `packages/providers`.
5. Clean and match: the generated message is cleaned and matched against
   the configured Conventional Commit mode.
6. Commit and record: `apps/cli` shows the message for the operator to
   accept or reject. `packages/session` writes the transcript, and Git
   creates the commit unless `--dry-run` is set.

## Boundary contract

Every call across the TypeScript-to-Rust boundary passes plain data:
strings, byte buffers, and small numeric arguments. No JavaScript object,
closure, or class instance crosses the boundary. `02-native-core-and-ffi.md`
defines the exact function signatures.
