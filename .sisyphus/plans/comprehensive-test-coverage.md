# Comprehensive Test Coverage Work Plan

## TL;DR

> **Objective**: Bring the christina codebase to high, meaningful test coverage by adding tests to 17 untested modules.
>
> **Deliverables**: 
> - Test helper infrastructure (temp git repos, deterministic Tokenizer, mock stdin)
> - 120+ new unit and integration tests
> - Refactored modules for testability (remove process::exit, add trait abstractions)
> - All tests pass quality gates (`just check`, `just clippy`, `just test`)
>
> **Estimated Effort**: Large (15-20 hours of focused work)
> **Parallel Execution**: YES - 4 waves with 3-5 tasks each
> **Critical Path**: Wave 1 (foundations) → Wave 3 (git/chunking) → Wave 4 (orchestration)

---

## Context

### Original Request
Bring a Rust workspace with 93 source files to very high, meaningful test coverage. Currently 165 tests pass, but 30+ files lack tests entirely.

### Research Findings

**Existing Test Infrastructure:**
- Tests are **inline** using `#[cfg(test)] mod tests` pattern
- **No external mocking frameworks** - uses manual mocks (Provider::Mock pattern)
- **Async tests** use `#[tokio::test]` with optional `start_paused = true`
- **cargo nextest** for test execution
- Quality gates: `just check`, `just clippy`, `just test`

**Key Testing Patterns from Codebase:**
1. Manual mock variants in enums: `Provider::Mock`, `Provider::MockSequence`
2. Test helpers defined inline in test modules
3. Error assertions check message substrings: `assert!(err.to_string().contains("..."))`
4. Async tests control time with `tokio::time::pause()` when needed

**Testability Challenges Identified:**
1. `process::exit` calls in CLI handlers abort test runner
2. Git operations tightly coupled to git2 crate
3. Tokenizer trait needs deterministic stub for chunking tests
4. Async orchestration needs dependency injection
5. Terminal input/output hard to mock

---

## Work Objectives

### Core Objective
Achieve comprehensive test coverage for all non-trivial logic across 17 untested modules, prioritizing core domain logic and business-critical paths.

### Concrete Deliverables
1. **Test Infrastructure** (`christina-core/src/test_helpers.rs` or inline)
   - `TempRepo` builder for git integration tests
   - `DeterministicTokenizer` stub implementing Tokenizer trait
   - `MockStdin` for CLI confirmation prompts
   - `TempConfig` constructor for isolated config tests

2. **Unit Tests** (Pure logic, no I/O)
   - Config serialization/deserialization
   - CLI argument parsing
   - DiffTool parsing and Display
   - GenerationId newtype
   - determine_initial_state logic
   - format_error_message error mapping

3. **Integration Tests** (With real git repos)
   - Git adapter operations (stage, unstage, commit, diff)
   - AppContextData branch operations
   - Config persistence

4. **Refactored Modules** (For testability)
   - Replace `process::exit` with `Result` returns
   - Extract GitRepository trait from adapter functions
   - Inject orchestrator/provider into generate.rs
   - Abstract input source in producers.rs

### Definition of Done
- [ ] All 17 modules have meaningful test coverage
- [ ] `cargo nextest run` passes with 0 failures
- [ ] `just check` and `just clippy` pass with 0 warnings
- [ ] All tests complete in <1 second total
- [ ] No test coverage for trivial getters/setters (coverage for coverage's sake)

### Must Have
- Tests for all error paths and edge cases
- Property-based tests for chunking algorithm invariants
- Integration tests for git operations with temp repos
- Async tests for generation orchestration with mocked providers

### Must NOT Have (Guardrails)
- Tests for trivial getters/setters
- Tests that hit real git repos in user's home directory
- Tests that make real network calls
- Tests that depend on specific terminal environments
- Flaky tests that depend on timing

---

## Verification Strategy

### Test Decision
- **Infrastructure exists**: YES (cargo test, nextest)
- **User wants tests**: YES (TDD-style: test-first for new logic, test-after for existing)
- **Framework**: Built-in `cargo test` + `tokio::test` for async

### Test Organization
Following existing codebase patterns:
- **Inline tests**: `#[cfg(test)] mod tests` at bottom of each source file
- **Test helpers**: Defined in same test module or shared via `test_helpers` module
- **No separate tests/ directory** for unit tests (integration tests may use tests/)

### Automated Verification

**For Unit Tests:**
```bash
# Run specific module tests
cargo nextest run -- config::config_file
cargo nextest run -- cli::tests

# Run all tests
cargo nextest run

# With coverage (optional)
cargo tarpaulin --workspace --timeout 120
```

**For Integration Tests:**
```bash
# Integration tests use temp directories, no setup needed
cargo nextest run --test '*'
```

**Quality Gates:**
```bash
just check   # Must pass with 0 warnings
just clippy  # Must pass with 0 warnings  
just test    # Must pass with 0 failures
```

---

## Execution Strategy

### Parallel Execution Waves

```
Wave 1 (Foundations - Start Immediately):
├── Task 1: Test helpers infrastructure
├── Task 2: Core config modules (config_file, resolved, ids, snapshot)
├── Task 3: CLI parsing (cli.rs, diff_tool.rs)
└── Task 4: Pure logic (init.rs partial, handlers.rs format_error)

Wave 2 (State & Context - After Wave 1):
├── Task 5: App state modules (context.rs, state.rs)
├── Task 6: Config CLI handlers (cli.rs with mocks)
└── Task 7: Event loop handlers (full coverage)

Wave 3 (Git & Chunking - After Wave 2):
├── Task 8: Git adapter trait abstraction + tests
├── Task 9: Chunking algorithm tests (unit + property)
└── Task 10: Profile CLI refactor + tests

Wave 4 (Orchestration - After Wave 3):
├── Task 11: Generate.rs refactor + tests
└── Task 12: Event producers refactor + tests

Critical Path: Task 1 → Task 8 → Task 11
Parallel Speedup: ~50% faster than sequential
```

### Dependency Matrix

| Task | Depends On | Blocks | Can Parallelize With |
|------|------------|--------|---------------------|
| 1 (Test Helpers) | None | 5, 8, 9, 10, 11, 12 | 2, 3, 4 |
| 2 (Core Config) | None | None | 1, 3, 4 |
| 3 (CLI Parsing) | None | None | 1, 2, 4 |
| 4 (Pure Logic) | None | 7 | 1, 2, 3 |
| 5 (App State) | 1 | None | 6, 7 |
| 6 (Config CLI) | 1 | None | 5, 7 |
| 7 (Event Handlers) | 4 | None | 5, 6 |
| 8 (Git Adapter) | 1 | 11 | 9, 10 |
| 9 (Chunking) | 1 | None | 8, 10 |
| 10 (Profile CLI) | 1 | None | 8, 9 |
| 11 (Generate) | 1, 8 | None | 12 |
| 12 (Producers) | 1 | None | 11 |

### Agent Dispatch Summary

| Wave | Tasks | Recommended Agent Profile |
|------|-------|--------------------------|
| 1 | 1-4 | `quick` or `unspecified-high` - straightforward unit tests |
| 2 | 5-7 | `unspecified-high` - requires test helpers, moderate complexity |
| 3 | 8-10 | `unspecified-high` or `ultrabrain` - complex refactoring + tests |
| 4 | 11-12 | `ultrabrain` - complex async orchestration, trait design |

---

## TODOs

### Wave 1: Foundations

- [ ] **1. Create Test Helper Infrastructure**

  **What to do:**
  - Create `christina-core/src/test_helpers.rs` (or inline in lib.rs)
  - Implement `TempRepo` struct using `tempfile::TempDir` + `git2::Repository::init`
  - Implement `DeterministicTokenizer` implementing Tokenizer trait (counts by whitespace)
  - Implement `MockStdin` for simulating user input in CLI tests
  - Implement `temp_config()` function creating isolated Config

  **Must NOT do:**
  - Add external mocking crates (mockall, mockito)
  - Create global test state
  - Depend on user's actual git config or home directory

  **Recommended Agent Profile:**
  - **Category**: `unspecified-high`
  - **Skills**: rust, testing-patterns
  - **Justification**: Requires understanding of git2, tempfile, and existing Tokenizer trait

  **Parallelization:**
  - **Can Run In Parallel**: YES
  - **Parallel Group**: Wave 1
  - **Blocks**: Tasks 5, 8, 9, 10, 11, 12 (all need test helpers)
  - **Blocked By**: None

  **References:**
  - `christina-core/src/tokenizer.rs` - Tokenizer trait definition
  - `christina/src/io/llm/provider.rs:Provider::Mock` - Manual mock pattern
  - `tempfile` crate docs for TempDir
  - `git2::Repository::init` for temp repo creation

  **Acceptance Criteria:**
  - [ ] `TempRepo::new()` creates temp dir with initialized git repo
  - [ ] `TempRepo::commit_file(path, content)` creates commit and returns oid
  - [ ] `DeterministicTokenizer` implements Tokenizer trait deterministically
  - [ ] `MockStdin::new(inputs: Vec<String>)` simulates stdin lines
  - [ ] `temp_config()` returns Config backed by temp file
  - [ ] All helpers compile with `#[cfg(test)]` guard
  - [ ] Example test using each helper passes

  **Commit**: YES
  - Message: `test: add test helper infrastructure`
  - Files: `christina-core/src/test_helpers.rs`, `christina-core/src/lib.rs`
  - Pre-commit: `cargo nextest run -- test_helpers`

---

- [ ] **2. Test Core Config Modules**

  **What to do:**
  - Add tests to `christina-core/src/config/config_file.rs`
  - Add tests to `christina-core/src/config/resolved.rs`
  - Add tests to `christina-core/src/ids.rs`
  - Add tests to `christina-core/src/git/snapshot.rs`

  **Specific test cases:**

  **config_file.rs:**
  - `test_default_values()` - verify Default impl sets expected values
  - `test_serde_roundtrip()` - serialize then deserialize, verify equality
  - `test_optional_fields()` - None fields excluded from serialization
  - `test_ignore_files_default()` - verify default ignore patterns

  **resolved.rs:**
  - `test_get_active_profile()` - returns Some when active_profile set and exists
  - `test_get_active_profile_missing()` - returns None when active_profile not set
  - `test_get_active_profile_not_found()` - returns None when profile name not in map
  - `test_get_profile()` - returns correct profile by name
  - `test_default_values()` - verify default field values

  **ids.rs:**
  - `test_generation_id_new()` - preserves value
  - `test_generation_id_traits()` - Copy, Eq, Debug work correctly
  - `test_generation_id_display()` - if Display implemented

  **snapshot.rs:**
  - `test_repo_snapshot_clone()` - Clone works
  - `test_repo_snapshot_debug()` - Debug output reasonable
  - (Optional: only if downstream logic depends on invariants)

  **Must NOT do:**
  - Test trivial struct field access
  - Add integration tests (these are pure data types)

  **Recommended Agent Profile:**
  - **Category**: `quick`
  - **Skills**: rust
  - **Justification**: Straightforward unit tests for data types

  **Parallelization:**
  - **Can Run In Parallel**: YES
  - **Parallel Group**: Wave 1
  - **Blocks**: None
  - **Blocked By**: None

  **References:**
  - `christina-core/src/types/provider_kind.rs` - Example of enum tests
  - `christina-core/src/types/model_name.rs` - Example of newtype tests
  - `christina-core/src/git/file.rs` - Example of struct tests

  **Acceptance Criteria:**
  - [ ] Each file has `#[cfg(test)] mod tests` with tests
  - [ ] `cargo nextest run -- config::config_file` passes
  - [ ] `cargo nextest run -- config::resolved` passes
  - [ ] `cargo nextest run -- ids` passes
  - [ ] `cargo nextest run -- git::snapshot` passes
  - [ ] All tests run in <100ms total

  **Commit**: YES (can group with Task 3)
  - Message: `test: add unit tests for core config types`
  - Files: `christina-core/src/config/config_file.rs`, `christina-core/src/config/resolved.rs`, `christina-core/src/ids.rs`, `christina-core/src/git/snapshot.rs`

---

- [ ] **3. Test CLI Parsing**

  **What to do:**
  - Add tests to `christina/src/cli.rs` for Clap parsing
  - Add tests to `christina/src/config/diff_tool.rs` for DiffTool parsing

  **Specific test cases:**

  **cli.rs:**
  - `test_cli_default_config()` - default config path is "config.toml"
  - `test_cli_verbose_count()` - -v, -vv, -vvv parsed correctly
  - `test_subcommand_config_get()` - `config get <key>` parses correctly
  - `test_subcommand_config_set()` - `config set <key> <value>` parses correctly
  - `test_subcommand_profile_create()` - profile create with all args
  - `test_invalid_args()` - unknown flags produce error

  **diff_tool.rs:**
  - `test_difftool_from_str_valid()` - all variants parse case-insensitively
  - `test_difftool_from_str_invalid()` - error includes valid options
  - `test_difftool_display()` - Display outputs expected strings
  - `test_difftool_from_env()` - env var parsing works
  - `test_difftool_with_env_override()` - override respects both vars
  - `test_difftool_aliases()` - "diffsofancy" parses to DiffSoFancy

  **Must NOT do:**
  - Test the actual command execution (that's for integration tests)
  - Test help text output (too brittle)

  **Recommended Agent Profile:**
  - **Category**: `quick`
  - **Skills**: rust
  - **Justification**: Pure parsing logic, straightforward assertions

  **Parallelization:**
  - **Can Run In Parallel**: YES
  - **Parallel Group**: Wave 1
  - **Blocks**: None
  - **Blocked By**: None

  **References:**
  - `christina/src/cli.rs` - Cli struct with clap derives
  - `christina/src/config/diff_tool.rs` - DiffTool enum
  - Clap docs for `try_parse_from`

  **Acceptance Criteria:**
  - [ ] `cargo nextest run -- cli::tests` passes
  - [ ] `cargo nextest run -- config::diff_tool` passes
  - [ ] All valid CLI combinations parse correctly
  - [ ] Invalid inputs produce errors

  **Commit**: YES (group with Task 2)
  - Message: `test: add CLI parsing tests`
  - Files: `christina/src/cli.rs`, `christina/src/config/diff_tool.rs`

---

- [ ] **4. Test Pure Logic Functions**

  **What to do:**
  - Add tests to `christina/src/app/init.rs` for `determine_initial_state`
  - Add tests to `christina/src/event_loop/handlers.rs` for `format_error_message`
  - Add tests to `christina/src/event_loop/handlers.rs` for event handlers

  **Specific test cases:**

  **init.rs - determine_initial_state:**
  - `test_empty_repo()` - staged empty, unstaged empty → StagingSelection + warning
  - `test_staged_only()` - staged has processable file, unstaged empty → Dashboard
  - `test_unstaged_only()` - staged empty, unstaged non-empty → StagingSelection
  - `test_staged_binary_only()` - staged only binary files → StagingSelection + warning
  - `test_staged_empty_diff()` - staged files with no diff content → StagingSelection + warning
  - `test_mixed_staged_unstaged()` - both present → Dashboard

  **handlers.rs - format_error_message:**
  - `test_format_unauthorized()` - CompletionError::Unauthorized → specific message
  - `test_format_rate_limited()` - CompletionError::RateLimited → specific message
  - `test_format_server_error()` - CompletionError::ServerError(msg) → includes msg
  - `test_format_other_error()` - non-CompletionError → err.to_string()

  **handlers.rs - event handlers:**
  - `test_handle_tick_increments_frame()` - frame_count increases
  - `test_handle_generation_complete_matching_id()` - state updates when id matches
  - `test_handle_generation_complete_mismatched_id()` - no change when id differs
  - `test_handle_generation_error_matching_id()` - error_message set, state changes
  - `test_handle_generation_progress_matching_id()` - progress updated

  **Must NOT do:**
  - Test init_context (needs git repo - integration test)
  - Test load_file_lists (needs git adapter - integration test)
  - Test actual TUI state transitions (tested elsewhere)

  **Recommended Agent Profile:**
  - **Category**: `unspecified-high`
  - **Skills**: rust, testing-patterns
  - **Justification**: Requires understanding state machine and error types

  **Parallelization:**
  - **Can Run In Parallel**: YES
  - **Parallel Group**: Wave 1
  - **Blocks**: Task 7 (builds on these patterns)
  - **Blocked By**: None

  **References:**
  - `christina/src/app/init.rs` - determine_initial_state function
  - `christina/src/event_loop/handlers.rs` - handler functions
  - `christina-core/src/error.rs` - CompletionError variants
  - `christina-core/src/state.rs` - AppState enum

  **Acceptance Criteria:**
  - [ ] `determine_initial_state` all branches covered
  - [ ] `format_error_message` all CompletionError variants covered
  - [ ] Event handler id-matching logic tested
  - [ ] All tests pass with `cargo nextest run`

  **Commit**: YES
  - Message: `test: add tests for pure logic functions`
  - Files: `christina/src/app/init.rs`, `christina/src/event_loop/handlers.rs`

---

### Wave 2: State & Context

- [ ] **5. Test App State Modules**

  **What to do:**
  - Add tests to `christina/src/app/context.rs` for AppContextData
  - Add tests to `christina/src/app/state.rs` for AbortOnDrop and state types

  **Specific test cases:**

  **context.rs:**
  - `test_refresh_branch_with_branch()` - temp repo with branch, refresh_branch updates branch_name
  - `test_refresh_branch_detached_head()` - detached HEAD → branch_name is None
  - `test_refresh_branch_no_repo()` - repo is None → no panic

  **state.rs:**
  - `test_abort_on_drop_aborts_task()` - spawn task, wrap in AbortOnDrop, drop it, task is aborted
  - `test_generation_state_idle()` - Idle variant exists
  - `test_generation_state_running()` - Running variant stores task and generation_id
  - `test_tui_ui_state_default()` - default values correct
  - `test_tui_session_data_default()` - default values correct

  **Must NOT do:**
  - Test actual TUI rendering (integration test territory)
  - Test async task behavior beyond abortion

  **Recommended Agent Profile:**
  - **Category**: `unspecified-high`
  - **Skills**: rust, tokio
  - **Justification**: Requires tokio runtime and temp git repos

  **Parallelization:**
  - **Can Run In Parallel**: YES
  - **Parallel Group**: Wave 2
  - **Blocks**: None
  - **Blocked By**: Task 1 (needs TempRepo)

  **References:**
  - `christina/src/app/context.rs` - AppContextData
  - `christina/src/app/state.rs` - AbortOnDrop, GenerationState
  - Test helpers from Task 1

  **Acceptance Criteria:**
  - [ ] `refresh_branch` tested with real temp repo
  - [ ] `AbortOnDrop` abortion verified via tokio::test
  - [ ] All state types have basic coverage

  **Commit**: YES (group with Task 6)
  - Message: `test: add app state tests`
  - Files: `christina/src/app/context.rs`, `christina/src/app/state.rs`

---

- [ ] **6. Test Config CLI Handlers**

  **What to do:**
  - Refactor `christina/src/config/cli.rs` to return Result instead of process::exit
  - Add tests for handle_config_command and helpers

  **Refactoring required:**
  - Replace `process::exit(code)` with `return Err(...)`
  - Add Config loader injection (function parameter or trait)
  - Return Result from all handle_* functions
  - Main CLI entry point handles exit codes

  **Specific test cases:**
  - `test_handle_get_existing_key()` - returns value
  - `test_handle_get_api_key_hidden()` - api_key value is hidden
  - `test_handle_get_missing_key()` - error for unknown key
  - `test_handle_set_updates_config()` - config updated and saved
  - `test_handle_list_shows_profiles()` - lists all profiles
  - `test_handle_path_shows_path()` - returns config path

  **Must NOT do:**
  - Test TUI mode (handle_tui) - requires terminal
  - Test profile management (delegates to profile_cli)

  **Recommended Agent Profile:**
  - **Category**: `unspecified-high`
  - **Skills**: rust, refactoring
  - **Justification**: Requires refactoring for testability

  **Parallelization:**
  - **Can Run In Parallel**: YES
  - **Parallel Group**: Wave 2
  - **Blocks**: None
  - **Blocked By**: Task 1 (needs temp_config)

  **References:**
  - `christina/src/config/cli.rs` - Current implementation with process::exit
  - `christina/src/config/mod.rs` - Config::load, Config::save_to_global

  **Acceptance Criteria:**
  - [ ] All `process::exit` calls replaced with Result returns
  - [ ] handle_get tested with temp config
  - [ ] handle_set tested with temp config
  - [ ] handle_list tested
  - [ ] handle_path tested
  - [ ] Main CLI still exits correctly (integration test)

  **Commit**: YES
  - Message: `refactor: make config CLI testable and add tests`
  - Files: `christina/src/config/cli.rs`
  - Pre-commit: Verify CLI still works: `cargo run -- config list`

---

- [ ] **7. Complete Event Loop Handler Tests**

  **What to do:**
  - Complete test coverage for `christina/src/event_loop/handlers.rs`
  - Test remaining handlers: handle_input, handle_generation_progress, handle_token_count_update

  **Specific test cases:**

  **handle_input:**
  - `test_handle_input_quit()` - 'q' key triggers quit
  - `test_handle_input_navigate()` - arrow keys navigate
  - `test_handle_input_confirm()` - Enter confirms
  - `test_handle_input_cancel()` - Esc cancels
  - `test_handle_input_edit()` - edit mode toggles

  **handle_generation_progress:**
  - `test_progress_updates_status()` - status message updated
  - `test_progress_matching_id_only()` - only updates if generation_id matches

  **handle_token_count_update:**
  - `test_token_count_updates()` - token count updated
  - `test_token_count_matching_id_only()` - only updates if generation_id matches

  **Must NOT do:**
  - Test actual crossterm input (integration test)
  - Test TUI rendering

  **Recommended Agent Profile:**
  - **Category**: `unspecified-high`
  - **Skills**: rust
  - **Justification**: Builds on Task 4 patterns

  **Parallelization:**
  - **Can Run In Parallel**: YES
  - **Parallel Group**: Wave 2
  - **Blocks**: None
  - **Blocked By**: Task 4 (establishes handler test patterns)

  **References:**
  - `christina/src/event_loop/handlers.rs` - All handler functions
  - `christina/src/app/mod.rs` - App struct fields

  **Acceptance Criteria:**
  - [ ] All handler functions have tests
  - [ ] generation_id matching logic covered
  - [ ] Input handling covered

  **Commit**: YES (group with Task 5)
  - Message: `test: complete event handler tests`
  - Files: `christina/src/event_loop/handlers.rs`

---

### Wave 3: Git & Chunking

- [ ] **8. Refactor Git Adapter + Add Tests**

  **What to do:**
  - Extract `GitRepository` trait from `christina/src/io/git/adapter.rs`
  - Implement trait for git2::Repository
  - Add comprehensive integration tests

  **Refactoring required:**
  ```rust
  pub trait GitRepository {
      fn get_staged_files(&self) -> Result<Vec<GitFile>>;
      fn get_unstaged_files(&self) -> Result<Vec<GitFile>>;
      fn stage_files(&self, paths: &[String]) -> Result<()>;
      fn unstage_files(&self, paths: &[String]) -> Result<()>;
      fn create_commit(&self, message: &str) -> Result<Oid>;
      fn has_staged_changes(&self) -> Result<bool>;
      fn validate_for_commit(&self) -> Result<()>;
      fn build_staged_diff(&self) -> Result<String>;
  }
  
  impl GitRepository for git2::Repository { ... }
  
  #[cfg(test)]
  pub struct MockGitRepository { ... }
  
  #[cfg(test)]
  impl GitRepository for MockGitRepository { ... }
  ```

  **Specific test cases:**
  - `test_get_staged_files_empty()` - empty index returns empty vec
  - `test_get_staged_files_with_changes()` - returns correct GitFile entries
  - `test_get_unstaged_files()` - detects unstaged modifications
  - `test_stage_files_adds_to_index()` - files added to staging area
  - `test_unstage_files_removes_from_index()` - files removed from staging
  - `test_create_commit_initial()` - first commit works (unborn branch)
  - `test_create_commit_normal()` - subsequent commit works
  - `test_has_staged_changes_true()` - returns true when changes staged
  - `test_has_staged_changes_false()` - returns false when nothing staged
  - `test_validate_for_commit_no_changes()` - errors when no staged changes
  - `test_validate_for_commit_with_conflicts()` - errors when conflicts exist
  - `test_build_staged_diff()` - returns correct diff string
  - `test_build_staged_diff_empty()` - errors when no staged changes

  **Must NOT do:**
  - Test GPG signing path (skip by setting commit.gpgsign = false)
  - Test with user's actual repositories

  **Recommended Agent Profile:**
  - **Category**: `ultrabrain`
  - **Skills**: rust, git2, trait-design
  - **Justification**: Complex trait extraction, many edge cases

  **Parallelization:**
  - **Can Run In Parallel**: YES (after Task 1)
  - **Parallel Group**: Wave 3
  - **Blocks**: Task 11 (generate.rs uses git adapter)
  - **Blocked By**: Task 1 (needs TempRepo)

  **References:**
  - `christina/src/io/git/adapter.rs` - Current implementation
  - `christina-core/src/git/file.rs` - GitFile type
  - `git2` crate docs for Repository, Index, Diff

  **Acceptance Criteria:**
  - [ ] GitRepository trait extracted and implemented
  - [ ] All public functions have integration tests
  - [ ] Tests use temp repos (not user repos)
  - [ ] GPG signing disabled in test repos
  - [ ] All tests pass

  **Commit**: YES
  - Message: `refactor: extract GitRepository trait and add tests`
  - Files: `christina/src/io/git/adapter.rs`

---

- [ ] **9. Test Chunking Algorithm**

  **What to do:**
  - Add unit tests to `christina/src/io/git/chunking.rs`
  - Add property-based tests for algorithm invariants
  - Use DeterministicTokenizer from test helpers

  **Specific test cases:**

  **split_recursive:**
  - `test_split_empty()` - empty input returns empty vec
  - `test_split_single_small_file()` - single file fits in one chunk
  - `test_split_multiple_files_one_chunk()` - multiple small files packed together
  - `test_split_large_file_multiple_chunks()` - large file split across chunks
  - `test_split_respects_token_limit()` - no chunk exceeds limit
  - `test_split_lockfile_truncation()` - lockfiles truncated with marker

  **split_by_hunks:**
  - `test_split_by_hunks_basic()` - splits on hunk boundaries
  - `test_split_by_hunks_header_only()` - handles header-only hunks
  - `test_split_by_hunks_fallback_to_lines()` - falls back when hunk too large

  **split_by_lines:**
  - `test_split_by_lines_basic()` - splits on line boundaries
  - `test_split_oversized_line()` - handles lines exceeding token limit

  **truncate_to_token_limit:**
  - `test_truncate_respects_limit()` - output within token limit
  - `test_truncate_preserves_newline()` - tries to end at newline
  - `test_truncate_fallback_to_lines()` - falls back when needed

  **Property-based tests:**
  ```rust
  proptest! {
      #[test]
      fn chunks_never_exceed_token_limit(
          files in vec(file_diff_strategy(), 0..10),
          limit in 100usize..10000
      ) {
          let tokenizer = DeterministicTokenizer::new();
          let chunks = split_recursive(files, TokenCount(limit), &[], &tokenizer);
          
          for chunk in chunks {
              let tokens = tokenizer.count_tokens(&chunk.content);
              prop_assert!(tokens <= limit);
          }
      }
      
      #[test]
      fn utf8_boundaries_preserved(
          content in "\PC{0,10000}"
      ) {
          let tokenizer = DeterministicTokenizer::new();
          let truncated = truncate_to_token_limit(&content, TokenCount(100), &tokenizer);
          // Should be valid UTF-8
          prop_assert!(std::str::from_utf8(truncated.as_bytes()).is_ok());
      }
  }
  ```

  **Must NOT do:**
  - Test with real LLM tokenizer (too slow, non-deterministic)
  - Skip edge cases (empty input, single token, exact limit)

  **Recommended Agent Profile:**
  - **Category**: `ultrabrain`
  - **Skills**: rust, algorithms, property-testing
  - **Justification**: Complex algorithm, needs property-based testing

  **Parallelization:**
  - **Can Run In Parallel**: YES (after Task 1)
  - **Parallel Group**: Wave 3
  - **Blocks**: None
  - **Blocked By**: Task 1 (needs DeterministicTokenizer)

  **References:**
  - `christina/src/io/git/chunking.rs` - All chunking functions
  - `christina-core/src/tokenizer.rs` - Tokenizer trait
  - `proptest` crate for property-based testing

  **Acceptance Criteria:**
  - [ ] All chunking functions have unit tests
  - [ ] Property-based tests verify invariants
  - [ ] Edge cases covered (empty, single, oversized)
  - [ ] Tests run in <1 second

  **Commit**: YES
  - Message: `test: add chunking algorithm tests`
  - Files: `christina/src/io/git/chunking.rs`
  - Pre-commit: `cargo nextest run -- chunking`

---

- [ ] **10. Refactor Profile CLI + Add Tests**

  **What to do:**
  - Refactor `christina/src/config/profile_cli.rs` for testability
  - Replace process::exit with Result returns
  - Abstract stdin for confirmation prompts
  - Add comprehensive tests

  **Refactoring required:**
  - Replace `process::exit` with `Result` returns
  - Add `input: &mut dyn BufRead` parameter to functions needing input
  - Or use trait abstraction: `trait UserInput { fn confirm(&mut self, msg: &str) -> bool; }`
  - Inject Config loader/saver

  **Specific test cases:**
  - `test_handle_list_shows_profiles()` - lists all profiles
  - `test_handle_show_existing()` - shows profile details
  - `test_handle_show_missing()` - error for unknown profile
  - `test_handle_create_success()` - creates new profile
  - `test_handle_create_duplicate()` - error for duplicate name
  - `test_handle_create_invalid_provider()` - error for bad provider
  - `test_handle_edit_updates_profile()` - edits existing profile
  - `test_handle_delete_confirmed()` - deletes when confirmed
  - `test_handle_delete_cancelled()` - doesn't delete when cancelled
  - `test_handle_switch_sets_active()` - switches active profile
  - `test_handle_duplicate_creates_copy()` - duplicates profile
  - `test_parse_secret_input_env()` - parses EnvVar syntax
  - `test_parse_secret_input_keyring()` - parses Keyring syntax
  - `test_parse_secret_input_literal()` - parses literal fallback

  **Must NOT do:**
  - Leave process::exit calls (breaks test runner)
  - Test actual keyring operations (use mocks)

  **Recommended Agent Profile:**
  - **Category**: `ultrabrain`
  - **Skills**: rust, refactoring, cli-testing
  - **Justification**: Complex refactoring, many branches

  **Parallelization:**
  - **Can Run In Parallel**: YES (after Task 1)
  - **Parallel Group**: Wave 3
  - **Blocks**: None
  - **Blocked By**: Task 1 (needs MockStdin, temp_config)

  **References:**
  - `christina/src/config/profile_cli.rs` - Current implementation
  - `christina/src/config/cli.rs` - See Task 6 for Result pattern
  - `christina-core/src/config/mod.rs` - Config, ProviderProfile

  **Acceptance Criteria:**
  - [ ] All process::exit calls replaced
  - [ ] Stdin abstracted for testing
  - [ ] All subcommands tested
  - [ ] Confirmation prompts tested
  - [ ] Error paths tested

  **Commit**: YES
  - Message: `refactor: make profile CLI testable and add tests`
  - Files: `christina/src/config/profile_cli.rs`

---

### Wave 4: Orchestration

- [ ] **11. Refactor Generate Module + Add Tests**

  **What to do:**
  - Refactor `christina/src/generate.rs` for dependency injection
  - Add tests for orchestration logic with mocked dependencies

  **Refactoring required:**
  - Extract `AIOrchestrator` factory trait or use function parameters
  - Inject `Provider` instead of constructing inline
  - Extract `get_commit_history` to accept Repository parameter
  - Make `generate_commit_message_with_progress` accept:
    - `orchestrator: &dyn Orchestrator` (or generic)
    - `repo: &dyn GitRepository`
    - `tokenizer: &dyn Tokenizer`

  ```rust
  pub trait Orchestrator {
      async fn generate(
          &self,
          chunks: Vec<DiffChunk>,
          user_context: Option<String>,
          // ... other params
      ) -> Result<GenerationResult>;
  }
  
  // Production implementation
  pub struct AIOrchestratorImpl { ... }
  
  // Test implementation
  #[cfg(test)]
  pub struct MockOrchestrator { ... }
  ```

  **Specific test cases:**
  - `test_progress_receiver_dropped_early()` - returns error if receiver closed
  - `test_missing_api_key()` - returns error if API key not found
  - `test_empty_chunks()` - returns error if no processable content
  - `test_sends_progress_events()` - sends correct progress sequence
  - `test_sends_token_count_update()` - sends token count event
  - `test_orchestrator_error_propagated()` - propagates orchestrator errors
  - `test_successful_generation()` - returns correct GenerationResult
  - `test_commit_history_included()` - includes commit history when enabled
  - `test_commit_history_truncated()` - truncates history to fit budget

  **Must NOT do:**
  - Test with real LLM calls (use MockOrchestrator)
  - Test with real git repos (use MockGitRepository)

  **Recommended Agent Profile:**
  - **Category**: `ultrabrain`
  - **Skills**: rust, async, trait-design, testing
  - **Justification**: Complex async orchestration, requires careful trait design

  **Parallelization:**
  - **Can Run In Parallel**: YES (after Tasks 1, 8)
  - **Parallel Group**: Wave 4
  - **Blocks**: None
  - **Blocked By**: Task 1 (test helpers), Task 8 (GitRepository trait)

  **References:**
  - `christina/src/generate.rs` - Current implementation
  - `christina/src/io/llm/orchestrator.rs` - AIOrchestrator
  - `christina/src/io/llm/provider.rs` - Provider trait + mocks

  **Acceptance Criteria:**
  - [ ] Orchestrator trait extracted
  - [ ] GitRepository trait used (from Task 8)
  - [ ] All error paths tested
  - [ ] Progress event sequence verified
  - [ ] Async tests use `#[tokio::test]`

  **Commit**: YES
  - Message: `refactor: make generate module testable and add tests`
  - Files: `christina/src/generate.rs`

---

- [ ] **12. Refactor Event Producers + Add Tests**

  **What to do:**
  - Refactor `christina/src/event_loop/producers.rs` for testability
  - Abstract input source and tick provider
  - Add tests for spawn/shutdown logic

  **Refactoring required:**
  - Extract input source trait:
    ```rust
    pub trait InputSource {
        fn poll_event(&self, timeout: Duration) -> Result<Option<Event>>;
    }
    
    // Production: CrosstermInput
    // Test: MockInput(Vec<Event>)
    ```
  - Extract tick provider trait:
    ```rust
    pub trait TickProvider {
        async fn tick(&self);
    }
    ```
  - Split `spawn` into composable functions:
    - `spawn_input_thread(input_source, event_tx, stop_signal)`
    - `spawn_tick_task(tick_provider, event_tx, stop_signal)`

  **Specific test cases:**
  - `test_input_thread_sends_events()` - input events sent to channel
  - `test_input_thread_handles_resize()` - resize events sent
  - `test_input_thread_respects_stop_signal()` - stops when signal set
  - `test_tick_task_sends_ticks()` - tick events sent periodically
  - `test_tick_task_respects_stop_signal()` - stops when signal set
  - `test_shutdown_stops_both()` - shutdown stops input and tick
  - `test_shutdown_completes_cleanly()` - no panics on shutdown

  **Must NOT do:**
  - Test with real terminal input (use MockInput)
  - Test with real time (use MockTickProvider or tokio::time::pause)

  **Recommended Agent Profile:**
  - **Category**: `ultrabrain`
  - **Skills**: rust, async, concurrency, testing
  - **Justification**: Complex async concurrency, requires careful abstraction

  **Parallelization:**
  - **Can Run In Parallel**: YES (after Task 1)
  - **Parallel Group**: Wave 4
  - **Blocks**: None
  - **Blocked By**: Task 1 (test helpers)

  **References:**
  - `christina/src/event_loop/producers.rs` - Current implementation
  - `crossterm::event` docs for event types
  - Tokio docs for spawn, channels, cancellation

  **Acceptance Criteria:**
  - [ ] InputSource trait extracted
  - [ ] TickProvider trait extracted
  - [ ] spawn split into testable functions
  - [ ] All shutdown scenarios tested
  - [ ] Async tests use `#[tokio::test]` with `start_paused = true` where needed

  **Commit**: YES
  - Message: `refactor: make event producers testable and add tests`
  - Files: `christina/src/event_loop/producers.rs`

---

## Commit Strategy

| After Task | Message | Files | Verification |
|------------|---------|-------|--------------|
| 1 | `test: add test helper infrastructure` | test_helpers.rs | `cargo nextest run -- test_helpers` |
| 2 | `test: add unit tests for core config types` | config_file.rs, resolved.rs, ids.rs, snapshot.rs | `cargo nextest run -- config` |
| 3 | `test: add CLI parsing tests` | cli.rs, diff_tool.rs | `cargo nextest run -- cli` |
| 4 | `test: add tests for pure logic functions` | init.rs, handlers.rs | `cargo nextest run -- app::init` |
| 5 | `test: add app state tests` | context.rs, state.rs | `cargo nextest run -- app::` |
| 6 | `refactor: make config CLI testable and add tests` | config/cli.rs | `cargo run -- config list` |
| 7 | `test: complete event handler tests` | handlers.rs | `cargo nextest run -- event_loop` |
| 8 | `refactor: extract GitRepository trait and add tests` | adapter.rs | `cargo nextest run -- git::adapter` |
| 9 | `test: add chunking algorithm tests` | chunking.rs | `cargo nextest run -- chunking` |
| 10 | `refactor: make profile CLI testable and add tests` | profile_cli.rs | `cargo run -- profile list` |
| 11 | `refactor: make generate module testable and add tests` | generate.rs | `cargo nextest run -- generate` |
| 12 | `refactor: make event producers testable and add tests` | producers.rs | `cargo nextest run -- producers` |

---

## Success Criteria

### Verification Commands
```bash
# All quality gates must pass
just check
just clippy
just test

# Test count should increase significantly
cargo nextest run --workspace 2>&1 | grep "test result"

# All tests should complete quickly
time cargo nextest run --workspace
```

### Final Checklist
- [ ] All 17 modules have meaningful test coverage
- [ ] Test count increased from 165 to ~285+ (120+ new tests)
- [ ] `just check` passes with 0 warnings
- [ ] `just clippy` passes with 0 warnings
- [ ] `just test` passes with 0 failures
- [ ] All tests complete in <1 second
- [ ] No process::exit calls in tested code paths
- [ ] No tests depend on user's home directory or real git repos
- [ ] No tests make real network calls

### Coverage Targets (Meaningful, Not Line Coverage)
- **Core domain types**: 100% (config_file, resolved, ids, snapshot)
- **CLI parsing**: 100% of valid/invalid input combinations
- **Pure logic functions**: 100% branch coverage (determine_initial_state, format_error_message)
- **Git adapter**: All public functions tested with temp repos
- **Chunking algorithm**: All edge cases + property-based invariants
- **Orchestration**: All error paths + success paths with mocks
- **State management**: All state transitions tested

---

## Notes for Executor

### Before Starting
1. Read existing test patterns in:
   - `christina-core/src/types/provider_kind.rs`
   - `christina-core/src/state.rs`
   - `christina/src/io/llm/orchestrator.rs`

2. Understand the manual mock pattern in:
   - `christina/src/io/llm/provider.rs` (Provider::Mock, Provider::MockSequence)

3. Review quality gates:
   - Run `just check`, `just clippy`, `just test` before starting to establish baseline

### During Execution
1. **Start with Wave 1** - these are independent and build foundation
2. **Task 1 (test helpers) is critical** - many other tasks depend on it
3. **Refactoring tasks** (6, 8, 10, 11, 12) require careful trait design
4. **Always run quality gates** after each commit
5. **Follow existing patterns** - inline tests, manual mocks, tokio::test

### Common Pitfalls to Avoid
1. Don't add external mocking crates - use manual mocks
2. Don't test with real git repos in user's home - use TempRepo helper
3. Don't leave process::exit in tested code - refactor to Result
4. Don't skip error paths - test both Ok and Err cases
5. Don't test trivial getters - focus on logic and edge cases

### When to Ask for Help
- If trait design for git adapter becomes unclear
- If async test timing is flaky
- If chunking algorithm invariants are unclear
- If CLI refactoring breaks existing behavior
