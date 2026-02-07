# Final Audit Report

Workspace: christina (christina-core + christina CLI). Edition 2024, resolver 3.
Quality gates: `just check`, `just clippy` (zero warnings, -D warnings), `just test`.

Each finding is a self-contained unit. Findings within the same priority tier are independent and parallelizable unless stated otherwise.

---

## P0 — Bugs and Correctness

### P0-01: FilePath::From impls panic on absolute paths — ✅ Solved

- Location: christina-core/src/types/path.rs lines 87-97
- Problem: `From<String>` and `From<&str>` call `FilePath::new()` which uses `assert!()`. Any git output with an absolute path (symlink targets, submodule paths, malformed git status) causes a panic in production.
- Assumption: `GitFile::new()` in christina-core/src/git/mod.rs and `get_delta_path()` in christina/src/git/adapter.rs pass git2 paths directly into `FilePath::from()`.
- Approach: Change both `From` impls to call `try_new()` and panic only with a debug_assert. Alternatively, make `From` impls use `try_new().expect()` with a descriptive message, or remove `From` impls entirely and force callers to use `try_new()` returning Result. The safest option: remove `From<String>` and `From<&str>` impls, keep only `new()` (panicking, for trusted inputs) and `try_new()` (fallible, for external inputs). Update all call sites in adapter.rs to use `try_new()` with `?` propagation.

### P0-02: LlmRequest::validate checks impossible condition — ✅ Solved

- Location: christina-core/src/llm/request.rs lines 77-81
- Problem: `max_tokens` is `TokenCount` which wraps `NonZeroU32`. The check `self.max_tokens.get() == 0` can never be true. This is dead validation code that gives false safety assurance.
- Approach: Remove the `max_tokens.get() == 0` check entirely. The type system already prevents this state.

### P0-03: Config::load default profile creation can crash CLI on startup — ✅ Solved

- Location: christina/src/config/settings.rs lines 240-248
- Problem: `config.profiles.add(default_profile)?` propagates error with `?`. If a profile named "default" somehow fails validation (e.g., future validation changes), the entire CLI crashes on startup with an opaque error.
- Approach: Replace `?` with a match that logs a warning and continues without the default profile, or use `add_or_update` semantics that cannot fail for valid profiles.

### P0-04: DiffBuilder loses parent state on file() — ✅ Solved

- Location: christina-core/src/test_helpers.rs lines 519-521, 568-569
- Problem: `DiffBuilder::file()` consumes `self` but `FileDiffBuilder` has no reference back to the `DiffBuilder`. `and_file()` creates an orphaned `FileDiffBuilder` that discards the previous file's data. Multi-file diffs cannot be constructed via the builder.
- Approach: Store accumulated files in `FileDiffBuilder` by passing `Vec<FileDiffBuilder>` through the chain, or change `file()` to take `&mut self` and return a sub-builder that borrows the parent. The simplest fix: `FileDiffBuilder::and_file()` should consume self, push self into an internal Vec, and return a new `FileDiffBuilder` that carries the Vec. `build()` on the final `FileDiffBuilder` produces the complete multi-file diff. This is test-only code.

### P0-05: CompletionError::from_api_error substring false positives — ✅ Solved

- Location: christina-core/src/error.rs lines 162-229
- Problem: Matching "400", "500", "404" as substrings will false-positive on messages like "processed 400 items" or "batch size 500". This misclassifies permanent errors as transient (ServerError) or transient errors as permanent (InvalidResponse).
- Approach: Match against HTTP status code patterns more precisely: check for "400 " (with space/punctuation after) or "HTTP 400" or "status: 400" patterns. Alternatively, prefix-match status codes at word boundaries. Use patterns like `" 400"`, `"400 "`, or regex `\b400\b` via a simple helper function that checks word boundaries without pulling in the regex crate.

### P0-06: azure.rs creates new reqwest::Client per request — ✅ Solved

- Location: christina/src/engines/default/azure.rs line 175
- Problem: `Client::new()` is called inside `execute_azure_request_inner()` on every request. This discards HTTP/2 connection pools, TLS sessions, and DNS cache. Each request pays full TCP+TLS handshake cost.
- Approach: Create a module-level `OnceLock<Client>` or pass `Client` through the function chain. A `OnceLock<Client>` with sensible defaults (timeout, pool idle timeout) is simplest. The client should be configured with a reasonable timeout (e.g., 30s connect, 120s total) since the orchestrator provides its own timeout wrapper.

### P0-07: groq.rs retry clones Strings instead of using Arc — ✅ Solved

- Location: christina/src/engines/default/groq.rs lines 31-47
- Problem: `execute_groq_request_with_retry` clones `request`, `api_key`, `base_url`, and `model` as owned Strings per retry attempt. The OpenAI equivalent uses `Arc<str>` for the same purpose.
- Approach: Change to `Arc<str>` for `api_key` and `model`, `Arc<LlmRequest>` for request, `Option<Arc<str>>` for `base_url`, matching the pattern in openai.rs.

---

## P1 — Hardening (Production Safety)

### P1-01: No timeout on git2 operations — ✅ Solved

- Location: christina/src/git/adapter.rs (all public functions)
- Problem: git2 calls (`diff_tree_to_index`, `find_similar`, `diff.print`, `create_commit`) have no timeout. A locked repository or NFS stall hangs the process indefinitely.
- Approach: Wrap git2 operations in `tokio::task::spawn_blocking` with `tokio::time::timeout`. Use a configurable timeout (default 30s). Return a descriptive error on timeout.

### P1-02: No MAX_DIFF_SIZE enforcement at git adapter level — ✅ Solved

- Location: christina/src/git/adapter.rs `build_staged_diff()`
- Problem: `build_staged_diff()` accumulates the entire diff string without size limits. A repository with a 500MB binary accidentally staged will OOM before `DiffProcessor` checks.
- Approach: Add a size check inside the `diff.print` callback. When accumulated bytes exceed `MAX_DIFF_SIZE`, stop accumulating and append a truncation notice. Alternatively, check total diff stat size before calling `diff.print`.

### P1-03: azure.rs reqwest Client has no timeout — ✅ Solved

- Location: christina/src/engines/default/azure.rs line 175-182
- Problem: Even after fixing P0-06, the Client needs explicit timeouts. Without them, a hung Azure endpoint blocks the tokio runtime thread pool.
- Approach: Configure the Client with `.timeout(Duration::from_secs(120))` and `.connect_timeout(Duration::from_secs(10))`.

### P1-04: Config file writes lack atomic protection — ✅ Solved

- Location: christina/src/config/settings.rs (config save paths, profile persistence)
- Problem: Config writes use `std::fs::write()` directly. A crash mid-write produces a truncated config file that fails to parse on next startup, effectively bricking the CLI until manually fixed.
- Approach: Write to a temporary file in the same directory, then `std::fs::rename()` into place. This is atomic on POSIX filesystems. On Windows, use `std::fs::rename` which is also atomic for same-volume renames.

### P1-05: ProviderProfile::new sets azure_api_version for all providers — ✅ Solved

- Location: christina-core/src/profile.rs lines 107-120
- Problem: `ProviderProfile::new()` unconditionally sets `azure_api_version: Some("2024-12-01-preview")` even for OpenAI and Groq providers. This leaks Azure-specific defaults into non-Azure profiles.
- Approach: Set `azure_api_version` to `None` in `new()`. Only set it when the provider is `ProviderKind::Azure`, or let the Azure provider constructor handle the default.

### P1-06: No validation that max_input_tokens > max_output_tokens at config load — ✅ Solved

- Location: christina/src/config/settings.rs `Config::validate()`
- Problem: A user can set max_input_tokens=500 and max_output_tokens=4000. This passes config validation but fails later with a confusing arithmetic underflow error in generate.rs budget calculation.
- Approach: Add a check in `Config::validate()` that `max_input_tokens > max_output_tokens` and emit a clear warning/error.

### P1-07: std::sync::Mutex used in async context — ✅ Solved

- Location: christina/src/orchestrator/mod.rs (TraceStats or similar shared state inside async blocks)
- Problem: `std::sync::Mutex` blocks the tokio worker thread while held. If contention occurs during concurrent map-phase tasks, this degrades async throughput.
- Approach: Audit usage: if the lock is held across `.await` points, switch to `tokio::sync::Mutex`. If the lock is held only for brief synchronous operations (counter increments), `std::sync::Mutex` is acceptable but should be documented with a comment explaining why.

---

## P2 — Dead Code and Stubs

### P2-01: telemetry/filters.rs is empty — ✅ Solved

- Location: christina/src/telemetry/filters.rs
- Problem: Module declared, doc comment present, zero implementation. Privacy-sensitive data (API keys) can appear in log output.
- Approach: Delete the module and its declaration in mod.rs.

### P2-02: ui/components/mod.rs is empty — ✅ Solved

- Location: christina/src/ui/components/mod.rs
- Problem: Empty module stub. Adds dead weight to the module tree.
- Approach: Delete the file and remove `pub mod components;` from ui/mod.rs.

### P2-03: Five Event variants are never emitted — ✅ Solved

- Location: christina/src/ui/events.rs lines 34-83
- Problem: `ChunkProcessed`, `RetryAttempt`, `CommitCreated`, `DiffProcessed`, `ProviderConnecting` are all `#[allow(dead_code)]`. They are never constructed anywhere in the codebase.
- Approach: Delete the unused variants. They can be re-added when actually needed. Suppressing dead_code warnings masks real issues.

### P2-04: git/diff_gen.rs is an empty module — ✅ Solved

- Location: christina-core/src/git/diff_gen.rs
- Problem: Contains only a doc comment, no code. The module is declared in git/mod.rs.
- Approach: Delete the file and remove its declaration from git/mod.rs.

---

## P3 — Code Quality and Duplication

### P3-01: Duplicated convert_messages and extract_system_prompt

- Location: christina/src/engines/default/openai.rs lines 119-135, christina/src/engines/default/groq.rs lines 105-121
- Problem: Identical functions in both files. Maintenance burden; a fix in one must be mirrored in the other.
- Approach: Extract both functions into christina/src/engines/default/mod.rs as `pub(super)` or `pub(crate)` functions. Update openai.rs and groq.rs to import from the parent module.

### P3-02: Duplicated diff collection logic in adapter.rs

- Location: christina/src/git/adapter.rs `get_staged_files()` lines 23-118, `get_unstaged_files()` lines 121-207
- Problem: Near-identical code for collecting file metadata and diff content. Only the diff source differs (`diff_tree_to_index` vs `diff_index_to_workdir`).
- Approach: Extract the shared logic into a private `collect_files_from_diff(diff: &git2::Diff) -> Result<Vec<GitFile>>` function. Both public functions create the diff and delegate to the shared collector.

### P3-03: clamp_partial_failure_rate warnings are unreachable

- Location: christina/src/config/settings.rs lines 49-74
- Problem: Warnings for 0.0 and 1.0 are generated before clamping to 0.01-0.50. Since 0.0 and 1.0 are outside the clamp range, the clamp warning will always fire, making the specific 0.0/1.0 warnings redundant noise.
- Approach: Remove the 0.0 and 1.0 specific checks. The clamp warning already covers these cases with a more informative message showing the actual clamped value.

### P3-04: PipelineState::advance_chunk uses unreachable!

- Location: christina-core/src/pipeline/state.rs line 55
- Problem: `unreachable!()` panics in release builds (with panic=abort, this terminates the process). This is a programming error, not an invariant violation that should terminate.
- Approach: This is acceptable per AGENTS.md ("Programming errors and invariant violations panic"). However, the wildcard match should use `panic!()` with a descriptive message rather than `unreachable!()` since the state is not actually unreachable — it is reachable but represents a programming error. Change `unreachable!()` to `panic!("advance_chunk called on non-Analyzing state: {:?}", self)` for better diagnostics.

---

## P4 — Improvements and Hardening Opportunities

### P4-01: Implement Tokenizer trait for TokenizerService

- Location: christina-core/src/processing/tokenizer.rs
- Problem: `TokenizerService` implements `count_tokens_exact`, `encode`, `decode` as inherent methods, plus `slice_to_token_limit` as an inherent method. It also has the `Tokenizer` trait impl (via the trait definition in tokenizer.rs). The inherent `slice_to_token_limit` differs from the trait's default impl by adding line-boundary preference. This behavioral inconsistency is confusing.
- Approach: Ensure `slice_to_token_limit` is only defined in one place. If the line-boundary behavior is desired, override the trait method. If not, remove the inherent method and use the trait default.

### P4-02: Free-tier limits gated behind experimental + Groq only

- Location: christina/src/generate.rs lines 96-118
- Problem: Free-tier rate limiting only applies when `use_experimental && usage_tier == Free && provider == Groq`. This is undocumented and surprising. Other providers have free tiers too.
- Approach: Either apply free-tier limits to all providers when `usage_tier == Free` (remove the Groq check), or document clearly in config.example.toml that free-tier limits are Groq-only. If keeping Groq-only, remove the `use_experimental` gate since it serves no purpose if the feature is provider-specific.

### P4-03: HashMap non-deterministic serialization for profiles

- Location: christina-core/src/profile.rs line 145
- Problem: `Profiles` uses `HashMap<String, ProviderProfile<S>>`. TOML serialization order is non-deterministic, causing unnecessary config file diffs.
- Approach: Replace `HashMap` with `BTreeMap` for deterministic key ordering. This also makes `list_names()` trivial (no sort needed).

### P4-04: Commit workflow clones diff string per regeneration loop

- Location: christina/src/cli/commit.rs (the regeneration loop)
- Problem: `diff.clone()` is called on every loop iteration when the user requests regeneration. For large diffs this is wasteful.
- Approach: Use `Arc<str>` for the diff string so clones are cheap reference count increments. Or pass `&str` into the generation function if ownership is not required.

### P4-05: Binary detection uses contains(&0) for small files

- Location: christina/src/git/diff_processor.rs lines 73-81
- Problem: For files <8KB, `content_bytes.contains(&0)` iterates byte-by-byte. `memchr::memchr(0, content_bytes)` is faster even for small inputs due to SIMD.
- Approach: Use `memchr::memchr(0, content_bytes).is_some()` for both small and large files. Remove the branching on file size.

### P4-06: Global allocator cap is effectively disabled

- Location: christina/src/main.rs or lib.rs (mimalloc + cap allocator setup)
- Problem: The memory cap is set to `usize::MAX`, which on 64-bit systems is 18 exabytes. This provides no actual protection.
- Approach: Set a reasonable cap (e.g., 512MB or 1GB) based on expected workload. Large diffs are the primary memory consumer; a 512MB cap covers diffs up to ~200MB with processing overhead.

---

## Execution Notes

- All P0 items must be completed before P1.
- P1 items are independent of each other.
- P2 items are independent of each other and of all other tiers.
- P3 items are independent of each other except P3-01 and P3-02 both touch adapter.rs/engines — no file overlap, safe to parallelize.
- P4 items are independent of each other.
- After all fixes: run `just fmt`, `just clippy`, `just test` to verify.
