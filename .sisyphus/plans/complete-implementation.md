# Christina TUI - Complete Implementation Plan

## TL;DR

> **Objective**: Complete all stubbed implementations, remove dead code, and wire configuration/profile system throughout the codebase.
>
> **Scope**: 3 major workstreams (Git Adapter, LLM Provider Wiring, Configuration/Profile Integration) with 15+ atomic tasks
>
> **Estimated Effort**: Large (8-12 hours of focused work)
> **Parallel Execution**: YES - 4 waves of parallel tasks
> **Critical Path**: Git Adapter Implementation → App Module Wiring → LLM Provider Integration → Generate Function Implementation

---

## Context

### Current State
This is a Rust TUI application (christina) for generating conventional commits using LLMs. The codebase has extensive stubbed implementations that need completion:

**Codebase Structure:**
- `christina-core/` - Core types, config, LLM abstractions
- `christina/` - TUI application, runtime, git adapter, LLM providers

**Configuration System (EXISTS BUT NOT USED):**
- `christina-core/src/profile.rs` - ProviderProfile struct fully implemented
- `christina-core/src/config/resolved.rs` - ResolvedConfig with get_active_profile()
- Profile system is complete but runtime hardcodes model strings

**Stubbed Implementations:**
1. Git adapter functions (status, get_staged_files, get_unstaged_files, stage_files, unstage_files, create_commit)
2. App module functions (refresh_git_files, create_commit, unstage_file)
3. Handler functions (handle_stage_file, handle_stage_files)
4. LLM provider functions (execute_openai_request, execute_azure_request, execute_groq_request)
5. Generate function (generate_commit_message_with_progress)

**Clippy Errors:**
- 10 total: unwrap/expect in tests, panic! in tests

**Dead Code:**
- Backup directory with duplicate files
- Multiple #[expect(dead_code)] annotations

### Research Findings

**From Backup Crate (Reference Implementation):**
- `backup/christina-llm/src/provider.rs` - Provider::from_profile() factory pattern
- `backup/christina-llm/src/providers/http.rs` - build_llm(config) using config.model
- Pattern: Config → ProviderProfile → Provider instance → LLM execution

---

## Work Objectives

### Core Objective
Complete all stubbed implementations to transform the codebase from a non-functional skeleton into a working conventional commit generator TUI that properly uses configuration profiles for LLM provider selection.

### Concrete Deliverables
1. **Git Adapter**: Fully functional git operations (status, stage, unstage, commit)
2. **App Module**: Working refresh_git_files, create_commit, unstage_file
3. **Handlers**: Functional handle_stage_file and handle_stage_files
4. **LLM Providers**: Profile-driven model selection (no hardcoded models)
5. **Generate Function**: Complete implementation using Provider::from_profile pattern
6. **Clippy Clean**: Zero warnings (just check, just clippy pass)
7. **Dead Code Removed**: Backup directory cleaned, unnecessary annotations removed

### Definition of Done
- [ ] `just check` passes with zero warnings
- [ ] `just clippy` passes with zero warnings
- [ ] All stub implementations replaced with working code
- [ ] Configuration/profile system fully wired
- [ ] No hardcoded model strings in LLM providers
- [ ] All tests pass

### Must Have
- Git adapter fully implements all functions
- App module uses git adapter for all operations
- Handlers call app methods (not stubs)
- LLM providers use profile.model instead of hardcoded strings
- Generate function uses Provider::from_profile pattern
- All unwrap/expect in tests properly annotated

### Must NOT Have (Guardrails)
- NO hardcoded model strings ("gpt-4", "llama-3.3-70b-versatile")
- NO #[allow(dead_code)] or #[expect(dead_code)] suppressions without justification
- NO placeholder "not yet implemented" error messages
- NO ignored send errors (let _ = progress_tx.send(...))
- NO backup directory in final state

---

## Verification Strategy

### Test Infrastructure Assessment
- **Infrastructure exists**: YES (cargo test, built-in test modules)
- **User wants tests**: TDD-style verification for each task
- **Framework**: Built-in Rust test framework with tokio for async

### Verification Approach
Each task includes:
1. **Compilation check**: `cargo check` after changes
2. **Clippy check**: `cargo clippy --all-targets` 
3. **Test execution**: `cargo test` for affected modules
4. **Integration verification**: Manual workflow testing where applicable

---

## Execution Strategy

### Dependency Graph

```
Wave 1 (Foundation - No Dependencies):
├── Task 1: Fix Clippy Errors in Tests
├── Task 2: Remove Backup Directory
└── Task 3: Implement Git Adapter - status()

Wave 2 (Git Operations - Depends on Wave 1):
├── Task 4: Implement Git Adapter - get_staged_files()
├── Task 5: Implement Git Adapter - get_unstaged_files()
├── Task 6: Implement Git Adapter - stage_files()
├── Task 7: Implement Git Adapter - unstage_files()
└── Task 8: Implement Git Adapter - create_commit()

Wave 3 (App Integration - Depends on Wave 2):
├── Task 9: Implement App::refresh_git_files()
├── Task 10: Implement App::create_commit()
├── Task 11: Implement App::unstage_file()
└── Task 12: Implement Handlers (handle_stage_file, handle_stage_files)

Wave 4 (LLM Integration - Depends on Wave 3):
├── Task 13: Wire LLM Providers with Profile-Driven Model Selection
├── Task 14: Implement Generate Function
└── Task 15: Final Cleanup and Verification
```

### Critical Path
```
Task 1 → Task 3 → Task 4/5 → Task 9 → Task 10/11/12 → Task 13 → Task 14 → Task 15
```

### Parallel Speedup
- Wave 1: 3 tasks in parallel (~30% faster)
- Wave 2: 5 tasks in parallel (~50% faster)
- Wave 3: 4 tasks in parallel (~40% faster)
- Wave 4: 3 tasks sequential (dependencies)

---

## TODOs

### Wave 1: Foundation

#### Task 1: Fix Clippy Errors in Tests

**What to do**:
- Fix unwrap/expect usage in test files
- Fix panic! usage in provider_spec.rs

**Files to modify**:
- `/home/yehezkieldio/Documents/Scratchpad/christina-vibe/christina-core/src/app/update.rs` (line 258)
- `/home/yehezkieldio/Documents/Scratchpad/christina-vibe/christina-core/src/config/azure_endpoint.rs` (lines 94, 107, 126, 168, 170)
- `/home/yehezkieldio/Documents/Scratchpad/christina-vibe/christina-core/src/llm/provider_spec.rs` (lines 42, 43, 59, 66)

**Must NOT do**:
- Do NOT use `#[allow(clippy::unwrap_used)]` at function level
- Do NOT suppress warnings with `--quiet`

**Recommended Agent Profile**:
- **Category**: `quick`
- **Skills**: `rust`, `clippy`
- **Reason**: Simple test refactoring requiring Rust expertise

**Parallelization**:
- **Can Run In Parallel**: YES
- **Parallel Group**: Wave 1
- **Blocks**: None
- **Blocked By**: None

**References**:
- `christina-core/src/app/update.rs:258` - Test using `.unwrap()` on CommitMessage::try_from
- `christina-core/src/config/azure_endpoint.rs:94` - Test using `.expect()` on try_into
- `christina-core/src/llm/provider_spec.rs:66` - Test using `panic!()` in match fallback

**Acceptance Criteria**:
- [ ] All test modules use `#[allow(clippy::unwrap_used)]` at module level ONLY
- [ ] `cargo clippy --all-targets` shows zero warnings
- [ ] All tests still pass: `cargo test`

**Commit**: YES
- Message: `fix(clippy): annotate test modules with allow(unwrap_used)`
- Files: `christina-core/src/app/update.rs`, `christina-core/src/config/azure_endpoint.rs`, `christina-core/src/llm/provider_spec.rs`

---

#### Task 2: Remove Backup Directory

**What to do**:
- Delete the entire `backup/` directory from the workspace
- This directory contains duplicate/outdated code that should not be part of the build

**Files to modify**:
- Delete: `/home/yehezkieldio/Documents/Scratchpad/christina-vibe/backup/` (entire directory)

**Must NOT do**:
- Do NOT keep any files from backup (they're archived in git history if needed)
- Do NOT move files from backup to src (use as reference only)

**Recommended Agent Profile**:
- **Category**: `quick`
- **Skills**: `git`
- **Reason**: Simple cleanup task

**Parallelization**:
- **Can Run In Parallel**: YES
- **Parallel Group**: Wave 1
- **Blocks**: None
- **Blocked By**: None

**Acceptance Criteria**:
- [ ] `backup/` directory no longer exists in workspace
- [ ] `cargo build` still succeeds
- [ ] `cargo clippy` still succeeds

**Commit**: YES
- Message: `chore(cleanup): remove backup directory`
- Files: `backup/` (deleted)

---

#### Task 3: Implement Git Adapter - status()

**What to do**:
- Complete the `status()` function in git adapter
- Currently returns RepoSnapshot with empty staged/unstaged vectors
- Should populate staged/unstaged by calling get_staged_files/get_unstaged_files

**Files to modify**:
- `/home/yehezkieldio/Documents/Scratchpad/christina-vibe/christina/src/io/git/adapter.rs` (lines 16-61)

**Current Implementation**:
```rust
pub fn status() -> Result<RepoSnapshot> {
    // ... discovers repo, gets branch ...
    Ok(RepoSnapshot {
        files,
        staged: vec![],   // TODO: populate from get_staged_files
        unstaged: vec![], // TODO: populate from get_unstaged_files
        branch,
        repo_root: root,
    })
}
```

**What it should do**:
- Call `get_staged_files(&repo)` and `get_unstaged_files(&repo)`
- Populate `RepoSnapshot.staged` and `RepoSnapshot.unstaged` with file paths
- Merge staged and unstaged files into `RepoSnapshot.files`

**Must NOT do**:
- Do NOT leave TODO comments in implementation
- Do NOT return empty vectors

**Recommended Agent Profile**:
- **Category**: `unspecified-high`
- **Skills**: `rust`, `git2`
- **Reason**: Requires git2 crate knowledge and proper error handling

**Parallelization**:
- **Can Run In Parallel**: YES (with Tasks 1, 2)
- **Parallel Group**: Wave 1
- **Blocks**: Tasks 4, 5 (they depend on status() working)
- **Blocked By**: None

**References**:
- `christina/src/io/git/adapter.rs:104-163` - get_staged_files() implementation (already complete)
- `christina/src/io/git/adapter.rs:165-215` - get_unstaged_files() implementation (already complete)
- `christina-core/src/git/snapshot.rs` - RepoSnapshot struct definition

**Acceptance Criteria**:
- [ ] `status()` calls `get_staged_files(&repo)` and `get_unstaged_files(&repo)`
- [ ] `RepoSnapshot.staged` contains staged file paths
- [ ] `RepoSnapshot.unstaged` contains unstaged file paths
- [ ] `RepoSnapshot.files` contains merged file list
- [ ] `cargo check` passes
- [ ] `cargo clippy` passes

**Commit**: YES
- Message: `feat(git): implement status() to populate staged/unstaged files`
- Files: `christina/src/io/git/adapter.rs`

---

### Wave 2: Git Operations

#### Task 4: Implement Git Adapter - get_staged_files()

**What to do**:
- The function exists but has `#[expect(dead_code)]`
- Ensure it properly captures diff content for each file
- Remove the dead_code annotation once wired

**Files to modify**:
- `/home/yehezkieldio/Documents/Scratchpad/christina-vibe/christina/src/io/git/adapter.rs` (lines 104-163)

**Current State**:
- Function is implemented but marked as dead_code
- Creates GitFile entries with empty diff_content

**What it should do**:
- Capture per-file diff content using git2 Diff::print
- Populate GitFile.diff_content with actual patch text
- Handle binary files correctly

**Must NOT do**:
- Do NOT leave diff_content empty
- Do NOT keep #[expect(dead_code)] after wiring

**Recommended Agent Profile**:
- **Category**: `unspecified-high`
- **Skills**: `rust`, `git2`
- **Reason**: Requires git2 diff handling expertise

**Parallelization**:
- **Can Run In Parallel**: YES (with Tasks 5-8)
- **Parallel Group**: Wave 2
- **Blocks**: Task 9
- **Blocked By**: Task 3

**References**:
- `backup/christina-git/src/diff_processor.rs` - Reference for diff processing
- `christina-core/src/git/file.rs` - GitFile struct definition

**Acceptance Criteria**:
- [ ] Each GitFile has populated diff_content
- [ ] Binary files detected and marked
- [ ] #[expect(dead_code)] removed
- [ ] `cargo check` passes
- [ ] `cargo clippy` passes

**Commit**: YES
- Message: `feat(git): populate diff_content in get_staged_files`
- Files: `christina/src/io/git/adapter.rs`

---

#### Task 5: Implement Git Adapter - get_unstaged_files()

**What to do**:
- Similar to Task 4 but for unstaged files
- Capture diff between index and workdir
- Handle untracked files

**Files to modify**:
- `/home/yehezkieldio/Documents/Scratchpad/christina-vibe/christina/src/io/git/adapter.rs` (lines 165-215)

**Current State**:
- Function is implemented but marked as dead_code
- Creates GitFile entries with empty diff_content

**What it should do**:
- Capture per-file diff content for unstaged changes
- Handle untracked files (show as new file content)
- Populate GitFile.diff_content

**Must NOT do**:
- Do NOT leave diff_content empty
- Do NOT keep #[expect(dead_code)] after wiring

**Recommended Agent Profile**:
- **Category**: `unspecified-high`
- **Skills**: `rust`, `git2`
- **Reason**: Similar to Task 4, requires git2 expertise

**Parallelization**:
- **Can Run In Parallel**: YES (with Tasks 4, 6-8)
- **Parallel Group**: Wave 2
- **Blocks**: Task 9
- **Blocked By**: Task 3

**Acceptance Criteria**:
- [ ] Each GitFile has populated diff_content
- [ ] Untracked files handled correctly
- [ ] #[expect(dead_code)] removed
- [ ] `cargo check` passes
- [ ] `cargo clippy` passes

**Commit**: YES
- Message: `feat(git): populate diff_content in get_unstaged_files`
- Files: `christina/src/io/git/adapter.rs`

---

#### Task 6: Implement Git Adapter - stage_files()

**What to do**:
- Function exists and is implemented
- Remove #[expect(dead_code)] annotation
- Ensure path normalization to repo-relative paths

**Files to modify**:
- `/home/yehezkieldio/Documents/Scratchpad/christina-vibe/christina/src/io/git/adapter.rs` (lines 217-236)

**Current State**:
- Implementation is complete but marked dead_code
- Uses index.add_path and index.remove_path

**What it should do**:
- Stage files by adding to git index
- Handle file deletions (remove_path)
- Normalize paths to be repo-relative

**Must NOT do**:
- Do NOT keep #[expect(dead_code)] after wiring

**Recommended Agent Profile**:
- **Category**: `quick`
- **Skills**: `rust`, `git2`
- **Reason**: Implementation mostly complete, just needs wiring

**Parallelization**:
- **Can Run In Parallel**: YES (with Tasks 4, 5, 7, 8)
- **Parallel Group**: Wave 2
- **Blocks**: Task 12
- **Blocked By**: Task 3

**Acceptance Criteria**:
- [ ] #[expect(dead_code)] removed
- [ ] Paths normalized to repo-relative
- [ ] `cargo check` passes
- [ ] `cargo clippy` passes

**Commit**: YES
- Message: `feat(git): wire stage_files and remove dead_code annotation`
- Files: `christina/src/io/git/adapter.rs`

---

#### Task 7: Implement Git Adapter - unstage_files()

**What to do**:
- Function exists and is implemented
- Remove #[expect(dead_code)] annotation
- Ensure proper handling of unborn branch case

**Files to modify**:
- `/home/yehezkieldio/Documents/Scratchpad/christina-vibe/christina/src/io/git/adapter.rs` (lines 238-262)

**Current State**:
- Implementation is complete but marked dead_code
- Handles both normal repo and unborn branch cases

**What it should do**:
- Unstage files by resetting index to HEAD
- Handle unborn branch (remove from index directly)
- Normalize paths

**Must NOT do**:
- Do NOT keep #[expect(dead_code)] after wiring

**Recommended Agent Profile**:
- **Category**: `quick`
- **Skills**: `rust`, `git2`
- **Reason**: Implementation mostly complete

**Parallelization**:
- **Can Run In Parallel**: YES (with Tasks 4-6, 8)
- **Parallel Group**: Wave 2
- **Blocks**: Task 11
- **Blocked By**: Task 3

**Acceptance Criteria**:
- [ ] #[expect(dead_code)] removed
- [ ] Paths normalized to repo-relative
- [ ] `cargo check` passes
- [ ] `cargo clippy` passes

**Commit**: YES
- Message: `feat(git): wire unstage_files and remove dead_code annotation`
- Files: `christina/src/io/git/adapter.rs`

---

#### Task 8: Implement Git Adapter - create_commit()

**What to do**:
- Function exists and is implemented
- Remove #[expect(dead_code)] annotation
- Ensure validate_for_commit is called before creating commit

**Files to modify**:
- `/home/yehezkieldio/Documents/Scratchpad/christina-vibe/christina/src/io/git/adapter.rs` (lines 264-293)

**Current State**:
- Implementation is complete but marked dead_code
- Creates commit with signature, tree, and parents

**What it should do**:
- Validate repository state before commit
- Create commit with proper parent handling
- Handle unborn branch case

**Must NOT do**:
- Do NOT keep #[expect(dead_code)] after wiring

**Recommended Agent Profile**:
- **Category**: `quick`
- **Skills**: `rust`, `git2`
- **Reason**: Implementation mostly complete

**Parallelization**:
- **Can Run In Parallel**: YES (with Tasks 4-7)
- **Parallel Group**: Wave 2
- **Blocks**: Task 10
- **Blocked By**: Task 3

**Acceptance Criteria**:
- [ ] #[expect(dead_code)] removed
- [ ] `validate_for_commit()` called before commit
- [ ] `cargo check` passes
- [ ] `cargo clippy` passes

**Commit**: YES
- Message: `feat(git): wire create_commit and remove dead_code annotation`
- Files: `christina/src/io/git/adapter.rs`

---

### Wave 3: App Integration

#### Task 9: Implement App::refresh_git_files()

**What to do**:
- Replace stub implementation that just clears lists
- Call git adapter functions to populate staged/unstaged files
- Update data_version and branch name

**Files to modify**:
- `/home/yehezkieldio/Documents/Scratchpad/christina-vibe/christina/src/app/mod.rs` (lines 44-61)

**Current Implementation**:
```rust
pub fn refresh_git_files(&mut self) {
    if let Some(ref _repo) = self.app_context.repo {
        // Simple stub - just clear the lists
        self.data.base.staged_files.clear();
        self.data.base.unstaged_files.clear();
        // ...
    }
}
```

**What it should do**:
- Call `crate::io::git::adapter::get_staged_files(repo)`
- Call `crate::io::git::adapter::get_unstaged_files(repo)`
- Populate `self.data.base.staged_files` and `self.data.base.unstaged_files`
- Call `self.app_context.refresh_branch()`
- Handle errors with toasts

**Must NOT do**:
- Do NOT just clear lists
- Do NOT ignore adapter errors

**Recommended Agent Profile**:
- **Category**: `unspecified-high`
- **Skills**: `rust`, `tui`
- **Reason**: Requires understanding of app state management

**Parallelization**:
- **Can Run In Parallel**: NO (depends on Wave 2)
- **Parallel Group**: Wave 3
- **Blocks**: Tasks 10, 11, 12
- **Blocked By**: Tasks 4, 5

**References**:
- `backup/christina/src/app/mod.rs:44-81` - Reference implementation
- `christina/src/io/git/adapter.rs` - Adapter functions

**Acceptance Criteria**:
- [ ] Calls `get_staged_files()` and `get_unstaged_files()`
- [ ] Populates app data with file lists
- [ ] Shows error toasts on failure
- [ ] `cargo check` passes
- [ ] `cargo clippy` passes

**Commit**: YES
- Message: `feat(app): implement refresh_git_files using git adapter`
- Files: `christina/src/app/mod.rs`

---

#### Task 10: Implement App::create_commit()

**What to do**:
- Replace stub that returns Err("not yet implemented")
- Call git adapter create_commit function
- Refresh file lists after successful commit

**Files to modify**:
- `/home/yehezkieldio/Documents/Scratchpad/christina-vibe/christina/src/app/mod.rs` (lines 187-197)

**Current Implementation**:
```rust
pub fn create_commit(
    &mut self,
    _message: &christina_core::types::CommitMessage,
) -> Result<String, String> {
    let Some(ref _repo) = self.app_context.repo else {
        return Err("No git repository".to_string());
    };
    Err("Commit functionality not yet implemented".to_string())
}
```

**What it should do**:
- Call `crate::io::git::adapter::validate_for_commit(repo)`
- Call `crate::io::git::adapter::create_commit(repo, message.as_ref())`
- On success: call `self.refresh_git_files()` and return OID string
- On error: return error string

**Must NOT do**:
- Do NOT return "not yet implemented" error
- Do NOT skip validation

**Recommended Agent Profile**:
- **Category**: `unspecified-high`
- **Skills**: `rust`, `git2`
- **Reason**: Requires proper error handling

**Parallelization**:
- **Can Run In Parallel**: YES (with Tasks 11, 12)
- **Parallel Group**: Wave 3
- **Blocks**: None
- **Blocked By**: Task 8, Task 9

**References**:
- `backup/christina/src/app/mod.rs:20-31` - Reference implementation
- `christina/src/io/git/adapter.rs:264-293` - create_commit function

**Acceptance Criteria**:
- [ ] Calls `validate_for_commit()` before creating commit
- [ ] Calls `create_commit()` with message
- [ ] Refreshes file lists on success
- [ ] Returns proper error messages
- [ ] `cargo check` passes
- [ ] `cargo clippy` passes

**Commit**: YES
- Message: `feat(app): implement create_commit using git adapter`
- Files: `christina/src/app/mod.rs`

---

#### Task 11: Implement App::unstage_file()

**What to do**:
- Replace stub that returns Err("not yet implemented")
- Call git adapter unstage_files function
- Refresh file lists after operation

**Files to modify**:
- `/home/yehezkieldio/Documents/Scratchpad/christina-vibe/christina/src/app/mod.rs` (lines 211-218)

**Current Implementation**:
```rust
pub fn unstage_file(&mut self, _path: &Path) -> Result<(), String> {
    let Some(ref _repo) = self.app_context.repo else {
        return Err("No git repository".to_string());
    };
    Err("Unstage functionality not yet implemented".to_string())
}
```

**What it should do**:
- Convert path to string
- Call `crate::io::git::adapter::unstage_files(repo, &[path_string])`
- On success: call `self.refresh_git_files()` and return Ok(())
- On error: return error string

**Must NOT do**:
- Do NOT return "not yet implemented" error

**Recommended Agent Profile**:
- **Category**: `quick`
- **Skills**: `rust`
- **Reason**: Straightforward implementation

**Parallelization**:
- **Can Run In Parallel**: YES (with Tasks 10, 12)
- **Parallel Group**: Wave 3
- **Blocks**: None
- **Blocked By**: Task 7, Task 9

**References**:
- `backup/christina/src/app/mod.rs:41-49` - Reference implementation
- `christina/src/io/git/adapter.rs:238-262` - unstage_files function

**Acceptance Criteria**:
- [ ] Converts path to string
- [ ] Calls `unstage_files()`
- [ ] Refreshes file lists on success
- [ ] `cargo check` passes
- [ ] `cargo clippy` passes

**Commit**: YES
- Message: `feat(app): implement unstage_file using git adapter`
- Files: `christina/src/app/mod.rs`

---

#### Task 12: Implement Handlers (handle_stage_file, handle_stage_files)

**What to do**:
- Replace stubs that show error toast "not yet implemented"
- Call git adapter stage_files function
- Refresh file lists after staging
- Update UI with success/error toasts

**Files to modify**:
- `/home/yehezkieldio/Documents/Scratchpad/christina-vibe/christina/src/app/handlers.rs` (lines 42-83)

**Current Implementation**:
```rust
fn handle_stage_file(&mut self, path: FilePath) {
    // ... repo check ...
    self.data.base.toasts.error("Stage functionality not yet implemented".to_string());
}

fn handle_stage_files(&mut self, _paths: Vec<FilePath>) {
    // ... repo check ...
    self.data.base.toasts.error("Stage functionality not yet implemented".to_string());
}
```

**What it should do**:
- Find files in unstaged_files list
- Convert FilePath to String
- Call `crate::io::git::adapter::stage_files(repo, &paths)`
- Show success toast on success
- Show error toast on failure
- Call `self.refresh_git_files()` to update UI
- For multi-file staging, transition to Dashboard

**Must NOT do**:
- Do NOT show "not yet implemented" toast
- Do NOT skip refresh after staging

**Recommended Agent Profile**:
- **Category**: `unspecified-high`
- **Skills**: `rust`, `tui`
- **Reason**: Requires UI state management

**Parallelization**:
- **Can Run In Parallel**: YES (with Tasks 10, 11)
- **Parallel Group**: Wave 3
- **Blocks**: None
- **Blocked By**: Task 6, Task 9

**References**:
- `backup/christina/src/app/handlers.rs` - Reference implementation
- `christina/src/io/git/adapter.rs:217-236` - stage_files function

**Acceptance Criteria**:
- [ ] Finds files in unstaged list before staging
- [ ] Calls `stage_files()` with path list
- [ ] Shows success toast on success
- [ ] Shows error toast on failure
- [ ] Calls `refresh_git_files()` after staging
- [ ] Transitions to Dashboard for multi-file staging
- [ ] `cargo check` passes
- [ ] `cargo clippy` passes

**Commit**: YES
- Message: `feat(handlers): implement stage file handlers`
- Files: `christina/src/app/handlers.rs`

---

### Wave 4: LLM Integration

#### Task 13: Wire LLM Providers with Profile-Driven Model Selection

**What to do**:
- Remove hardcoded model strings from LLM provider functions
- Add model parameter to execute_*_request functions
- Use profile.model instead of hardcoded strings

**Files to modify**:
- `/home/yehezkieldio/Documents/Scratchpad/christina-vibe/christina/src/io/llm/openai.rs` (line 17)
- `/home/yehezkieldio/Documents/Scratchpad/christina-vibe/christina/src/io/llm/azure.rs` (line 22)
- `/home/yehezkieldio/Documents/Scratchpad/christina-vibe/christina/src/io/llm/groq.rs` (line 17)

**Current Hardcoded Models**:
- openai.rs: `.model("gpt-4")` (line 17)
- azure.rs: `.model("gpt-4")` (line 22)
- groq.rs: `.model("llama-3.3-70b-versatile")` (line 17)

**What it should do**:
- Add `model: &str` or `model: &ModelName` parameter to each function
- Replace `.model("...")` with `.model(model)` or `.model(model.as_ref())`
- Remove #[expect(dead_code)] annotations

**Must NOT do**:
- Do NOT leave hardcoded model strings
- Do NOT keep #[expect(dead_code)] after wiring

**Recommended Agent Profile**:
- **Category**: `unspecified-high`
- **Skills**: `rust`, `llm`
- **Reason**: Requires understanding of LLM builder pattern

**Parallelization**:
- **Can Run In Parallel**: YES (all 3 files)
- **Parallel Group**: Wave 4
- **Blocks**: Task 14
- **Blocked By**: None (can start after Wave 1)

**References**:
- `backup/christina-llm/src/providers/http.rs` - build_llm pattern
- `backup/christina-llm/src/providers/openai.rs` - OpenAI provider pattern
- `backup/christina-llm/src/providers/azure.rs` - Azure provider pattern
- `christina-core/src/types/model_name.rs` - ModelName type

**Acceptance Criteria**:
- [ ] All three files accept model parameter
- [ ] No hardcoded model strings remain
- [ ] #[expect(dead_code)] annotations removed
- [ ] `cargo check` passes
- [ ] `cargo clippy` passes

**Commit**: YES
- Message: `feat(llm): use profile-driven model selection in providers`
- Files: `christina/src/io/llm/openai.rs`, `christina/src/io/llm/azure.rs`, `christina/src/io/llm/groq.rs`

---

#### Task 14: Implement Generate Function

**What to do**:
- Replace stub implementation that returns placeholder message
- Use Provider::from_profile pattern from backup
- Wire config_to_profile function
- Handle progress_tx.send errors properly

**Files to modify**:
- `/home/yehezkieldio/Documents/Scratchpad/christina-vibe/christina/src/generate.rs` (lines 55-88)

**Current Implementation**:
```rust
pub async fn generate_commit_message_with_progress(
    _config: Config,
    _diff: String,
    progress_tx: mpsc::Sender<Event>,
    generation_id: u64,
    _user_context: Option<String>,
) -> Result<GenerationResult> {
    // Stub implementation
    let _ = progress_tx.send(...).await;
    let message = CommitMessage::try_from("chore: placeholder stub implementation".to_string())?;
    Ok(GenerationResult { message, warnings: vec![...] })
}
```

**What it should do**:
- Call `config_to_profile(&config)` to get ProviderProfile
- Get API key from config
- Create Provider using `Provider::from_profile(&profile, &api_key)`
- Send progress updates via progress_tx
- Handle send errors (don't use `let _ =`)
- Build LLM request with diff and user_context
- Call LLM and get response
- Parse response into CommitMessage
- Return GenerationResult with message and warnings

**Must NOT do**:
- Do NOT return placeholder message
- Do NOT use `let _ = progress_tx.send(...)`
- Do NOT ignore errors

**Recommended Agent Profile**:
- **Category**: `ultrabrain`
- **Skills**: `rust`, `llm`, `async`
- **Reason**: Complex async integration requiring multiple components

**Parallelization**:
- **Can Run In Parallel**: NO
- **Parallel Group**: Wave 4
- **Blocks**: Task 15
- **Blocked By**: Task 13

**References**:
- `backup/christina/src/generate.rs` - Complete reference implementation
- `backup/christina-llm/src/provider.rs` - Provider::from_profile pattern
- `backup/christina-llm/src/providers/http.rs` - build_llm pattern
- `christina/src/generate.rs:37-53` - config_to_profile function (already exists)

**Acceptance Criteria**:
- [ ] Uses `config_to_profile()` to get profile
- [ ] Creates Provider using `Provider::from_profile()`
- [ ] Handles progress_tx.send errors properly
- [ ] Builds and sends LLM request
- [ ] Parses response into CommitMessage
- [ ] Returns proper GenerationResult
- [ ] `cargo check` passes
- [ ] `cargo clippy` passes

**Commit**: YES
- Message: `feat(generate): implement commit message generation with LLM`
- Files: `christina/src/generate.rs`

---

#### Task 15: Final Cleanup and Verification

**What to do**:
- Remove any remaining #[expect(dead_code)] annotations
- Verify all stub implementations are complete
- Run full test suite
- Verify just check and just clippy pass

**Files to check**:
- All modified files from previous tasks
- Any remaining files with #[expect(dead_code)]

**Must NOT do**:
- Do NOT leave any stub implementations
- Do NOT leave any unnecessary suppressions

**Recommended Agent Profile**:
- **Category**: `quick`
- **Skills**: `rust`, `clippy`
- **Reason**: Final verification and cleanup

**Parallelization**:
- **Can Run In Parallel**: NO
- **Parallel Group**: Wave 4
- **Blocks**: None
- **Blocked By**: All previous tasks

**Acceptance Criteria**:
- [ ] `just check` passes with zero warnings
- [ ] `just clippy` passes with zero warnings
- [ ] `cargo test` passes
- [ ] No #[expect(dead_code)] annotations remain (except justified ones)
- [ ] No "not yet implemented" strings in codebase
- [ ] No hardcoded model strings

**Commit**: YES
- Message: `chore(cleanup): final verification and dead code removal`
- Files: Any remaining files with suppressions

---

## Commit Strategy

| After Task | Message | Files |
|------------|---------|-------|
| 1 | `fix(clippy): annotate test modules with allow(unwrap_used)` | christina-core test files |
| 2 | `chore(cleanup): remove backup directory` | backup/ |
| 3 | `feat(git): implement status() to populate staged/unstaged files` | adapter.rs |
| 4 | `feat(git): populate diff_content in get_staged_files` | adapter.rs |
| 5 | `feat(git): populate diff_content in get_unstaged_files` | adapter.rs |
| 6 | `feat(git): wire stage_files and remove dead_code annotation` | adapter.rs |
| 7 | `feat(git): wire unstage_files and remove dead_code annotation` | adapter.rs |
| 8 | `feat(git): wire create_commit and remove dead_code annotation` | adapter.rs |
| 9 | `feat(app): implement refresh_git_files using git adapter` | app/mod.rs |
| 10 | `feat(app): implement create_commit using git adapter` | app/mod.rs |
| 11 | `feat(app): implement unstage_file using git adapter` | app/mod.rs |
| 12 | `feat(handlers): implement stage file handlers` | handlers.rs |
| 13 | `feat(llm): use profile-driven model selection in providers` | llm/*.rs |
| 14 | `feat(generate): implement commit message generation with LLM` | generate.rs |
| 15 | `chore(cleanup): final verification and dead code removal` | various |

---

## Success Criteria

### Verification Commands
```bash
# Check compilation
just check

# Check clippy
just clippy

# Run tests
cargo test

# Verify no hardcoded models
grep -r '"gpt-4"' christina/src/io/llm/ || echo "No hardcoded gpt-4 found"
grep -r '"llama-3.3-70b-versatile"' christina/src/io/llm/ || echo "No hardcoded llama found"

# Verify no "not yet implemented" strings
grep -r "not yet implemented" christina/src/ || echo "No stub messages found"

# Verify no backup directory
[ ! -d "backup" ] && echo "Backup directory removed"
```

### Final Checklist
- [ ] All "Must Have" items present
- [ ] All "Must NOT Have" items absent
- [ ] All tests pass
- [ ] just check passes
- [ ] just clippy passes
- [ ] Configuration/profile system fully wired
- [ ] Git operations functional
- [ ] LLM generation functional

---

## Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| git2 API changes | Low | Medium | Use stable git2 features only |
| LLM provider API changes | Low | High | Abstract behind Provider trait |
| Merge conflicts | Medium | Low | Small, focused commits |
| Test failures | Medium | Medium | Fix tests as we go |
| Clippy warnings persist | Low | Low | Address immediately per task |

---

## Notes for Implementers

1. **Git Adapter Pattern**: The adapter functions are mostly complete but marked dead_code. Focus on wiring them into the app module and populating diff_content properly.

2. **Provider Pattern**: Follow the backup crate pattern exactly:
   - Config → ProviderProfile (config_to_profile)
   - ProviderProfile + api_key → Provider (Provider::from_profile)
   - Provider.generate() → LLM response

3. **Error Handling**: Use anyhow for errors, convert to user-friendly strings for UI display via toasts.

4. **Testing**: Each task should maintain or improve test coverage. Run `cargo test` after each commit.

5. **Clippy**: Never suppress warnings with `--quiet`. Fix the code or use targeted `#[allow(...)]` at module level for tests only.

6. **Documentation**: The backup crate is your reference implementation. When in doubt, check how it was done there.
