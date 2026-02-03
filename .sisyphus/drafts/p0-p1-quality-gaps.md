# Draft: Christina Quality Gaps Implementation Plan

## Requirements (confirmed)

### P0 - MUST FIX (Blocking Release)

1. **FilePath Runtime Validation**
   - Location: `christina-core/src/types/file_path.rs`
   - Current: `FilePath::new()` uses `debug_assert!()` - validation only in debug builds
   - `try_new()` exists but unused in production code
   - 52 call sites using `FilePath::from()` which delegates to `new()`
   - Key call sites:
     - `staging.rs:231` - TUI screen
     - `dashboard.rs:648` - TUI screen
     - `file.rs:99` - git file processing
     - `parsing.rs:106` - diff parsing
     - `chunking.rs:523` - chunking logic
     - `buffer_pool.rs:128` - test only
     - `orchestrator.rs:1135+` - tests only
   - **VERIFIED**: Most call sites are in test code, but production code exists in staging.rs, dashboard.rs, file.rs, parsing.rs, chunking.rs

2. **Config Schema Version**
   - Location: `christina/src/config/settings.rs`
   - Current: Config struct has NO version field
   - Verified: Examined full Config struct - no schema versioning
   - Need: Add `schema_version: u32` field with serde default

3. **Tracing Subscriber Configuration**
   - Location: `christina/src/main.rs`
   - Current: `verbose: u8` CLI flag exists (clap action=Count)
   - Verified: NO tracing subscriber configured - `tracing::info!()` calls have no effect
   - `tracing = "0.1.41"` in dependencies but no `tracing-subscriber`
   - TUI apps MUST log to file (not stdout) to avoid terminal corruption
   - **Note**: generate.rs uses `tracing::{info, warn}` but no subscriber configured!

4. **Atomic Config File Writes**
   - Location: `christina/src/config/settings.rs`
   - **STATUS: ALREADY IMPLEMENTED** ✅
   - Uses temp file + sync_all + lock_exclusive + rename (line 276-314)
   - No action needed

5. **TokenBudget Invariant Validation**
   - Location: `christina/src/io/llm/tokenizer.rs`
   - Current: `TokenBudget::new()` performs NO validation
   - Validation only happens later in `remaining_for_diff()`
   - **VERIFIED**: Constructor accepts any values, validation deferred

### P1 - SHOULD HAVE (Release Quality)

6. **Shell Completions**
   - Location: `christina/src/cli.rs`
   - No `clap_complete` dependency
   - Need to add subcommand for completion generation

7. **Plaintext API Key Warnings**
   - Location: `christina-core/src/config/secret.rs`
   - Current: `SecretRef::parse()` accepts plain strings as `Literal` without warning
   - Line 115-118: Falls through to Literal for any non-prefixed string
   - No security warning for potential plaintext API keys

8. **TokenizerService Caching**
   - Location: `christina/src/generate.rs` line 120
   - Current: `TokenizerService::new()` called fresh each generation
   - `OnceLock` pattern exists in tokenizer.rs but only for tests (line 17)
   - Production code doesn't use the cached instance

9. **--dry-run Flag**
   - Location: `christina/src/cli.rs` and `christina/src/cli/commit.rs`
   - No dry-run mode exists
   - Need to add flag and skip actual commit creation

## Technical Decisions

- For FilePath: Use `assert!()` instead of `debug_assert!()` for unconditional validation
- For Config version: Use `#[serde(default)]` to maintain backward compatibility
- For Tracing: Use `tracing-appender` for file-based logging (TUI-safe)
- For TokenBudget: Add `try_new()` that validates upfront, keep `new()` for backward compat
- For completions: Add `Completions` subcommand with shell enum

## Research Findings

- 52 FilePath construction call sites found (most in tests)
- Production FilePath usages: staging.rs, dashboard.rs, file.rs, parsing.rs, chunking.rs
- Config already has atomic write - P0#4 is complete
- TokenizerService has internal LRU cache but instance recreation is the issue
- `tracing` crate already in dependencies, just needs subscriber setup

## Decisions (Finalized)

1. **FilePath validation**: Use unconditional `assert!` instead of `debug_assert!`
   - Rationale: Zero migration needed, panic on invalid paths in all builds
   - `try_new()` already exists for callers who want Result

2. **Verbose flag mapping**: `0→INFO, 1→DEBUG, 2+→TRACE`
   - Rationale: Standard convention, INFO default is appropriate for TUI apps

3. **Secret warning timing**: Warn at parse time in `SecretRef::parse()`
   - Rationale: Earliest possible feedback, users see warning when config loads
   - Use `tracing::warn!` so it respects verbosity settings

## Additional TokenBudget/TokenizerService Findings

### TokenBudget Validation Gap
- `TokenBudget::new()` accepts any values with NO validation
- Validation only happens in `remaining_for_diff()` which computes:
  - `reserved = max_output + reserved_for_prompt + reserved_for_messages`
  - Checks `reserved <= max_input`
- generate.rs calls `new()` then immediately `remaining_for_diff()?.map_err(...)` 
- **Fix**: Add `try_new()` that validates upfront, or move validation into `new()`

### TokenizerService Caching Gap
- `OnceLock<TokenizerService>` exists but is `#[cfg(test)]` only
- Production code in generate.rs creates fresh instance per request
- `tiktoken_rs::o200k_base()` loads BPE model data - expensive
- Internal LRU cache exists but instance recreation wastes it
- **Fix**: Add production global accessor using `OnceLock<Arc<TokenizerService>>`
- Also: `count_tokens()` has minor race - should re-check cache after re-lock before insert

## Scope Boundaries

- INCLUDE: P0 (1,2,3,5) and P1 (6,7,8,9) items
- EXCLUDE: P2 (settings.rs split) - deferred to future
- EXCLUDE: P0#4 (already implemented)

## Config & Tracing Additional Findings

### Config Schema Version Gap
- NO `schema_version` field exists in Config struct
- serde(default) provides forward/backward tolerance but no explicit versioning
- Many fields use `#[serde(skip_serializing)]` (api_key, token limits, etc.)
- **Fix**: Add `schema_version: u32` field, NOT skip_serializing, implement migration in load()

### Tracing Subscriber Gap
- `verbose: u8` defined in CLI with `clap::ArgAction::Count`
- NO tracing_subscriber initialization anywhere in codebase
- `tracing::{info, warn, debug}` used in: generate.rs, orchestrator.rs, producers.rs
- All tracing macros are NO-OPS without subscriber!
- Orchestrator has custom `debug_enabled()` checking `CHRISTINA_DEBUG` env var
- **Fix**: Add tracing-subscriber + tracing-appender deps, init in main.rs
- For TUI: MUST use file-based logging (tracing_appender::rolling) to avoid terminal corruption
- Map: 0 → INFO, 1 → DEBUG, 2+ → TRACE
