# Git integration

## Scope

Charlotte reads two things from a Git repository: the staged diff, and a bounded window of recent commit subjects for style context. Charlotte does not write to Git directly. It shells out to the operator's own `git commit` at the final step, the same way Christina does through `christina/src/git/adapter.rs`.

## Native reading path

`read_staged_diff` and `read_commit_history`, defined in `02-native-core-and-ffi.md`, wrap `git2` (a Rust binding to `libgit2`). Christina already uses `git2` for this purpose in `christina-core/src/git/repository.rs` and `christina/src/git/adapter.rs`, and Charlotte keeps the same library for two reasons. First, `git2` reads the index directly instead of parsing `git diff` output, which avoids a whole class of porcelain-format parsing bugs. Second, it lets `read_commit_history` walk the commit graph without spawning a `git log` subprocess per call.

## Staged-only guarantee

When the index holds no staged changes, Christina exits before contacting any model. Charlotte keeps this guarantee inside the native `read_staged_diff` call. An empty diff result short-circuits the CLI before configuration finishes loading, matching Christina's current fast-exit behavior.

## Diff processing

Binary detection and diff truncation move into the native core alongside diff reading, described in `02-native-core-and-ffi.md` and `05-diff-processing-and-chunking.md`. Christina's `DiffProcessor` (`christina/src/git/diff_processor.rs`) combines binary sniffing, per-file truncation, and size limiting in one struct. Charlotte splits these into separate native functions with one job each. The boundary contract in `01-architecture.md` favors small plain-data functions over one large stateful struct crossing the FFI edge.

## Commit history provider

Christina defines a `CommitHistoryProvider` trait (`christina/src/generate.rs`) with a real Git-backed implementation and a mock implementation for tests. Charlotte keeps this shape as a TypeScript interface with two implementations: a native-backed provider calling `read_commit_history`, and an in-memory fake provider for orchestrator tests. Keeping the interface at the TypeScript layer, not inside the native core, lets orchestrator tests run without loading the native module at all.

## Open decision

Christina's adapter shells out to the system `git` binary for the commit step itself, not `git2`. The likely reason is that this reuses the operator's own commit hooks and signing setup. Charlotte needs a check on whether this is still the right default before implementation starts. A native `git2`-based commit skips `pre-commit` and `commit-msg` hooks unless Charlotte reimplements hook invocation itself.
