# Christina-Vibe Completion Plan

## Executive Summary

This plan addresses **5 critical gaps** preventing the christina-vibe TUI application from being fully functional. The codebase is architecturally complete but has functional stubs that block runtime behavior. This plan delivers a **production-ready system** with parallel execution waves for maximum efficiency.

**Current State:**
- ✅ Architecture complete (Elm-like state model, TUI screens, error types)
- ✅ 401 tests pass, zero clippy warnings
- ❌ File lists not populated (blocking TUI display)
- ❌ Event loop commands are no-ops (blocking generation flow)
- ❌ Integration tests missing
- ❌ Tokenizer has performance TODO

**Target State:**
- Fully functional end-to-end commit message generation
- All quality gates passing (`just check`, `just clippy -- -D warnings`)
- Comprehensive integration test coverage
- Production-ready retry/concurrency integration

---

## Task Dependency Graph

| Task | Depends On | Blocks | Reason |
|------|------------|--------|--------|
| 1. Fix File List Population | None | 2, 3 | Core data flow - TUI cannot display files without this |
| 2. Wire Event Loop Commands | 1 | 4 | Generation flow depends on working file lists |
| 3. Implement Unstaged File Loading | 1 | 4 | Complete file list functionality |
| 4. Integration Tests | 1, 2, 3 | None | End-to-end verification requires working flows |
| 5. Tokenizer Optimization | None | None | Independent performance improvement |
| 6. Remove Dead Code Attributes | 1, 2, 3 | None | Cleanup after wiring complete |

---

## Parallel Execution Graph

```
Wave 1 (Start Immediately - No Dependencies):
├── Task 1: Fix File List Population (app/mod.rs, app/init.rs)
└── Task 5: Tokenizer Optimization (tokenizer.rs)

Wave 2 (After Wave 1 completes):
├── Task 2: Wire Event Loop Commands (cmd_exec.rs)
└── Task 3: Implement Unstaged File Loading (init.rs completion)

Wave 3 (After Wave 2 completes):
└── Task 4: Integration Tests (christina/tests/)

Wave 4 (Final cleanup):
└── Task 6: Remove Dead Code Attributes (retry.rs, concurrency.rs)

Critical Path: Task 1 → Task 2 → Task 4
Parallel Speedup: ~35% faster than sequential execution
```

---

## Tasks

### Task 1: Fix File List Population in app/mod.rs and app/init.rs

**Priority:** CRITICAL - BLOCKING  
**Estimated Effort:** Medium  
**Wave:** 1 (Start Immediately)

#### Problem Analysis

Two locations have stubs preventing file list population:

1. **app/mod.rs:114-117** in `validate_repo_state()`:
   ```rust
   let staged = Vec::new(); // Stub
   let unstaged = Vec::new(); // Stub
   ```
   When repo re-discovery succeeds, these empty vectors are assigned instead of calling `load_file_lists()`.

2. **app/init.rs:68-107** in `load_file_lists()`:
   - Staged files are loaded from git index (lines 81-93)
   - **Unstaged files are NEVER loaded** (line 95: `let unstaged = Vec::new();`)

#### Implementation Requirements

**Part A: Fix app/mod.rs `validate_repo_state()`**
- Replace stub vectors with call to `load_file_lists()`
- Handle the case where `load_file_lists()` returns warnings
- Ensure proper error handling for git operations

**Part B: Complete app/init.rs `load_file_lists()`**
- Implement unstaged file loading using `git2::Repository::diff_index_to_workdir()`
- Follow same pattern as staged file loading (lines 81-93)
- Include diff content capture for unstaged files
- Handle untracked files appropriately

#### Technical Details

**For unstaged file loading:**
```rust
// Use diff_index_to_workdir to get unstaged changes
let mut opts = DiffOptions::new();
opts.include_untracked(true)
    .ignore_whitespace_change(false);

let diff = repo.diff_index_to_workdir(Some(&index), Some(&mut opts))?;
// Similar foreach pattern to staged files
```

**Key considerations:**
- Untracked files should be included (user needs to see them)
- Binary file detection should be consistent with staged files
- Diff content should be captured for display in TUI
- Error handling should match staged file patterns

#### Acceptance Criteria

- [ ] `validate_repo_state()` calls `load_file_lists()` instead of using stub vectors
- [ ] `load_file_lists()` populates both staged AND unstaged file vectors
- [ ] Untracked files appear in unstaged list
- [ ] File status indicators (A, M, D, R, C, ?) are correct
- [ ] Diff content is captured for unstaged files
- [ ] `just check` passes with zero warnings
- [ ] `just clippy -- -D warnings` passes with zero warnings
- [ ] Existing 401 tests still pass

#### Verification

```bash
# Build and check
just check
just clippy -- -D warnings

# Run tests
cargo test --workspace

# Manual verification (requires git repo with changes)
cd /tmp && mkdir test_repo && cd test_repo && git init
echo "test" > file.txt
cargo run --bin christina 2>&1 | head -20
# Should show file.txt in unstaged list
```

#### Delegation Recommendation

- **Category:** `unspecified-high` - Complex git2 integration requiring careful error handling
- **Skills:** [`git-master`] - Git operations and repository handling
- **Reasoning:** This task requires deep understanding of git2 crate patterns and proper error handling. The git-master skill provides expertise in git operations.

---

### Task 2: Wire Event Loop Commands in cmd_exec.rs

**Priority:** CRITICAL - BLOCKING  
**Estimated Effort:** Large  
**Wave:** 2 (Depends on Task 1)
**Depends On:** Task 1

#### Problem Analysis

`christina/src/runtime/cmd_exec.rs` has three documented no-ops:

1. **`Cmd::StartGeneration`** (lines 54-58): Returns empty vec, does nothing
2. **`Cmd::CancelGeneration`** (lines 60-64): Returns empty vec, does nothing  
3. **`Cmd::ShowToast`** (lines 66-70): Returns empty vec, does nothing

The actual generation flow is in `event_loop/mod.rs:try_start_generation()` which works correctly. The issue is that commands issued through the command system don't trigger this flow.

#### Implementation Requirements

**Part A: Wire `Cmd::StartGeneration`**
- This command should trigger the generation flow
- Two approaches:
  1. **Direct approach:** Move generation logic from event_loop into cmd_exec
  2. **Message approach:** Return a message that event_loop handles

**Recommended approach:** The event_loop already has working generation logic. The `Cmd::StartGeneration` should return a message that causes the event loop to call `try_start_generation()`.

**Part B: Wire `Cmd::CancelGeneration`**
- Should abort the in-flight generation task
- Need access to `AbortOnDrop` handle from `GenerationState::Running`
- May require passing additional context to `execute_cmd`

**Part C: Wire `Cmd::ShowToast`**
- Should display toast notification via `ToastManager`
- Need access to app's toast manager
- Return a message that UI layer handles

#### Technical Architecture

**Current flow:**
```
User Input → handlers::handle_input() → Cmd → execute_cmd() → [NO-OP]
```

**Required flow:**
```
User Input → handlers::handle_input() → Cmd → execute_cmd() → Msg → Event Loop → Action
```

**Key insight:** The `execute_cmd` function returns `Vec<Msg>`. These messages are processed by the event loop. The wiring is already partially there - we just need to:
1. Return appropriate `Msg` variants from `execute_cmd`
2. Handle those messages in the event loop
3. Connect to existing working code

#### Required Changes

**In cmd_exec.rs:**
- `Cmd::StartGeneration`: Return `Msg::StartGeneration` (new variant)
- `Cmd::CancelGeneration`: Return `Msg::CancelGeneration` (new variant)
- `Cmd::ShowToast`: Return `Msg::ShowToast { message, severity }` (new variant)

**In event_loop/mod.rs:**
- Add handlers for new `Msg` variants:
  - `Msg::StartGeneration`: Call `try_start_generation(app, tx).await`
  - `Msg::CancelGeneration`: Abort running generation task
  - `Msg::ShowToast`: Add toast to app's toast manager

**In christina-core/src/app/msg.rs:**
- Add new message variants:
  ```rust
  StartGeneration,
  CancelGeneration,
  ShowToast { message: String, severity: ToastSeverity },
  ```

#### Acceptance Criteria

- [ ] `Cmd::StartGeneration` triggers the AI generation flow
- [ ] `Cmd::CancelGeneration` aborts in-flight generation
- [ ] `Cmd::ShowToast` displays toast notifications
- [ ] All three commands work end-to-end in the TUI
- [ ] Generation can be started from Staging screen
- [ ] Generation can be cancelled with 'q' or Esc key
- [ ] Toasts appear for user feedback
- [ ] `just check` passes with zero warnings
- [ ] `just clippy -- -D warnings` passes with zero warnings
- [ ] Existing 401 tests still pass

#### Verification

```bash
# Build and check
just check
just clippy -- -D warnings

# Run tests
cargo test --workspace

# Manual verification (requires API key)
cargo run --bin christina
# Navigate to staging screen, stage files, press 'g' to generate
# Should see generation progress and result
```

#### Delegation Recommendation

- **Category:** `ultrabrain` - Complex async flow coordination, message passing architecture
- **Skills:** [] - No specific skills needed; this is core Rust async architecture
- **Reasoning:** This requires understanding the Elm architecture pattern, async message passing, and careful coordination between command execution and event loop. The ultrabrain category provides deep reasoning capabilities for complex flow design.

---

### Task 3: Implement Unstaged File Loading

**Priority:** HIGH  
**Estimated Effort:** Medium  
**Wave:** 2 (Depends on Task 1)
**Depends On:** Task 1

#### Problem Analysis

This is technically part of Task 1, but separated because:
1. It can be done in parallel with Task 2 once Task 1's staged file fix is complete
2. It's a distinct chunk of work (unstaged vs staged loading)
3. It allows parallel development

Current state in `app/init.rs:95`:
```rust
let unstaged = Vec::new(); // TODO: Implement unstaged file loading
```

#### Implementation Requirements

Implement unstaged file loading following the staged file pattern:

1. **Get diff between index and workdir:**
   ```rust
   let diff = repo.diff_index_to_workdir(Some(&index), Some(&mut opts))?;
   ```

2. **Configure options:**
   - `include_untracked(true)` - Show new files
   - `ignore_whitespace_change(false)` - Show all changes
   - Enable rename detection

3. **Collect file metadata:**
   - Path
   - Status (A, M, D, R, C, ?)
   - Diff content (for display)

4. **Handle edge cases:**
   - Binary files (mark as binary, skip diff content)
   - Large files (truncate diff content)
   - Permission changes (show as modified)

#### Acceptance Criteria

- [ ] Unstaged files appear in TUI staging screen
- [ ] File status is correct (new files show 'A', modified show 'M', etc.)
- [ ] Untracked files are included
- [ ] Diff content is available for display
- [ ] Binary files are marked appropriately
- [ ] `just check` passes with zero warnings
- [ ] `just clippy -- -D warnings` passes with zero warnings

#### Verification

```bash
# Build and check
just check
just clippy -- -D warnings

# Manual verification
cd /tmp/test_repo
echo "new line" >> file.txt
cargo run --bin christina
# Should see file.txt in unstaged list with status 'M'
```

#### Delegation Recommendation

- **Category:** `unspecified-low` - Straightforward implementation following existing patterns
- **Skills:** [`git-master`] - Git diff operations
- **Reasoning:** This mirrors the staged file loading pattern already implemented. The git-master skill ensures proper git2 API usage.

---

### Task 4: Integration Tests

**Priority:** HIGH  
**Estimated Effort:** Large  
**Wave:** 3 (Depends on Tasks 1, 2, 3)
**Depends On:** 1, 2, 3

#### Problem Analysis

Current workspace has NO top-level integration tests. The `old_backup` directory has:
- `christina-llm/tests/integration_test.rs` - LLM orchestrator tests
- `christina-git/tests/integration_test.rs` - Git workflow tests

These need to be ported and adapted to the new merged crate structure.

#### Implementation Requirements

**Create `christina/tests/integration_test.rs`:**

**Test 1: End-to-End Generation Flow**
```rust
#[tokio::test]
async fn end_to_end_generation_with_mock_provider() {
    // Setup: Create temp git repo with staged changes
    // Run: AIOrchestrator.generate_commit_message()
    // Assert: Valid commit message generated
}
```

**Test 2: Full Staging and Commit Workflow**
```rust
#[test]
fn full_staging_and_commit_workflow() {
    // Setup: Create temp git repo
    // Stage files using GitAdapter
    // Generate commit message
    // Create commit
    // Assert: Commit exists with correct message
}
```

**Test 3: Large Diff Processing**
```rust
#[test]
fn large_diff_processing_pipeline() {
    // Setup: Create large file (1000+ lines)
    // Stage file
    // Get staged diff
    // Assert: Diff is correctly captured
}
```

**Test 4: File List Population**
```rust
#[test]
fn file_list_population() {
    // Setup: Create repo with staged and unstaged files
    // Call load_file_lists()
    // Assert: Both lists populated correctly
}
```

**Test 5: Event Loop Command Flow**
```rust
#[tokio::test]
async fn event_loop_command_execution() {
    // Test Cmd::StartGeneration triggers generation
    // Test Cmd::CancelGeneration aborts generation
    // Test Cmd::ShowToast displays toast
}
```

#### Test Infrastructure

**Add to `christina/Cargo.toml` dev-dependencies:**
```toml
[dev-dependencies]
tempfile = "3.8"
tokio-test = "0.4"
```

**Test utilities module:**
Create `christina/tests/common/mod.rs` with:
- `init_test_repo()` - Creates temp git repo with test config
- `create_test_file()` - Creates files with content
- `stage_test_file()` - Stages files for testing

#### Acceptance Criteria

- [ ] `christina/tests/integration_test.rs` created
- [ ] `christina/tests/common/mod.rs` test utilities created
- [ ] All 5 test cases implemented
- [ ] Tests run with `cargo test --workspace`
- [ ] Tests pass in CI environment
- [ ] Tests use mock providers (no API keys needed)
- [ ] Tests clean up temp directories
- [ ] `just check` passes with zero warnings
- [ ] `just clippy -- -D warnings` passes with zero warnings

#### Verification

```bash
# Run integration tests
cargo test --workspace

# Run only integration tests
cargo test --test integration_test

# Check test coverage
cargo tarpaulin --workspace  # if tarpaulin installed
```

#### Delegation Recommendation

- **Category:** `unspecified-high` - Complex test setup, async testing, git operations
- **Skills:** [`git-master`] - Git repository manipulation in tests
- **Reasoning:** Integration tests require careful setup of git repositories, async test patterns, and proper cleanup. The git-master skill is essential for test repo manipulation.

---

### Task 5: Tokenizer Optimization

**Priority:** MEDIUM (Performance)  
**Estimated Effort:** Medium  
**Wave:** 1 (Independent, can run in parallel with Task 1)
**Depends On:** None

#### Problem Analysis

`christina-core/src/tokenizer.rs:21-22` has a TODO:
```rust
/// TODO: Currently we use a binary search approach to find the largest valid slice.
///       Research optimizations or alternative algorithms to improve performance.
```

The current `slice_to_token_limit()` implementation:
1. Uses binary search over byte positions
2. Calls `count_tokens()` for each mid-point (O(n log n) tokenizations)
3. For large diffs, this is expensive

#### Implementation Requirements

**Option A: Cumulative Token Count Cache**
- Tokenize once, cache token positions
- Binary search over token indices instead of byte positions
- Reduces tokenizations from O(n log n) to O(1)

**Option B: Streaming Tokenization**
- Tokenize incrementally
- Stop when limit reached
- Single pass through text

**Option C: Heuristic-Based Approach**
- Use character count as proxy for token count
- Only tokenize when near boundary
- Fast path for common cases

**Recommended: Option A (Cumulative Token Count Cache)**

This provides the best balance:
- Deterministic results
- Minimal code changes
- Significant performance improvement
- Works with any tokenizer implementation

#### Implementation Details

```rust
fn slice_to_token_limit<'a>(&self, text: &'a str, limit: TokenCount) -> &'a str {
    if self.count_tokens(text) <= limit {
        return text;
    }
    
    // Tokenize once and cache positions
    let tokens = self.encode(text);
    let token_positions = self.get_token_byte_positions(text, &tokens);
    
    // Binary search over token indices
    let mut low = 0;
    let mut high = tokens.len();
    let limit_usize = limit.as_usize();
    
    while low < high {
        let mid = (low + high) / 2;
        if mid <= limit_usize {
            low = mid + 1;
        } else {
            high = mid;
        }
    }
    
    // Find valid UTF-8 boundary
    let byte_pos = token_positions[low.saturating_sub(1)];
    &text[..byte_pos]
}
```

#### Acceptance Criteria

- [ ] Algorithm is O(n) instead of O(n log n) for tokenization
- [ ] All existing tests pass
- [ ] Performance improvement measurable on large diffs (>10KB)
- [ ] Correct UTF-8 boundary handling maintained
- [ ] `just check` passes with zero warnings
- [ ] `just clippy -- -D warnings` passes with zero warnings

#### Verification

```bash
# Run tokenizer tests
cargo test --package christina-core tokenizer

# Benchmark (manual)
cargo test --package christina-core -- --nocapture
# Check that large text tests complete quickly
```

#### Delegation Recommendation

- **Category:** `ultrabrain` - Algorithm optimization, performance analysis
- **Skills:** [] - Pure algorithmic work
- **Reasoning:** This requires algorithmic thinking and careful benchmarking. The ultrabrain category provides deep reasoning for optimization decisions.

---

### Task 6: Remove Dead Code Attributes

**Priority:** LOW (Cleanup)  
**Estimated Effort:** Small  
**Wave:** 4 (Final cleanup)
**Depends On:** 1, 2, 3

#### Problem Analysis

Two files have `#![allow(dead_code)]` attributes that should be removed after wiring:

1. **`christina/src/io/llm/retry.rs:1-4`**:
   ```rust
   #![allow(
       dead_code,
       reason = "retry policy is defined here but wired into providers later"
   )]
   ```

2. **`christina/src/io/llm/concurrency.rs:1-4`**:
   ```rust
   #![allow(
       dead_code,
       reason = "rate limiting is defined here but wired into providers later"
   )]
   ```

#### Verification of Usage

**retry.rs is USED by:**
- `orchestrator.rs` - `use crate::io::llm::retry::{RetryPolicy, retry_with_backoff};`
- `orchestrator.rs` - `generate_with_retry()` function

**concurrency.rs is USED by:**
- `orchestrator.rs` - `use crate::io::llm::concurrency::RequestLimiter;`
- `orchestrator.rs` - `AIOrchestrator::map_phase()` uses `limiter.acquire()`

Both ARE already wired and used! The `dead_code` attributes are outdated.

#### Implementation

Simply remove the `#![allow(dead_code)]` attributes from both files.

#### Acceptance Criteria

- [ ] Remove `#![allow(dead_code)]` from `retry.rs`
- [ ] Remove `#![allow(dead_code)]` from `concurrency.rs`
- [ ] `just check` passes with zero warnings
- [ ] `just clippy -- -D warnings` passes with zero warnings
- [ ] No dead code warnings appear

#### Verification

```bash
just check
just clippy -- -D warnings
```

#### Delegation Recommendation

- **Category:** `quick` - Simple cleanup task
- **Skills:** [] - No special skills needed
- **Reasoning:** This is a trivial cleanup task that any agent can handle.

---

## Gap 4 Analysis: Retry/Concurrency Integration

**Status:** ✅ ALREADY COMPLETE

After thorough analysis of the codebase:

**retry.rs:**
- ✅ `RetryPolicy` used by `orchestrator.rs:RetryPolicy::default()`
- ✅ `retry_with_backoff()` used by `orchestrator.rs:generate_with_retry()`
- ✅ `generate_with_retry()` wraps all provider calls

**concurrency.rs:**
- ✅ `RequestLimiter` used by `orchestrator.rs:AIOrchestrator`
- ✅ `limiter.acquire()` called in `map_phase()` for each chunk
- ✅ Concurrency limit respected across all LLM requests

**Conclusion:** Gap 4 is already fully implemented. The dead_code attributes are outdated and will be removed in Task 6.

---

## Commit Strategy

| After Task | Commit Message | Files |
|------------|----------------|-------|
| Task 1 | `fix(app): wire file list population in validate_repo_state and load_file_lists` | `app/mod.rs`, `app/init.rs` |
| Task 2 | `feat(runtime): wire event loop commands for generation flow` | `cmd_exec.rs`, `event_loop/mod.rs`, `msg.rs` |
| Task 3 | `feat(app): implement unstaged file loading` | `app/init.rs` |
| Task 4 | `test: add integration tests for end-to-end flows` | `tests/integration_test.rs`, `tests/common/mod.rs` |
| Task 5 | `perf(tokenizer): optimize slice_to_token_limit with cumulative cache` | `tokenizer.rs` |
| Task 6 | `chore: remove outdated dead_code attributes` | `retry.rs`, `concurrency.rs` |

---

## Success Criteria

### Functional Verification

```bash
# 1. All quality gates pass
just check
just clippy -- -D warnings

# 2. All tests pass
cargo test --workspace

# 3. Manual end-to-end test
cargo run --bin christina
# - Navigate to staging screen
# - Verify staged/unstaged files appear
# - Stage a file
# - Press 'g' to generate
# - Verify generation completes
# - Verify commit message appears in review screen
```

### Final Checklist

- [ ] File lists populate correctly (staged and unstaged)
- [ ] Generation flow works end-to-end
- [ ] Cancel generation works
- [ ] Toast notifications appear
- [ ] Integration tests cover main flows
- [ ] Tokenizer optimized
- [ ] Dead code attributes removed
- [ ] Zero clippy warnings
- [ ] All 401+ tests pass
- [ ] No regressions in existing functionality

---

## Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Git2 API edge cases | Medium | High | Extensive testing with various repo states |
| Async race conditions | Low | High | Careful message passing design, existing patterns |
| Breaking existing tests | Low | Medium | Run full test suite after each task |
| Performance regression | Low | Low | Benchmark tokenizer before/after |

---

## Notes for Executor

1. **Task 1 is CRITICAL** - Everything depends on file lists working
2. **Task 2 is COMPLEX** - Requires understanding async message flow; take time to design properly
3. **Integration tests** - Can reference old_backup tests but don't blindly copy; adapt to new structure
4. **Quality gates are MANDATORY** - Zero warnings is non-negotiable per AGENTS.md
5. **Test as you go** - Don't wait until end to run tests
6. **Git operations** - Use git2 crate patterns consistently; handle errors gracefully

This plan delivers a **fully functional, production-ready christina-vibe** with all gaps closed and all quality gates passing.
