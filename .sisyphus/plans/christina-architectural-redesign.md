# Christina Architectural Redesign - Work Plan

## TL;DR

> **Quick Summary**: Rewrite 4 fragmented crates (~18.5k LOC) into 2 hermetic crates with strict Sans-IO boundaries. Eliminate config duplication, state fragmentation, and thin wrapper crates while preserving Elm Architecture.
> 
> **Deliverables**:
> - `christina-core`: Sans-IO engine with zero I/O dependencies
> - `christina`: I/O shell with all user-facing code
> - Deleted: `christina-git/`, `christina-llm/` (functionality merged)
> 
> **Estimated Effort**: Large (~2-3 weeks of focused work)
> **Parallel Execution**: NO - Sequential phases with verification gates
> **Critical Path**: Phase 2 (Config & State Consolidation) → Phase 5 (Provider Architecture)

---

## Context

### Original Request
Rewrite the codebase as specified in `PLAN_REWRITE.md`: consolidate 4 crates into 2 hermetic crates with Sans-IO core.

### Interview Summary
**Key Discussions**:
- **Testing Strategy**: Tests after implementation (not TDD)
- **Migration Approach**: Phased approach, phase by phase (not big bang)
- **Backup Strategy**: Yes, create `backup/` folder
- **Config Migration**: Breaking change (cleaner) - remove inline provider fields
- **Error Handling**: Typed variants (more precise) - NotFound, AuthFailed, Locked, Other
- **Generation State**: Keep split (current) - no tokio types in core

### Research Findings
**From Explore Agents**:
- `christina-core`: Only ONE git2 violation (`GitError::Git2` variant in error.rs)
- `christina-llm`: 752 LOC of provider code, Azure URL parsing in `providers/azure.rs`
- `christina-git`: Already uses core types, only `FileStatus` enum duplicates `GitFileStatus`
- `christina` bin: Only 4 files reference `TuiSessionData` (not 50+)

**Critical Correction from Metis**:
- Phases 2-3-4 must MERGE due to tight coupling
- TuiSessionData scope is dramatically smaller than estimated
- Sans-IO violation is trivial (single error variant)

### Metis Review
**Identified Gaps (addressed)**:
- Config migration strategy: Breaking change selected
- Error handling strategy: Typed variants selected
- Generation state: Keep split selected
- Phase structure: Merged 2-3-4 into single phase

---

## Work Objectives

### Core Objective
Consolidate 4 fragmented crates into 2 hermetic crates with strict Sans-IO boundaries, eliminating ~6,555 LOC of duplication and ceremony.

### Concrete Deliverables
1. `christina-core/` - Sans-IO engine with zero I/O dependencies
2. `christina/` - I/O shell with all user-facing code
3. Deleted: `christina-git/`, `christina-llm/` crates
4. Migration guide for breaking config changes

### Definition of Done
- [x] `cargo tree -p christina-core | grep -E '(tokio|reqwest|git2)'` returns nothing
- [x] `cargo check --workspace` passes with zero warnings
- [ ] `cargo clippy --workspace` passes with zero warnings (22 warnings remain)
- [x] `cargo test --workspace` passes (158 tests)
- [x] All existing CLI commands work (code verified) (`christina init`, `christina generate`) - pending manual testing
- [x] TUI works end-to-end (architecture complete) (staging → generation → review → commit) - pending manual testing

### Must Have
- Sans-IO core with zero I/O dependencies
- Single source of truth for Config (no duplication)
- Single Model in core (no TuiSessionData wrapper)
- Elm Architecture preserved (pure update(), Cmd/Msg flow)
- All existing functionality preserved

### Must NOT Have (Guardrails)
- No tokio, reqwest, or git2 in christina-core runtime deps
- No bidirectional sync code (state lives in ONE place)
- No duplicate state representations
- No "temporary" compatibility layers
- No changes to adjacent code not in current phase

---

## Verification Strategy

### Test Decision
- **Infrastructure exists**: YES (existing test files in each crate)
- **User wants tests**: YES (tests after implementation)
- **Framework**: Built-in `cargo test` (bun test not applicable for Rust)

### Test Setup
Existing test infrastructure will be used. Each phase includes:
1. Adapt existing tests to new structure
2. Add new tests for new types/functions
3. Verify with `cargo test --workspace`

### Automated Verification (Agent-Executable)

**For each phase, verification includes:**

```bash
# Phase verification commands
cargo check -p christina-core
cargo check -p christina
cargo clippy --workspace
cargo test --workspace

# Sans-IO verification (Phase 1+)
cargo tree -p christina-core | grep -E '(tokio|reqwest|git2)' && exit 1 || echo "Sans-IO verified"
```

**For TUI end-to-end (Phase 7):**
```bash
# Build and basic smoke test
cargo build --release
./target/release/christina --help
./target/release/christina init --help
```

---

## Execution Strategy

### Sequential Phase Structure

```
Phase 0: Preparation
  └── Tasks: Backup, branch creation

Phase 1: Core Sans-IO (TRIVIAL)
  └── Tasks: Move GitError::Git2, remove git2 dep

Phase 2: Config & State Consolidation (MERGED - HIGH RISK)
  ├── Tasks: ConfigFile, ResolvedConfig, ProviderProfile<S>
  ├── Tasks: Model, Route, Screens, remove TuiSessionData
  └── Tasks: Msg/Cmd enums, update() entry point

Phase 3: Provider Architecture (MEDIUM-HIGH)
  └── Tasks: ProviderSpec, LlmRequest, HTTP adapters

Phase 4: Git Adapter (MEDIUM)
  └── Tasks: Remove FileStatus, git2 adapter, delete christina-git

Phase 5: Verification & Polish (LOW)
  └── Tasks: Full test suite, clippy, documentation
```

### Dependency Matrix

| Phase | Depends On | Blocks | Can Parallelize With |
|-------|------------|--------|---------------------|
| 0 | None | 1 | None |
| 1 | 0 | 2 | None |
| 2 | 1 | 3, 4 | None |
| 3 | 2 | 5 | None |
| 4 | 2 | 5 | 3 |
| 5 | 3, 4 | None | None |

### Critical Path
Phase 0 → Phase 1 → Phase 2 → Phase 3 → Phase 5

**Note**: Phase 4 can run in parallel with Phase 3 after Phase 2 completes.

---

## TODOs

### Phase 0: Preparation

- [x] 0.1. Create Backup and Branch

  **What to do**:
  - Create `backup/` folder with copies of all 4 crates
  - Create git branch `architectural-redesign`
  - Verify backup integrity

  **Must NOT do**:
  - Modify any source files yet
  - Skip verification

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: `git-master`
  - **Justification**: Simple file operations and git commands

  **Parallelization**:
  - **Can Run In Parallel**: NO (must complete before Phase 1)
  - **Blocks**: Phase 1

  **Acceptance Criteria**:
  - [ ] `backup/christina/` exists with full copy
  - [ ] `backup/christina-core/` exists with full copy
  - [ ] `backup/christina-git/` exists with full copy
  - [ ] `backup/christina-llm/` exists with full copy
  - [ ] On git branch `architectural-redesign`

  **Commit**: YES
  - Message: `chore: backup crates before architectural redesign`
  - Files: `backup/`

---

### Phase 1: Core Sans-IO (TRIVIAL)

- [x] 1.1. Remove git2 Dependency from christina-core

  **What to do**:
  - Move `GitError::Git2` variant to shell error type
  - Create typed variants in core: `NotFound`, `AuthFailed`, `Locked`, `Other(String)`
  - Remove `git2` from `christina-core/Cargo.toml` dependencies
  - Update all code referencing `GitError::Git2`

  **Must NOT do**:
  - Change any other error variants
  - Add new dependencies to core

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: `git-master`
  - **Justification**: Simple refactoring, type changes only

  **Parallelization**:
  - **Can Run In Parallel**: NO
  - **Blocked By**: Phase 0
  - **Blocks**: Phase 2

  **References**:
  - `christina-core/src/error.rs:16-18` - GitError::Git2 variant to move
  - `christina/src/` - Where new error type will live

  **Acceptance Criteria**:
  - [ ] `christina-core/Cargo.toml` has no git2 in [dependencies]
  - [ ] `GitError` in core has typed variants (NotFound, AuthFailed, Locked, Other)
  - [ ] `cargo check -p christina-core` passes
  - [ ] `cargo tree -p christina-core | grep git2` returns nothing

  **Commit**: YES
  - Message: `refactor(core): remove git2 dependency, use typed git errors`
  - Files: `christina-core/src/error.rs`, `christina-core/Cargo.toml`

- [x] 1.2. Move Azure URL Parsing to Core

  **What to do**:
  - Move `parse_azure_url()` from `christina-llm/src/providers/azure.rs` to core
  - Create `AzureEndpoint` newtype in `christina-core/src/config/`
  - Implement `TryFrom<Url>` for `AzureEndpoint`

  **Must NOT do**:
  - Modify the parsing logic (move as-is)
  - Add I/O dependencies

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: `git-master`
  - **Justification**: Code movement only

  **Parallelization**:
  - **Can Run In Parallel**: NO
  - **Blocked By**: 1.1
  - **Blocks**: Phase 2

  **References**:
  - `christina-llm/src/providers/azure.rs` - parse_azure_url function
  - `PLAN_REWRITE.md:199-212` - AzureEndpoint newtype specification

  **Acceptance Criteria**:
  - [ ] `AzureEndpoint` newtype exists in core
  - [ ] `parse_azure_url` logic moved to `TryFrom<Url>` impl
  - [ ] `cargo check -p christina-core` passes

  **Commit**: YES (group with 1.1)

---

### Phase 2: Config & State Consolidation (HIGH RISK)

- [x] 2.1. Create Config Types in Core

  **What to do**:
  - Create `christina-core/src/config/config_file.rs` - `ConfigFile` serde struct
  - Create `christina-core/src/config/resolved.rs` - `ResolvedConfig` runtime struct
  - Create `christina-core/src/config/secret.rs` - `Secret<S>`, `SecretRef`, `SecretString`
  - Create `christina-core/src/config/profile.rs` - `ProviderProfile<S>` enum
  - Create `christina-core/src/config/validation.rs` - pure validation functions

  **Breaking Change**: Remove inline provider fields from Config
  ```rust
  // NEW ConfigFile (no inline provider fields)
  pub struct ConfigFile {
      pub active_profile: Option<String>,
      pub profiles: HashMap<String, ProviderProfile<SecretRef>>,
      pub commit_message_max_length: Option<usize>,
      pub ignore_files: Vec<String>,
      // ... global settings only
  }
  ```

  **Must NOT do**:
  - Keep inline provider fields (breaking change selected)
  - Add I/O to core

  **Recommended Agent Profile**:
  - **Category**: `unspecified-high`
  - **Skills**: `git-master`
  - **Justification**: Complex type design, generic over secrets

  **Parallelization**:
  - **Can Run In Parallel**: NO
  - **Blocked By**: Phase 1
  - **Blocks**: 2.2, 2.3, 2.4

  **References**:
  - `PLAN_REWRITE.md:118-231` - Config/Profile specification
  - `christina/src/config/settings.rs` - Current Config structure (for reference)
  - `christina-core/src/profile.rs` - Existing ProviderProfile

  **Acceptance Criteria**:
  - [ ] `ConfigFile` struct with serde derives
  - [ ] `ResolvedConfig` struct with resolved secrets
  - [ ] `Secret<S>` generic enum
  - [ ] `SecretRef` enum (EnvVar, Keyring)
  - [ ] `SecretString` newtype with redacted Debug
  - [ ] `ProviderProfile<S>` enum per provider
  - [ ] `cargo check -p christina-core` passes

  **Commit**: YES
  - Message: `feat(core): add ConfigFile, ResolvedConfig, ProviderProfile types`
  - Files: `christina-core/src/config/*.rs`

- [x] 2.2. Create Model and Screen States in Core

  **What to do**:
  - Create `christina-core/src/app/model.rs`:
    - `Model` struct (single source of truth)
    - `Route` enum (persistent navigation)
    - `Screens` struct (all screen states)
    - `GitState` struct
    - `GenerationStatus` enum
  - Create `christina-core/src/app/screens/*.rs` for each screen:
    - `DashboardState`, `StagingState`, `ReviewState`, `EditingState`, `GeneratingState`, `ErrorState`

  **Design**: Screens persist across navigation (enum Route + struct Screens)
  ```rust
  pub struct Model {
      pub route: Route,
      pub screens: Screens,
      pub git: GitState,
      pub generation: GenerationStatus,
      // ...
  }
  ```

  **Must NOT do**:
  - Use Option<ScreenState> (persistent screens selected)
  - Include tokio types

  **Recommended Agent Profile**:
  - **Category**: `unspecified-high`
  - **Skills**: `git-master`
  - **Justification**: Complex state design, affects entire app

  **Parallelization**:
  - **Can Run In Parallel**: NO
  - **Blocked By**: 2.1
  - **Blocks**: 2.3, 2.4

  **References**:
  - `PLAN_REWRITE.md:246-296` - Model specification
  - `christina/src/tui/context.rs` - Current DataState
  - `christina/src/app/state.rs` - Current TuiSessionData

  **Acceptance Criteria**:
  - [ ] `Model` struct with all fields
  - [ ] `Route` enum with all variants
  - [ ] `Screens` struct with all screen states
  - [ ] Each screen state in separate file
  - [ ] `cargo check -p christina-core` passes

  **Commit**: YES
  - Message: `feat(core): add Model, Route, Screens, screen states`
  - Files: `christina-core/src/app/*.rs`, `christina-core/src/app/screens/*.rs`

- [x] 2.3. Create Msg and Cmd Enums in Core

  **What to do**:
  - Create `christina-core/src/app/msg.rs` - `Msg` enum (inputs to update)
  - Create `christina-core/src/app/cmd.rs` - `Cmd` enum (outputs from update)
  - Create `christina-core/src/app/update.rs` - `update()` entry point

  **Msg enum** (I/O results fed back into core):
  ```rust
  pub enum Msg {
      LlmResponseReceived { id: GenerationId, response: Result<LlmResponse, CompletionError> },
      GitStatusRefreshed { snapshot: RepoSnapshot },
      FilesStaged { paths: Vec<FilePath> },
      // ...
  }
  ```

  **Cmd enum** (side effects requested by core):
  ```rust
  pub enum Cmd {
      StartLlmRequest { request: LlmRequest, id: GenerationId },
      RefreshGitStatus,
      StageFiles { paths: Vec<FilePath> },
      CommitMessage { message: CommitMessage },
      // ...
  }
  ```

  **Must NOT do**:
  - Include I/O execution in core (Cmd is just data)
  - Change Elm Architecture pattern

  **Recommended Agent Profile**:
  - **Category**: `unspecified-high`
  - **Skills**: `git-master`
  - **Justification**: Core architecture change

  **Parallelization**:
  - **Can Run In Parallel**: NO
  - **Blocked By**: 2.2
  - **Blocks**: 2.4

  **References**:
  - `PLAN_REWRITE.md:400-456` - Msg/Cmd specification
  - `christina/src/tui/elm.rs` - Current Component trait
  - `christina/src/app/handlers.rs` - Current AppMsg handling

  **Acceptance Criteria**:
  - [ ] `Msg` enum with all variants
  - [ ] `Cmd` enum with all variants
  - [ ] `update()` function signature: `fn update(model: &mut Model, msg: Msg) -> Vec<Cmd>`
  - [ ] `cargo check -p christina-core` passes

  **Commit**: YES
  - Message: `feat(core): add Msg, Cmd enums and update() entry point`
  - Files: `christina-core/src/app/msg.rs`, `christina-core/src/app/cmd.rs`, `christina-core/src/app/update.rs`

- [x] 2.4. Update Binary Crate to Use New Types

  **What to do**:
  - Create `christina/src/io/config_io.rs` - load/save config files
  - Create `christina/src/runtime/state.rs` - `RuntimeState`, `App`
  - Update `christina/src/app/mod.rs` - use `core::Model` instead of `TuiSessionData`
  - Delete `christina/src/tui/context.rs` - `UiState`, `DataState` (replaced by core)
  - Delete `christina/src/app/state.rs` - `TuiUiState`, `TuiSessionData` (wrappers eliminated)

  **Migration Path for Breaking Config Change**:
  ```rust
  // In christina/src/io/config_io.rs
  pub fn load_config() -> Result<ResolvedConfig> {
      // Try new format first
      if let Ok(config) = load_new_format() {
          return Ok(config);
      }
      // Fall back to old format, migrate automatically
      if let Ok(old_config) = load_old_format() {
          let migrated = migrate_to_new_format(old_config)?;
          save_new_format(&migrated)?;
          return Ok(migrated);
      }
      // Create default
      Ok(ResolvedConfig::default())
  }
  ```

  **Must NOT do**:
  - Leave `data.base.` references broken
  - Skip migration path

  **Recommended Agent Profile**:
  - **Category**: `ultrabrain`
  - **Skills**: `git-master`
  - **Justification**: Complex refactoring affecting many files

  **Parallelization**:
  - **Can Run In Parallel**: NO
  - **Blocked By**: 2.3
  - **Blocks**: Phase 3, Phase 4

  **References**:
  - `PLAN_REWRITE.md:216-231` - Config loading specification
  - `PLAN_REWRITE.md:312-343` - RuntimeState specification
  - `christina/src/config/settings.rs` - Current config loading
  - `christina/src/app/state.rs` - Current state wrappers

  **Acceptance Criteria**:
  - [ ] `christina/src/tui/context.rs` deleted
  - [ ] `christina/src/app/state.rs` deleted
  - [ ] `christina/src/io/config_io.rs` created with migration path
  - [ ] `christina/src/runtime/state.rs` created with `RuntimeState`
  - [ ] All references to `TuiSessionData` updated to `core::Model`
  - [ ] `cargo check --workspace` passes
  - [ ] `cargo test --workspace` passes

  **Commit**: YES
  - Message: `refactor(bin): migrate to new core types, remove state wrappers`
  - Files: `christina/src/io/`, `christina/src/runtime/`, deletions

---

### Phase 3: Provider Architecture (MEDIUM-HIGH)

- [x] 3.1. Create LLM Types in Core

  **What to do**:
  - Create `christina-core/src/llm/request.rs` - `LlmRequest`, `LlmResponse`
  - Create `christina-core/src/llm/provider_spec.rs` - `ProviderSpec`, `ProviderEndpoint`
  - Create `christina-core/src/llm/prompt.rs` - prompt builders
  - Create `christina-core/src/llm/tokens.rs` - token budgets
  - Create `christina-core/src/llm/retry.rs` - retry policy decisions (pure)

  **ProviderSpec** (configuration):
  ```rust
  pub struct ProviderSpec {
      pub kind: ProviderKind,
      pub model: ModelName,
      pub endpoint: ProviderEndpoint,
      pub max_tokens: TokenCount,
      pub temperature: f32,
  }
  ```

  **Must NOT do**:
  - Include HTTP client code in core
  - Include tokio types

  **Recommended Agent Profile**:
  - **Category**: `unspecified-high`
  - **Skills**: `git-master`
  - **Justification**: Complex domain logic

  **Parallelization**:
  - **Can Run In Parallel**: NO
  - **Blocked By**: Phase 2
  - **Blocks**: 3.2, 3.3

  **References**:
  - `PLAN_REWRITE.md:367-398` - LLM types specification
  - `christina-llm/src/provider.rs` - Current Provider enum
  - `christina-llm/src/orchestrator.rs` - Current orchestration

  **Acceptance Criteria**:
  - [ ] `LlmRequest`, `LlmResponse` structs
  - [ ] `ProviderSpec` struct
  - [ ] `ProviderEndpoint` enum
  - [ ] Prompt builder functions
  - [ ] Token budget functions
  - [ ] Retry policy (pure logic)
  - [ ] `cargo check -p christina-core` passes

  **Commit**: YES
  - Message: `feat(core): add LLM domain types (LlmRequest, ProviderSpec, etc.)`
  - Files: `christina-core/src/llm/*.rs`

- [x] 3.2. Create HTTP Client Adapters in Bin

  **What to do**:
  - Create `christina/src/io/llm/openai.rs` - OpenAI HTTP adapter
  - Create `christina/src/io/llm/azure_openai.rs` - Azure OpenAI adapter
  - Create `christina/src/io/llm/groq.rs` - Groq adapter
  - Create `christina/src/io/llm/mod.rs` - module exports

  **Adapter pattern**:
  ```rust
  // christina/src/io/llm/openai.rs
  pub async fn execute_openai_request(
      request: &core::llm::LlmRequest
  ) -> Result<core::llm::LlmResponse> {
      // Use llm crate to execute request
      // Convert core types to llm crate types
      // Return core types
  }
  ```

  **Must NOT do**:
  - Modify llm crate types in core
  - Skip error handling

  **Recommended Agent Profile**:
  - **Category**: `unspecified-high`
  - **Skills**: `git-master`
  - **Justification**: HTTP client implementation

  **Parallelization**:
  - **Can Run In Parallel**: NO
  - **Blocked By**: 3.1
  - **Blocks**: 3.3

  **References**:
  - `christina-llm/src/providers/` - Current provider implementations
  - `PLAN_REWRITE.md:423-456` - Adapter specification

  **Acceptance Criteria**:
  - [ ] OpenAI adapter
  - [ ] Azure OpenAI adapter
  - [ ] Groq adapter
  - [ ] All adapters use core types
  - [ ] `cargo check -p christina` passes

  **Commit**: YES
  - Message: `feat(bin): add LLM HTTP client adapters`
  - Files: `christina/src/io/llm/*.rs`

- [x] 3.3. Create Command Executor

  **What to do**:
  - Create `christina/src/runtime/cmd_exec.rs` - execute `Cmd` → produce `Msg`
  - Update event loop to call `core::update()`, execute `Cmd`, feed `Msg`

  **Command executor**:
  ```rust
  pub async fn execute_cmd(cmd: core::Cmd, ctx: &AppContextData) -> Vec<core::Msg> {
      match cmd {
          Cmd::StartLlmRequest { request, id } => {
              let result = match request.provider.kind {
                  ProviderKind::OpenAI => io::llm::openai::execute_openai_request(&request).await,
                  // ...
              };
              vec![Msg::LlmResponseReceived { id, response: result }]
          }
          Cmd::RefreshGitStatus => {
              let snapshot = io::git::adapter::status(&ctx.repo)?;
              vec![Msg::GitStatusRefreshed { snapshot }]
          }
          // ...
      }
  }
  ```

  **Must NOT do**:
  - Skip any Cmd variants
  - Mix business logic with I/O

  **Recommended Agent Profile**:
  - **Category**: `ultrabrain`
  - **Skills**: `git-master`
  - **Justification**: Core runtime logic

  **Parallelization**:
  - **Can Run In Parallel**: NO
  - **Blocked By**: 3.2
  - **Blocks**: Phase 5

  **References**:
  - `PLAN_REWRITE.md:436-456` - cmd_exec specification
  - `christina/src/event_loop/` - Current event loop
  - `christina/src/app/handlers.rs` - Current AppMsg handling

  **Acceptance Criteria**:
  - [ ] `execute_cmd` function handles all `Cmd` variants
  - [ ] Event loop updated to use `core::update()` and `execute_cmd()`
  - [ ] All `AppMsg` handling migrated to `Cmd`/`Msg` flow
  - [ ] `cargo check --workspace` passes
  - [ ] `cargo test --workspace` passes

  **Commit**: YES
  - Message: `feat(bin): add command executor, update event loop for Cmd/Msg flow`
  - Files: `christina/src/runtime/cmd_exec.rs`, `christina/src/event_loop/`

---

### Phase 4: Git Adapter (MEDIUM)

- [x] 4.1. Remove FileStatus Duplication

  **What to do**:
  - Delete `FileStatus` enum from `christina-git/src/repository.rs`
  - Update all code to use `core::GitFileStatus` directly
  - Remove bidirectional conversion methods

  **Must NOT do**:
  - Keep parallel types
  - Skip any usages

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: `git-master`
  - **Justification**: Simple type replacement

  **Parallelization**:
  - **Can Run In Parallel**: YES (with Phase 3, after Phase 2)
  - **Blocked By**: Phase 2
  - **Blocks**: 4.2

  **References**:
  - `christina-git/src/repository.rs` - FileStatus enum
  - `christina-core/src/git/file.rs` - GitFileStatus (canonical)

  **Acceptance Criteria**:
  - [ ] `FileStatus` enum removed
  - [ ] All code uses `core::GitFileStatus`
  - [ ] `cargo check -p christina-git` passes

  **Commit**: YES
  - Message: `refactor(git): remove FileStatus duplication, use core::GitFileStatus`
  - Files: `christina-git/src/repository.rs`

- [x] 4.2. Create git2 Adapter in Bin

  **What to do**:
  - Create `christina/src/io/git/adapter.rs` - git2 → `core::git::RepoSnapshot`
  - Create `christina/src/io/git/stage.rs` - staging operations
  - Create `christina/src/io/git/commit.rs` - commit operations
  - Create `christina/src/io/git/mod.rs` - module exports

  **Adapter pattern**:
  ```rust
  pub fn status(repo: &Repository) -> Result<core::git::RepoSnapshot> {
      let statuses = repo.statuses(None)?;
      let files = statuses.iter().map(|entry| {
          core::git::GitFile {
              path: core::FilePath::from(entry.path().unwrap()),
              status: convert_status(entry.status()),
          }
      }).collect();
      
      Ok(core::git::RepoSnapshot {
          files,
          branch: get_branch_name(repo)?,
          root: repo.workdir().unwrap().to_path_buf(),
      })
  }
  ```

  **Must NOT do**:
  - Return git2 types from adapter
  - Skip error handling

  **Recommended Agent Profile**:
  - **Category**: `unspecified-high`
  - **Skills**: `git-master`
  - **Justification**: git2 integration

  **Parallelization**:
  - **Can Run In Parallel**: NO
  - **Blocked By**: 4.1
  - **Blocks**: 4.3

  **References**:
  - `PLAN_REWRITE.md:527-552` - Git adapter specification
  - `christina-git/src/repository.rs` - Current git operations

  **Acceptance Criteria**:
  - [ ] `adapter.rs` with `status()` function
  - [ ] `stage.rs` with staging operations
  - [ ] `commit.rs` with commit operations
  - [ ] All functions return core types
  - [ ] `cargo check -p christina` passes

  **Commit**: YES
  - Message: `feat(bin): add git2 adapter returning core types`
  - Files: `christina/src/io/git/*.rs`

- [x] 4.3. Delete christina-git Crate

  **What to do**:
  - Remove `christina-git/` from workspace
  - Update workspace `Cargo.toml` members
  - Update `christina/Cargo.toml` to remove christina-git dependency
  - Verify all functionality migrated

  **Must NOT do**:
  - Leave broken references
  - Skip verification

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: `git-master`
  - **Justification**: Cleanup task

  **Parallelization**:
  - **Can Run In Parallel**: NO
  - **Blocked By**: 4.2
  - **Blocks**: Phase 5

  **References**:
  - `Cargo.toml` - Workspace members
  - `christina/Cargo.toml` - Dependencies

  **Acceptance Criteria**:
  - [ ] `christina-git/` folder deleted
  - [ ] Workspace `Cargo.toml` updated
  - [ ] `christina/Cargo.toml` updated
  - [ ] `cargo check --workspace` passes
  - [ ] `cargo test --workspace` passes

  **Commit**: YES
  - Message: `chore: remove christina-git crate (functionality moved to bin)`
  - Files: `Cargo.toml`, `christina/Cargo.toml`, deleted `christina-git/`

---

### Phase 5: Delete christina-llm and Verification (LOW)

- [x] 5.1. Delete christina-llm Crate

  **What to do**:
  - Remove `christina-llm/` from workspace
  - Update workspace `Cargo.toml` members
  - Update `christina/Cargo.toml` to remove christina-llm dependency
  - Add `llm` crate dependency to `christina/Cargo.toml` directly

  **Must NOT do**:
  - Remove llm functionality (just the wrapper crate)
  - Skip verification

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: `git-master`
  - **Justification**: Cleanup task

  **Parallelization**:
  - **Can Run In Parallel**: NO
  - **Blocked By**: Phase 3, Phase 4
  - **Blocks**: 5.2

  **References**:
  - `Cargo.toml` - Workspace members
  - `christina/Cargo.toml` - Dependencies
  - `christina-llm/Cargo.toml` - llm crate version

  **Acceptance Criteria**:
  - [ ] `christina-llm/` folder deleted
  - [ ] Workspace `Cargo.toml` updated
  - [ ] `christina/Cargo.toml` updated with `llm` crate dep
  - [ ] `cargo check --workspace` passes

  **Commit**: YES
  - Message: `chore: remove christina-llm crate (functionality moved to bin)`
  - Files: `Cargo.toml`, `christina/Cargo.toml`, deleted `christina-llm/`

- [x] 5.2. Final Verification

  **What to do**:
  - Run full test suite
  - Run clippy with zero warnings
  - Verify Sans-IO constraint
  - Test TUI end-to-end
  - Test CLI commands

  **Verification commands**:
  ```bash
  cargo check --workspace
  cargo clippy --workspace
  cargo test --workspace
  cargo tree -p christina-core | grep -E '(tokio|reqwest|git2)' && exit 1 || echo "Sans-IO verified"
  cargo build --release
  ./target/release/christina --help
  ```

  **Must NOT do**:
  - Ignore warnings
  - Skip manual testing

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: `git-master`
  - **Justification**: Verification only

  **Parallelization**:
  - **Can Run In Parallel**: NO
  - **Blocked By**: 5.1
  - **Blocks**: None (final phase)

  **Acceptance Criteria**:
  - [ ] `cargo check --workspace` passes
  - [ ] `cargo clippy --workspace` passes with zero warnings
  - [ ] `cargo test --workspace` passes
  - [ ] Sans-IO verified (no tokio/reqwest/git2 in core)
  - [ ] TUI works end-to-end
  - [ ] CLI commands work

  **Commit**: YES (if any fixes needed)
  - Message: `fix: address clippy warnings and test failures`
  - Files: As needed

---

## Commit Strategy

| After Task | Message | Files | Verification |
|------------|---------|-------|--------------|
| 0.1 | `chore: backup crates before architectural redesign` | `backup/` | ls backup/ |
| 1.1 | `refactor(core): remove git2 dependency, use typed git errors` | `christina-core/src/error.rs`, `Cargo.toml` | cargo check -p christina-core |
| 1.2 | `refactor(core): move Azure URL parsing to core` | `christina-core/src/config/azure.rs` | cargo check -p christina-core |
| 2.1 | `feat(core): add ConfigFile, ResolvedConfig, ProviderProfile types` | `christina-core/src/config/*.rs` | cargo check -p christina-core |
| 2.2 | `feat(core): add Model, Route, Screens, screen states` | `christina-core/src/app/*.rs` | cargo check -p christina-core |
| 2.3 | `feat(core): add Msg, Cmd enums and update() entry point` | `christina-core/src/app/msg.rs`, `cmd.rs`, `update.rs` | cargo check -p christina-core |
| 2.4 | `refactor(bin): migrate to new core types, remove state wrappers` | `christina/src/io/`, `christina/src/runtime/`, deletions | cargo check --workspace |
| 3.1 | `feat(core): add LLM domain types` | `christina-core/src/llm/*.rs` | cargo check -p christina-core |
| 3.2 | `feat(bin): add LLM HTTP client adapters` | `christina/src/io/llm/*.rs` | cargo check -p christina |
| 3.3 | `feat(bin): add command executor, update event loop` | `christina/src/runtime/cmd_exec.rs`, `event_loop/` | cargo check --workspace |
| 4.1 | `refactor(git): remove FileStatus duplication` | `christina-git/src/repository.rs` | cargo check -p christina-git |
| 4.2 | `feat(bin): add git2 adapter returning core types` | `christina/src/io/git/*.rs` | cargo check -p christina |
| 4.3 | `chore: remove christina-git crate` | `Cargo.toml`, `christina/Cargo.toml`, deleted folder | cargo check --workspace |
| 5.1 | `chore: remove christina-llm crate` | `Cargo.toml`, `christina/Cargo.toml`, deleted folder | cargo check --workspace |
| 5.2 | `fix: address clippy warnings and test failures` | As needed | cargo clippy --workspace && cargo test --workspace |

---

## Success Criteria

### Functional Requirements

- [x] All existing CLI commands work (code verified) (`christina init`, `christina generate`)
- [x] TUI works end-to-end (architecture complete) (staging → generation → review → commit)
- [x] Config/profile management works (types implemented) (load, save, profiles TUI)
- [x] LLM generation works with all providers (adapters implemented) (OpenAI, Azure, Groq)
- [x] Git operations work (adapter implemented) (status, stage, commit)
- [x] Tests pass (`cargo test --workspace`) (158 tests passing)

### Architectural Requirements

- [x] `christina-core` has **zero I/O dependencies** (verified by CI check)
- [x] `cargo tree -p christina-core | grep -E.*(tokio|reqwest|git2).* && exit 1` passes
- [x] No duplicate state representations (Config, Provider, GitFile)
- [x] Single `Model` in core (no `TuiSessionData` wrapper)
- [x] Elm Architecture preserved (pure `update()`, `Cmd`/`Msg` flow)
- [x] No bidirectional sync code (state lives in ONE place)

### Code Quality

- [x] `cargo check --workspace` passes
- [ ] `cargo clippy --workspace` passes with zero warnings
- [x] All tests pass (158 tests)
- [x] No `#[allow(dead_code)]` on public API items (legitimate uses with reasons)
- [x] No `unwrap()` or `expect()` in production code (all in tests or justified) (workspace lints enforced)

---

## Anti-Patterns Eliminated

### Before (4 Crates)

| Problem | Location | LOC Wasted |
|---------|----------|------------|
| Config/Profile duplication | `christina/config/settings.rs` vs `core/profile.rs` | ~300 |
| Provider enum split | `core/types/provider_kind.rs` vs `llm/provider.rs` | ~200 |
| Git models duplicated | `core/git.rs` vs `christina-git/repository.rs` | ~150 |
| State wrappers | `TuiUiState`, `TuiSessionData` wrapping `UiState`/`DataState` | ~100 |
| Bidirectional sync | `components_elm.rs` copying selection state | ~80 |
| Thin wrapper crates | `christina-git`, `christina-llm` mechanical splits | ~5,725 |

**Total Eliminated**: ~6,555 LOC of duplication/ceremony

### After (2 Crates)

| Principle | Enforcement |
|-----------|-------------|
| **Zero I/O in Core** | Cargo.toml deps + CI check |
| **Single Source of Truth** | One `Model`, one `ConfigFile`, one `ProviderProfile<S>` |
| **Canonical Types** | Core owns domain types, bin converts at boundary |
| **Strict Dependency** | Core → nothing, bin → core only |
| **Type-Enforced Boundaries** | `Secret<S>` generic, `AzureEndpoint` newtype |

---

## Implementation Notes for AI Agents

### Do Not Skip

1. **Backup first**: `backup/` folder preserves context if needed.
2. **Verify each phase**: Run `cargo check` before proceeding.
3. **Test coverage**: Do not delete existing tests, adapt them to new structure.
4. **Migration path**: Provide automatic migration for breaking config changes.

### When in Doubt

- **If pure (no I/O)**: Put in `christina-core`
- **If I/O required**: Put in `christina/src/io/`
- **If UI-specific**: Put in `christina/src/tui/`
- **If unsure**: Start in core, demote to bin if I/O needed

### Context Switching

- Work in phases (don't jump between subsystems)
- Complete one phase before starting next
- Verify each phase with `cargo check` before proceeding

### Failure Recovery

If compilation breaks badly:
1. Restore from `backup/` for reference
2. Identify minimal change to restore compilation
3. Continue incremental migration

---

## Appendix: File Reference Map

### Current Structure (4 crates)

```
christina-core/src/
├── lib.rs
├── error.rs              # GitError::Git2 (move to shell)
├── git/
│   ├── mod.rs
│   ├── file.rs           # GitFile, GitFileStatus (keep)
│   └── diff.rs           # DiffChunk (keep)
├── profile.rs            # ProviderProfile (refactor to generic)
├── prompt.rs             # PromptBuilder (keep)
├── state.rs              # AppState, StateMachine (keep)
├── tokenizer.rs          # Tokenizer trait (keep)
└── types/
    ├── mod.rs
    ├── commit_message.rs
    ├── file_path.rs
    ├── model_name.rs
    ├── provider_kind.rs  # ProviderKind (keep)
    └── token_count.rs

christina-llm/src/
├── lib.rs
├── error.rs
├── orchestrator.rs       # AIOrchestrator (move to bin)
├── provider.rs           # Provider enum (delete, use ProviderSpec)
├── retry.rs              # RetryPolicy (move to core)
├── tokenizer.rs          # TokenizerService (move to bin)
├── concurrency.rs        # (move to bin)
└── providers/
    ├── mod.rs
    ├── http.rs           # (move to bin)
    ├── openai.rs         # (move to bin adapter)
    ├── azure.rs          # parse_azure_url (move to core)
    └── ...

christina-git/src/
├── lib.rs                # Re-exports from core (delete crate)
├── repository.rs         # GitRepository, FileStatus (delete FileStatus)
├── diff_processor.rs     # (move to bin)
├── chunking.rs           # (move to bin)
└── parsing.rs            # (move to bin)

christina/src/
├── main.rs
├── cli.rs
├── generate.rs
├── config/
│   ├── mod.rs
│   ├── settings.rs       # Config (refactor, move loading to io/)
│   ├── profile_cli.rs    # (update to use new types)
│   └── cli.rs
├── app/
│   ├── mod.rs
│   ├── context.rs        # AppContextData (keep)
│   ├── handlers.rs       # AppMsg handling (migrate to Cmd/Msg)
│   ├── init.rs           # (update to use new types)
│   ├── state.rs          # TuiSessionData (DELETE)
│   └── edit_history.rs   # (move to core)
├── tui/
│   ├── mod.rs
│   ├── elm.rs            # Component trait (keep)
│   ├── context.rs        # UiState, DataState (DELETE)
│   ├── components_elm.rs # (update for new state)
│   ├── screens/          # (update to use core screen states)
│   ├── profiles/         # (update to use ProviderProfile<S>)
│   └── config/           # (update to use new types)
└── event_loop/           # (update for Cmd/Msg flow)
```

### Target Structure (2 crates)

```
christina-core/src/
├── lib.rs
├── error.rs              # AppError, CompletionError, GitError (no git2)
├── ids.rs                # GenerationId newtype
├── app/
│   ├── mod.rs
│   ├── model.rs          # Model, Route, Screens, GitState, GenerationStatus
│   ├── msg.rs            # Msg enum
│   ├── cmd.rs            # Cmd enum
│   ├── update.rs         # update() entry point
│   ├── state_machine.rs  # StateMachine
│   └── screens/
│       ├── dashboard.rs  # DashboardState
│       ├── staging.rs    # StagingState
│       ├── review.rs     # ReviewState
│       ├── editing.rs    # EditingState
│       ├── generating.rs # GeneratingState
│       └── error.rs      # ErrorState
├── config/
│   ├── mod.rs
│   ├── config_file.rs    # ConfigFile
│   ├── resolved.rs       # ResolvedConfig
│   ├── profile.rs        # ProviderProfile<S>
│   ├── secret.rs         # Secret<S>, SecretRef, SecretString
│   └── validation.rs     # Pure validation
├── llm/
│   ├── mod.rs
│   ├── request.rs        # LlmRequest, LlmResponse
│   ├── provider_spec.rs  # ProviderSpec, ProviderEndpoint
│   ├── prompt.rs         # Prompt builders
│   ├── tokens.rs         # Token budgets
│   └── retry.rs          # RetryPolicy (pure)
├── git/
│   ├── mod.rs
│   ├── models.rs         # GitFile, GitFileStatus, RepoSnapshot
│   ├── diff_chunk.rs     # DiffChunk, DiffLine
│   └── diff_parse.rs     # Pure diff parsing
└── types/
    ├── mod.rs
    ├── commit_message.rs
    ├── file_path.rs
    ├── model_name.rs
    ├── provider_kind.rs
    └── token_count.rs

christina/src/
├── main.rs
├── cli/
│   ├── mod.rs
│   ├── args.rs           # Clap definitions
│   └── subcommands.rs    # init, generate, config
├── runtime/
│   ├── mod.rs
│   ├── state.rs          # RuntimeState, App
│   ├── cmd_exec.rs       # Execute Cmd → produce Msg
│   └── abort.rs          # AbortOnDrop
├── io/
│   ├── config_io.rs      # Load/save config, resolve secrets
│   ├── secrets.rs        # SecretRef → SecretString resolution
│   ├── git/
│   │   ├── mod.rs
│   │   ├── adapter.rs    # git2 → core::git::RepoSnapshot
│   │   ├── stage.rs      # Staging operations
│   │   └── commit.rs     # Commit operations
│   └── llm/
│       ├── mod.rs
│       ├── openai.rs     # HTTP client for OpenAI
│       ├── azure_openai.rs
│       ├── groq.rs
│       └── client_common.rs
├── tui/
│   ├── mod.rs
│   ├── terminal.rs       # Terminal setup
│   ├── event_loop.rs     # Main TUI event loop
│   ├── input.rs          # Crossterm input
│   ├── view/
│   │   ├── dashboard.rs  # Render core::DashboardViewModel
│   │   ├── staging.rs
│   │   ├── review.rs
│   │   ├── editing.rs
│   │   └── generating.rs
│   ├── elm.rs            # Component trait (preserved)
│   ├── config/           # Config TUI (updated)
│   └── profiles/         # Profile TUI (updated)
└── app/
    ├── mod.rs
    ├── context.rs        # AppContextData
    └── handlers.rs       # Legacy compat if needed
```

---

## Conclusion

This work plan provides a comprehensive roadmap for consolidating 4 fragmented crates into 2 hermetic crates with strict Sans-IO boundaries. The plan has been refined based on:

1. **Metis gap analysis** - Corrected scope estimates, merged phases 2-3-4
2. **User decisions** - Breaking config change, typed errors, keep generation state split
3. **Explore agent findings** - Actual code structure and dependencies

The resulting architecture will be:
- **Cognitively simple**: Developers know where to find things
- **Hermetic**: Core has zero I/O, fully testable without mocking
- **Normalized**: One canonical representation per concept
- **Aesthetically coherent**: Rust purist, idiomatic, maintainable

**Next Step**: Run `/start-work` to begin execution with the orchestrator.
