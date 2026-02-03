# Christina Quality Gaps Implementation Plan (P0 + P1)

## TL;DR

> **Quick Summary**: Fix 8 quality gaps in the Christina codebase - 4 blocking P0 issues (FilePath validation, Config schema version, Tracing subscriber, TokenBudget validation) and 4 P1 quality improvements (Shell completions, Plaintext API key warnings, TokenizerService caching, --dry-run flag).
> 
> **Deliverables**:
> - FilePath unconditional validation in all builds
> - Config schema versioning with migration support
> - File-based tracing for TUI-safe logging
> - TokenBudget upfront validation
> - Shell completion generation (bash/zsh/fish/powershell)
> - Security warnings for plaintext API keys
> - Global TokenizerService caching
> - --dry-run flag for commit preview
> 
> **Estimated Effort**: Medium (8 tasks, ~4-6 hours total)
> **Parallel Execution**: YES - 3 waves
> **Critical Path**: Task 3 (Tracing) → Task 6 (Secret Warning) → Task 8 (dry-run)

---

## Context

### Original Request
Fix quality gaps identified in Christina completeness report. P0 items are blocking release, P1 items are release quality improvements.

### Interview Summary
**Key Discussions**:
- P0#4 (Atomic config writes) already implemented - SKIP
- FilePath validation: Use unconditional `assert!` (not try_new migration)
- Verbose mapping: `0→INFO, 1→DEBUG, 2+→TRACE`
- Secret warning: Warn at parse time using `tracing::warn!`

**Research Findings**:
- 52 FilePath call sites total, production code in: staging.rs, dashboard.rs, file.rs, parsing.rs, chunking.rs
- `tracing` crate already in deps, needs `tracing-subscriber` + `tracing-appender`
- TokenizerService has internal LRU cache but instance recreation (line 120 of generate.rs) wastes it
- No test infrastructure assessment needed - existing tests use bun-equivalent patterns

### Self-Review (Metis-equivalent)
**Identified Gaps** (addressed):
- Tracing must be initialized BEFORE any config loading (so secret warnings work) → Task ordering enforced
- TokenizerService caching requires Arc for thread safety → Design includes Arc<TokenizerService>
- dry-run flag must work in both TUI and CLI modes → Scope clarified to CLI-only (TUI already previews)

---

## Work Objectives

### Core Objective
Bring Christina to release quality by fixing all P0 blocking issues and implementing P1 quality improvements.

### Concrete Deliverables
- `christina-core/src/types/file_path.rs` - unconditional validation
- `christina/src/config/settings.rs` - schema_version field + migration logic
- `christina/src/main.rs` - tracing subscriber initialization
- `christina/src/io/llm/tokenizer.rs` - TokenBudget::try_new() + global accessor
- `christina/src/cli.rs` - Completions subcommand + --dry-run flag
- `christina-core/src/config/secret.rs` - plaintext warning
- `christina/src/generate.rs` - use cached tokenizer
- `christina/src/cli/commit.rs` - dry-run implementation

### Definition of Done
- [ ] `just check` passes with zero warnings
- [ ] `just clippy` passes with zero warnings
- [ ] All existing tests pass
- [ ] New functionality has test coverage where applicable

### Must Have
- FilePath panics on absolute paths in ALL builds (not just debug)
- Config loads successfully with or without schema_version field
- Tracing macros produce visible output when -v flags used
- TokenBudget rejects invalid configurations upfront
- Shell completions generate valid scripts for all shells
- Plaintext API keys trigger visible warning
- TokenizerService instance reused across generations
- --dry-run shows commit message without creating commit

### Must NOT Have (Guardrails)
- NO try_new() migration for FilePath (zero call-site changes)
- NO stdout logging in TUI mode (file appender only)
- NO breaking changes to config file format (serde(default) for version)
- NO changes to existing CLI behavior without explicit flags
- NO premature abstraction (keep changes minimal and focused)
- NO documentation file changes (code-only)

---

## Verification Strategy

> **UNIVERSAL RULE: ZERO HUMAN INTERVENTION**
>
> ALL tasks in this plan MUST be verifiable WITHOUT any human action.

### Test Decision
- **Infrastructure exists**: YES (bun test equivalent via cargo test)
- **Automated tests**: Tests-after where meaningful
- **Framework**: cargo test (workspace-level)

### Agent-Executed QA Scenarios (MANDATORY)

**Verification Tool by Deliverable Type:**

| Type | Tool | How Agent Verifies |
|------|------|-------------------|
| **Rust Code** | Bash (cargo) | `just check`, `just clippy`, `cargo test` |
| **CLI Flags** | Bash (cargo run) | Run christina with flags, verify output |
| **Tracing** | Bash (env + cargo run) | Check log file creation and content |
| **Shell Completions** | Bash | Generate completion, verify valid syntax |

---

## Execution Strategy

### Parallel Execution Waves

```
Wave 1 (Start Immediately - No Dependencies):
├── Task 1: FilePath validation (christina-core)
├── Task 2: Config schema version (christina/config)
├── Task 3: Tracing subscriber (christina/main.rs)
└── Task 4: TokenBudget validation (christina/io/llm)

Wave 2 (After Wave 1 - Depends on Wave 1):
├── Task 5: Shell completions (depends: none, but logically after CLI stabilizes)
├── Task 6: Secret warning (depends: Task 3 for tracing::warn! to work)
└── Task 7: TokenizerService caching (depends: Task 4 for consistent patterns)

Wave 3 (After Wave 2):
└── Task 8: --dry-run flag (depends: Task 5 for CLI structure)
```

### Dependency Matrix

| Task | Depends On | Blocks | Can Parallelize With |
|------|------------|--------|---------------------|
| 1 | None | None | 2, 3, 4 |
| 2 | None | None | 1, 3, 4 |
| 3 | None | 6, 8 | 1, 2, 4 |
| 4 | None | 7 | 1, 2, 3 |
| 5 | None | 8 | 6, 7 |
| 6 | 3 | None | 5, 7 |
| 7 | 4 | None | 5, 6 |
| 8 | 3, 5 | None | None (final) |

### Agent Dispatch Summary

| Wave | Tasks | Recommended Category |
|------|-------|---------------------|
| 1 | 1, 2, 3, 4 | quick (each is single-file, focused change) |
| 2 | 5, 6, 7 | quick (small additions) |
| 3 | 8 | quick (CLI flag + simple logic) |

---

## TODOs

### P0 - MUST FIX (Blocking Release)

- [ ] 1. FilePath: Replace debug_assert! with assert!

  **What to do**:
  - Change `debug_assert!` to `assert!` in `FilePath::new()` (line 26-30)
  - Update docstring to remove "debug builds" mention (line 16-17)
  - Update test `filepath_absolute_panics_debug` to remove `#[cfg(debug_assertions)]` attribute
  - Rename test to `filepath_absolute_panics`

  **Must NOT do**:
  - Do NOT change `try_new()` - it already exists and is correct
  - Do NOT add new error types
  - Do NOT change From impls
  - Do NOT touch any call sites

  **Recommended Agent Profile**:
  - **Category**: `quick`
    - Reason: Single file change, <10 lines modified, purely mechanical
  - **Skills**: `[]`
    - No special skills needed for simple Rust edit

  **Parallelization**:
  - **Can Run In Parallel**: YES
  - **Parallel Group**: Wave 1 (with Tasks 2, 3, 4)
  - **Blocks**: None
  - **Blocked By**: None

  **References**:
  - `christina-core/src/types/file_path.rs:24-32` - FilePath::new() with debug_assert
  - `christina-core/src/types/file_path.rs:127-131` - Test to update (remove cfg attribute)

  **Acceptance Criteria**:
  - [ ] `debug_assert!` replaced with `assert!` at line 26
  - [ ] Docstring no longer mentions "debug builds" or "release builds"
  - [ ] Test `filepath_absolute_panics` exists without `#[cfg(debug_assertions)]`
  - [ ] `cargo test -p christina-core filepath_absolute_panics` → PASS (test runs and panics as expected)
  - [ ] `just check` → zero warnings
  - [ ] `just clippy` → zero warnings

  **Agent-Executed QA Scenarios**:

  ```
  Scenario: Validation works in release build
    Tool: Bash
    Preconditions: Workspace compiles
    Steps:
      1. cargo build --release -p christina-core
      2. Verify build succeeds (assert! doesn't prevent compilation)
    Expected Result: Build succeeds
    Evidence: Build output captured

  Scenario: Test confirms panic behavior
    Tool: Bash
    Preconditions: None
    Steps:
      1. cargo test -p christina-core filepath_absolute_panics -- --nocapture
      2. Assert: Output shows test passed (panic caught by should_panic)
    Expected Result: Test passes
    Evidence: Test output showing "test ... ok"
  ```

  **Commit**: YES
  - Message: `fix(core): enforce FilePath relative path validation in all builds`
  - Files: `christina-core/src/types/file_path.rs`
  - Pre-commit: `just check && just clippy`

---

- [ ] 2. Config: Add schema_version field

  **What to do**:
  - Add `schema_version: u32` field to Config struct (after line 102, before closing brace)
  - Add `#[serde(default = "default_schema_version")]` attribute
  - Add `fn default_schema_version() -> u32 { 1 }` helper function
  - Set `schema_version: default_schema_version()` in Default impl
  - Do NOT add migration logic yet (just the field for now)

  **Must NOT do**:
  - Do NOT add `#[serde(skip_serializing)]` - version should persist
  - Do NOT add complex migration logic in this task
  - Do NOT change existing field ordering

  **Recommended Agent Profile**:
  - **Category**: `quick`
    - Reason: Adding one field to struct, straightforward serde pattern
  - **Skills**: `[]`
    - Standard Rust/serde, no special skills

  **Parallelization**:
  - **Can Run In Parallel**: YES
  - **Parallel Group**: Wave 1 (with Tasks 1, 3, 4)
  - **Blocks**: None
  - **Blocked By**: None

  **References**:
  - `christina/src/config/settings.rs:25-102` - Config struct definition
  - `christina/src/config/settings.rs:104-136` - Default impl to update

  **Acceptance Criteria**:
  - [ ] `schema_version: u32` field exists in Config struct
  - [ ] Field has `#[serde(default = "default_schema_version")]`
  - [ ] `default_schema_version()` returns `1`
  - [ ] Default impl includes `schema_version: default_schema_version()`
  - [ ] Config loads from file without schema_version field (serde default kicks in)
  - [ ] Config saves with schema_version field
  - [ ] `just check` → zero warnings
  - [ ] `just clippy` → zero warnings

  **Agent-Executed QA Scenarios**:

  ```
  Scenario: Config loads without schema_version (backward compat)
    Tool: Bash
    Preconditions: None
    Steps:
      1. Create temp config without schema_version:
         echo '[diff]' > /tmp/test_config.toml
         echo 'enabled = true' >> /tmp/test_config.toml
      2. cargo test -p christina config -- --nocapture
      3. Assert: Tests pass (loading doesn't fail)
    Expected Result: Config loads with default version
    Evidence: Test output

  Scenario: Config struct compiles correctly
    Tool: Bash
    Preconditions: None
    Steps:
      1. just check
      2. Assert: Exit code 0, no warnings about schema_version
    Expected Result: Compiles without issues
    Evidence: Build output
  ```

  **Commit**: YES
  - Message: `feat(config): add schema_version field for future migrations`
  - Files: `christina/src/config/settings.rs`
  - Pre-commit: `just check && just clippy`

---

- [ ] 3. Tracing: Initialize subscriber with file appender

  **What to do**:
  - Add `tracing-subscriber` and `tracing-appender` to christina/Cargo.toml dependencies
  - Create `fn init_tracing(verbose: u8)` in main.rs (before main function)
  - Map verbose levels: 0→INFO, 1→DEBUG, 2+→TRACE
  - Use `tracing_appender::rolling::daily()` for TUI-safe file logging
  - Log directory: `~/.local/share/christina/logs/` (via directories crate)
  - Call `init_tracing(cli.verbose)` immediately after `Cli::parse()` in main()
  - For non-TUI mode (CLI commit), also output to stderr

  **Must NOT do**:
  - Do NOT log to stdout in TUI mode (corrupts terminal)
  - Do NOT make tracing mandatory (graceful fallback if dir creation fails)
  - Do NOT add complex filtering logic

  **Recommended Agent Profile**:
  - **Category**: `quick`
    - Reason: Standard tracing setup pattern, well-documented
  - **Skills**: `[]`
    - Standard tracing-subscriber usage

  **Parallelization**:
  - **Can Run In Parallel**: YES
  - **Parallel Group**: Wave 1 (with Tasks 1, 2, 4)
  - **Blocks**: Task 6 (Secret warning needs tracing to work)
  - **Blocked By**: None

  **References**:
  - `christina/src/main.rs:47-71` - main() function where init goes
  - `christina/src/cli.rs:14-16` - verbose flag definition
  - `christina/Cargo.toml:40` - existing tracing dependency
  - `christina/src/generate.rs:6` - existing tracing::{info, warn} usage

  **Acceptance Criteria**:
  - [ ] `tracing-subscriber` and `tracing-appender` in Cargo.toml
  - [ ] `init_tracing(verbose: u8)` function exists
  - [ ] Verbose 0 → LevelFilter::INFO
  - [ ] Verbose 1 → LevelFilter::DEBUG
  - [ ] Verbose 2+ → LevelFilter::TRACE
  - [ ] Log file created at `~/.local/share/christina/logs/christina.YYYY-MM-DD.log`
  - [ ] `just check` → zero warnings
  - [ ] `just clippy` → zero warnings

  **Agent-Executed QA Scenarios**:

  ```
  Scenario: Tracing produces log file with verbose flag
    Tool: Bash
    Preconditions: Built binary
    Steps:
      1. cargo build -p christina
      2. rm -rf ~/.local/share/christina/logs/
      3. ./target/debug/christina -v --help 2>/dev/null || true
      4. ls ~/.local/share/christina/logs/
      5. Assert: Log file exists with today's date
    Expected Result: Log file created
    Evidence: ls output showing log file

  Scenario: Tracing level changes with -vv
    Tool: Bash
    Preconditions: Log directory exists
    Steps:
      1. ./target/debug/christina -vv --help 2>/dev/null || true
      2. cat ~/.local/share/christina/logs/christina.*.log | head -20
      3. Assert: Contains DEBUG or TRACE level entries
    Expected Result: Higher verbosity produces more output
    Evidence: Log file content
  ```

  **Commit**: YES
  - Message: `feat(cli): initialize tracing subscriber with file appender`
  - Files: `christina/Cargo.toml`, `christina/src/main.rs`
  - Pre-commit: `just check && just clippy`

---

- [ ] 4. TokenBudget: Add upfront validation

  **What to do**:
  - Add `pub fn try_new(...) -> Result<Self, String>` to TokenBudget
  - Move validation logic from `remaining_for_diff()` into `try_new()`
  - Keep `new()` but have it call `try_new().expect("invalid budget")` for convenience constructors
  - Update `small()`, `medium()`, `large()`, `massive()` to use `new()` (they're known-valid)
  - Update generate.rs to use `try_new()` instead of `new()` + `remaining_for_diff()` validation

  **Must NOT do**:
  - Do NOT remove `remaining_for_diff()` - it's still useful for calculating available space
  - Do NOT change the validation logic itself, just move it earlier
  - Do NOT change TokenBudget fields

  **Recommended Agent Profile**:
  - **Category**: `quick`
    - Reason: Moving existing code, not writing new logic
  - **Skills**: `[]`
    - Standard Rust refactoring

  **Parallelization**:
  - **Can Run In Parallel**: YES
  - **Parallel Group**: Wave 1 (with Tasks 1, 2, 3)
  - **Blocks**: Task 7 (TokenizerService caching builds on this pattern)
  - **Blocked By**: None

  **References**:
  - `christina/src/io/llm/tokenizer.rs:176-189` - TokenBudget::new()
  - `christina/src/io/llm/tokenizer.rs:231-253` - remaining_for_diff() with validation
  - `christina/src/generate.rs:126-135` - Usage site that validates after new()

  **Acceptance Criteria**:
  - [ ] `TokenBudget::try_new()` exists and validates upfront
  - [ ] `TokenBudget::new()` calls `try_new().expect()`
  - [ ] generate.rs uses `try_new()` directly
  - [ ] Existing tests still pass
  - [ ] Invalid budget configurations fail at construction, not at remaining_for_diff()
  - [ ] `just check` → zero warnings
  - [ ] `just clippy` → zero warnings

  **Agent-Executed QA Scenarios**:

  ```
  Scenario: Invalid budget fails at construction
    Tool: Bash
    Preconditions: None
    Steps:
      1. cargo test -p christina token_budget_invalid -- --nocapture
      2. Assert: Test passes (error returned from try_new or panic from new)
    Expected Result: Invalid configs caught early
    Evidence: Test output

  Scenario: Valid budgets construct successfully
    Tool: Bash
    Preconditions: None
    Steps:
      1. cargo test -p christina token_budget -- --nocapture
      2. Assert: All budget tests pass
    Expected Result: Valid configs work
    Evidence: Test output showing passing tests
  ```

  **Commit**: YES
  - Message: `fix(tokenizer): validate TokenBudget at construction time`
  - Files: `christina/src/io/llm/tokenizer.rs`, `christina/src/generate.rs`
  - Pre-commit: `just check && just clippy`

---

### P1 - SHOULD HAVE (Release Quality)

- [ ] 5. CLI: Add shell completions subcommand

  **What to do**:
  - Add `clap_complete` to christina/Cargo.toml
  - Add `Completions` variant to Commands enum in cli.rs
  - Add `CompletionCommands` subcommand struct with shell selection
  - Implement handler in main.rs that generates completion script to stdout
  - Support: bash, zsh, fish, powershell, elvish

  **Must NOT do**:
  - Do NOT install completions automatically
  - Do NOT write to files (output to stdout, user redirects)
  - Do NOT add complex shell detection

  **Recommended Agent Profile**:
  - **Category**: `quick`
    - Reason: Standard clap_complete pattern, well-documented
  - **Skills**: `[]`
    - Standard clap usage

  **Parallelization**:
  - **Can Run In Parallel**: YES
  - **Parallel Group**: Wave 2 (with Tasks 6, 7)
  - **Blocks**: Task 8 (--dry-run builds on CLI structure)
  - **Blocked By**: None (logically after Wave 1 but no hard dependency)

  **References**:
  - `christina/src/cli.rs:31-39` - Commands enum to extend
  - `christina/src/main.rs:54-70` - Command handling in main()
  - clap_complete docs: https://docs.rs/clap_complete/latest/clap_complete/

  **Acceptance Criteria**:
  - [ ] `clap_complete` in Cargo.toml
  - [ ] `christina completions bash` outputs valid bash completion
  - [ ] `christina completions zsh` outputs valid zsh completion
  - [ ] `christina completions fish` outputs valid fish completion
  - [ ] `just check` → zero warnings
  - [ ] `just clippy` → zero warnings

  **Agent-Executed QA Scenarios**:

  ```
  Scenario: Generate bash completions
    Tool: Bash
    Preconditions: Binary built
    Steps:
      1. cargo build -p christina
      2. ./target/debug/christina completions bash > /tmp/christina.bash
      3. bash -n /tmp/christina.bash  # syntax check
      4. Assert: Exit code 0 (valid bash)
    Expected Result: Valid bash completion script
    Evidence: Syntax check passes

  Scenario: Generate zsh completions
    Tool: Bash
    Preconditions: Binary built
    Steps:
      1. ./target/debug/christina completions zsh > /tmp/_christina
      2. head -5 /tmp/_christina
      3. Assert: Contains #compdef christina
    Expected Result: Valid zsh completion
    Evidence: File content
  ```

  **Commit**: YES
  - Message: `feat(cli): add shell completion generation command`
  - Files: `christina/Cargo.toml`, `christina/src/cli.rs`, `christina/src/main.rs`
  - Pre-commit: `just check && just clippy`

---

- [ ] 6. Secret: Warn on plaintext API keys

  **What to do**:
  - Add `tracing::warn!` in `SecretRef::parse()` when falling through to Literal
  - Warning message: "API key stored as plaintext. Consider using env:VAR_NAME or keyring:KEY_NAME for better security."
  - Only warn if the literal looks like an API key (length > 20, no spaces)

  **Must NOT do**:
  - Do NOT block parsing (still return Ok)
  - Do NOT warn for short strings (likely not API keys)
  - Do NOT warn for strings that look like env var names
  - Do NOT add new dependencies

  **Recommended Agent Profile**:
  - **Category**: `quick`
    - Reason: Single tracing::warn! call with simple heuristic
  - **Skills**: `[]`
    - Standard Rust

  **Parallelization**:
  - **Can Run In Parallel**: YES
  - **Parallel Group**: Wave 2 (with Tasks 5, 7)
  - **Blocks**: None
  - **Blocked By**: Task 3 (tracing must be initialized for warn! to work)

  **References**:
  - `christina-core/src/config/secret.rs:108-119` - SecretRef::parse()
  - `christina-core/src/config/secret.rs:115-118` - Literal fallthrough branch

  **Acceptance Criteria**:
  - [ ] `tracing::warn!` added in Literal branch
  - [ ] Warning only triggers for strings > 20 chars without spaces
  - [ ] Parsing still succeeds (returns Ok)
  - [ ] `just check` → zero warnings
  - [ ] `just clippy` → zero warnings

  **Agent-Executed QA Scenarios**:

  ```
  Scenario: Warning appears for long plaintext string
    Tool: Bash
    Preconditions: Tracing initialized
    Steps:
      1. cargo test -p christina-core secret_ref_parse_literal -- --nocapture
      2. Check test output for warning presence (may not show in test)
    Expected Result: Test passes, warning logic exists
    Evidence: Test output

  Scenario: No warning for short strings
    Tool: Bash
    Preconditions: None
    Steps:
      1. Verify code: short strings (<=20 chars) don't trigger warn
      2. cargo test -p christina-core secret -- --nocapture
    Expected Result: All secret tests pass
    Evidence: Test output
  ```

  **Commit**: YES
  - Message: `feat(security): warn when API key stored as plaintext`
  - Files: `christina-core/src/config/secret.rs`
  - Pre-commit: `just check && just clippy`

---

- [ ] 7. TokenizerService: Add global caching

  **What to do**:
  - Move `#[cfg(test)]` from `static TOKENIZER: OnceLock<TokenizerService>` to make it always available
  - Change type to `OnceLock<Arc<TokenizerService>>` for thread-safe sharing
  - Update `get_tokenizer()` to return `Result<Arc<TokenizerService>>`
  - Update generate.rs line 120 to use `get_tokenizer()?` instead of `TokenizerService::new()?`
  - Fix potential race in `count_tokens()`: re-check cache after re-acquiring lock before insert

  **Must NOT do**:
  - Do NOT remove TokenizerService::new() - keep it for direct instantiation
  - Do NOT change the Tokenizer trait impl
  - Do NOT add complex lazy initialization

  **Recommended Agent Profile**:
  - **Category**: `quick`
    - Reason: Standard OnceLock pattern, moving cfg attribute
  - **Skills**: `[]`
    - Standard Rust concurrency patterns

  **Parallelization**:
  - **Can Run In Parallel**: YES
  - **Parallel Group**: Wave 2 (with Tasks 5, 6)
  - **Blocks**: None
  - **Blocked By**: Task 4 (consistent tokenizer patterns)

  **References**:
  - `christina/src/io/llm/tokenizer.rs:3-4` - OnceLock import with cfg(test)
  - `christina/src/io/llm/tokenizer.rs:16-17` - TOKENIZER static with cfg(test)
  - `christina/src/io/llm/tokenizer.rs:23-42` - get_tokenizer() with cfg(test)
  - `christina/src/generate.rs:120` - TokenizerService::new() call site

  **Acceptance Criteria**:
  - [ ] `TOKENIZER` static available in all builds (no cfg(test))
  - [ ] Type is `OnceLock<Arc<TokenizerService>>`
  - [ ] `get_tokenizer()` returns `Result<Arc<TokenizerService>>`
  - [ ] generate.rs uses `get_tokenizer()?`
  - [ ] Race condition in count_tokens fixed (re-check after re-lock)
  - [ ] `just check` → zero warnings
  - [ ] `just clippy` → zero warnings

  **Agent-Executed QA Scenarios**:

  ```
  Scenario: TokenizerService reused across calls
    Tool: Bash
    Preconditions: None
    Steps:
      1. cargo test -p christina tokenizer -- --nocapture
      2. Assert: Tests pass, no "Failed to load" errors
    Expected Result: Global tokenizer works
    Evidence: Test output

  Scenario: generate.rs compiles with new signature
    Tool: Bash
    Preconditions: None
    Steps:
      1. just check
      2. Assert: No errors about get_tokenizer()
    Expected Result: Type compatibility maintained
    Evidence: Build output
  ```

  **Commit**: YES
  - Message: `perf(tokenizer): cache TokenizerService globally to avoid repeated BPE loading`
  - Files: `christina/src/io/llm/tokenizer.rs`, `christina/src/generate.rs`
  - Pre-commit: `just check && just clippy`

---

- [ ] 8. CLI: Add --dry-run flag

  **What to do**:
  - Add `#[arg(long)]` `dry_run: bool` to Cli struct
  - Pass dry_run to `cli::commit::run()`
  - In commit.rs run(), if dry_run: skip `execute_commit()`, print message, return Ok
  - Print clear indication: "Dry run mode - commit NOT created"

  **Must NOT do**:
  - Do NOT add dry-run to TUI mode (TUI already previews before commit)
  - Do NOT change the generation flow
  - Do NOT skip validation steps

  **Recommended Agent Profile**:
  - **Category**: `quick`
    - Reason: Single flag, simple conditional
  - **Skills**: `[]`
    - Standard clap/CLI pattern

  **Parallelization**:
  - **Can Run In Parallel**: NO
  - **Parallel Group**: Wave 3 (final task)
  - **Blocks**: None
  - **Blocked By**: Tasks 3, 5 (tracing for potential logging, CLI structure)

  **References**:
  - `christina/src/cli.rs:13-29` - Cli struct
  - `christina/src/main.rs:67` - commit::run() call
  - `christina/src/cli/commit.rs:14-63` - run() function
  - `christina/src/cli/commit.rs:154-160` - execute_commit()

  **Acceptance Criteria**:
  - [ ] `--dry-run` flag in Cli struct
  - [ ] `christina --dry-run` generates message but doesn't commit
  - [ ] Clear output indicating dry run mode
  - [ ] Without --dry-run, behavior unchanged
  - [ ] `just check` → zero warnings
  - [ ] `just clippy` → zero warnings

  **Agent-Executed QA Scenarios**:

  ```
  Scenario: dry-run shows message without committing
    Tool: Bash
    Preconditions: Git repo with staged changes, API key configured
    Steps:
      1. cd /tmp && git init test-repo && cd test-repo
      2. echo "test" > file.txt && git add file.txt
      3. Set up minimal config with API key
      4. cargo run -p christina -- --dry-run 2>&1 || true
      5. git log --oneline | wc -l
      6. Assert: No commits created (count is 0)
    Expected Result: Message generated, no commit
    Evidence: git log output

  Scenario: Help shows dry-run flag
    Tool: Bash
    Preconditions: Binary built
    Steps:
      1. ./target/debug/christina --help
      2. Assert: Output contains "--dry-run"
    Expected Result: Flag documented
    Evidence: Help output
  ```

  **Commit**: YES
  - Message: `feat(cli): add --dry-run flag to preview commit without creating`
  - Files: `christina/src/cli.rs`, `christina/src/main.rs`, `christina/src/cli/commit.rs`
  - Pre-commit: `just check && just clippy`

---

## Commit Strategy

| After Task | Message | Files | Verification |
|------------|---------|-------|--------------|
| 1 | `fix(core): enforce FilePath relative path validation in all builds` | file_path.rs | just check && just clippy |
| 2 | `feat(config): add schema_version field for future migrations` | settings.rs | just check && just clippy |
| 3 | `feat(cli): initialize tracing subscriber with file appender` | Cargo.toml, main.rs | just check && just clippy |
| 4 | `fix(tokenizer): validate TokenBudget at construction time` | tokenizer.rs, generate.rs | just check && just clippy |
| 5 | `feat(cli): add shell completion generation command` | Cargo.toml, cli.rs, main.rs | just check && just clippy |
| 6 | `feat(security): warn when API key stored as plaintext` | secret.rs | just check && just clippy |
| 7 | `perf(tokenizer): cache TokenizerService globally to avoid repeated BPE loading` | tokenizer.rs, generate.rs | just check && just clippy |
| 8 | `feat(cli): add --dry-run flag to preview commit without creating` | cli.rs, main.rs, commit.rs | just check && just clippy |

---

## Success Criteria

### Verification Commands
```bash
# All must pass
just check        # Expected: zero warnings
just clippy       # Expected: zero warnings
cargo test        # Expected: all tests pass

# Feature verification
./target/debug/christina --help          # Expected: shows --dry-run, completions
./target/debug/christina completions bash # Expected: valid bash script
ls ~/.local/share/christina/logs/         # Expected: log files after -v usage
```

### Final Checklist
- [ ] All P0 items fixed (1-4)
- [ ] All P1 items implemented (5-8)
- [ ] All tests pass
- [ ] Zero clippy warnings
- [ ] Zero compiler warnings
- [ ] No new unsafe code introduced
- [ ] No documentation changes (per guardrails)
