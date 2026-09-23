# Build phases

## Phase 0: workspace and native build pipeline

Scaffold the Bun workspace and Turborepo setup named in `01-architecture.md`. Stand up `native/charlotte-core` and prove that `packages/native-core` can load it through `bun:ffi` before any other phase starts. This is the riskiest piece of infrastructure, per `02-native-core-and-ffi.md`, and a failure here blocks every later phase.

Exit check: `add(a, b)`, a trivial native function, builds through `cargo build` and calls successfully from a Bun script through `bun:ffi`, on at least one platform.

## Phase 1: config

Build `packages/config` as specified in `03-config-and-profiles.md`: the Zod schema, the layered loader, profile storage, and secret resolution. Nothing past this phase can run without a working config layer.

Exit check: every config and profile test named in `10-testing.md` passes.

## Phase 2: native core

Port git reading, binary detection, chunking, and tokenization into `native/charlotte-core`, per `02-native-core-and-ffi.md`, `04-git-integration.md`, and `05-diff-processing-and-chunking.md`. Carry Christina's existing Rust tests across with minimal change.

Exit check: the native core's own Rust test suite passes, and `packages/native-core`'s boundary test suite (`10-testing.md`) passes against a sample diff set.

## Phase 3: schemas, providers, and orchestrator

Build `packages/schemas` (the shared Zod schemas), `packages/providers` (AI SDK integration, retry, throttle), and `packages/orchestrator` (map-reduce pipeline), per `06-providers-and-ai-sdk.md` and `07-orchestrator-pipeline.md`. Run the structured-output reliability check from `06-providers-and-ai-sdk.md` before this phase closes.

Exit check: every orchestrator test named in `10-testing.md` passes against the mock AI SDK model, and the structured-output check passes for every provider Charlotte ships.

## Phase 4: session storage and stats

Build `packages/session`: the event writer and reader, and the `stats` subcommand logic, per `08-session-storage-and-stats.md`. Decide the retention policy named in that spec before this phase closes.

Exit check: a full generation run produces a valid session file, and `charlotte stats` reports correct totals against a small fixture set of session files.

## Phase 5: UI and CLI

Build `packages/ui` (styled output, prompts) and `apps/cli` (argument parsing, command wiring), per `09-cli-and-ui.md`. Fold in the `ui-extractable` styling logic instead of keeping it as a separate package.

Exit check: `apps/cli` runs end to end against a real repository, with accept, edit, regenerate, decline, and `--dry-run` all working through the interactive flow.

## Phase 6: scenario pass

Run the scenario checklist from `10-testing.md` against a real repository. This covers direct generation, chunking, binary diffs, lockfile diffs, an empty index, deletion-only diffs, config commands, and a forced provider error.

Exit check: every scenario in the checklist behaves as specified.

## Sequencing note

These phases are largely sequential, because each depends on the one before it. Config feeds the native core's file resolution. The native core feeds the orchestrator's chunk input. The orchestrator feeds both the session log and the CLI's display layer. Phase 4 and phase 5 can run in parallel once phase 3 closes, since neither depends on the other's output.
