# Christina Workspace Architectural Rewrite Plan

## Critical Corrections from Adversarial Review

**Status**: Second draft after adversarial self-review  
**Date**: January 2026  
**Total Lines**: ~17,500 across 4 crates (using `tokei`)
**Target**: 2 crates (christina binary, christina-core library)

---

## Issues Found in First Draft

### 1. **EditHistory is ACTIVELY USED** ❌
- **Location**: `christina/src/app/edit_history.rs` (171 lines)
- **Usage**: `editing.rs` uses undo/redo (Ctrl+Z/Y) with 50-entry history
- **Original plan**: Incorrectly stated "Not used in current UI"
- **Correction**: Must preserve in binary crate as part of editing screen state

### 2. **Elm Architecture is Substantial and Well-Designed** ❌
- **Location**: `christina/src/tui/elm.rs` (77 lines), `components_elm.rs` (323 lines)
- **Pattern**: Clean Component trait + AppMsg enum for side-effect routing
- **Original plan**: Dismissed as "not needed" - WRONG
- **Correction**: Preserve this pattern; it provides excellent separation of concerns
- **Why**: 323-line routing system cleanly dispatches between 6 screens with state sync

### 3. **DiffRenderer is Real Functionality** ❌
- **Location**: `christina/src/tui/diff_renderer.rs`
- **Usage**: Integrated into DashboardState for diff preview with tool detection
- **Original plan**: Said "inline where used"
- **Correction**: Keep as separate module; handles delta/diff-so-fancy/git/basic tool chain

### 4. **GPG Signing Major Feature - Completely Missed** ❌❌❌
- **Location**: `christina-git/src/repository.rs` lines 288-408 (120 lines)
- **Functionality**: Full GPG commit signing with configurable program
- **Original plan**: Not mentioned at all
- **Correction**: Explicitly preserve; add to git module in library

### 5. **Configuration TUI is Complex Multi-Screen Flow** ❌
- **Location**: `christina/src/tui/config/` (4 files), `christina/src/tui/profiles/` (5 files)
- **Original plan**: "Merge into single screens/config.rs"
- **Reality**: Nested TUI flow with profile management integration
- **Correction**: Keep as dedicated module structure but flatten slightly

### 6. **Inconsistent Naming** ❌
- **Issue**: Plan used `christina-lib` and `christina-core` interchangeably
- **Correction**: Use `christina-core` consistently (renaming existing core)

### 7. **StateMachine Has Async Correlation Logic** ❌
- **Location**: `christina-core/src/state.rs` (520 lines with tests)
- **Function**: Generation ID tracking for async operation correlation
- **Original plan**: "Inline into app.rs"
- **Correction**: Keep as dedicated type; critical for generation cancellation

### 8. **Missing Systems** ❌
- **Theme system**: `christina/src/tui/theme.rs` - extensive color constants
- **Toast system**: `christina/src/tui/widgets/toast.rs` - ToastManager with queue
- **Form system**: `christina/src/tui/form/` - Editable trait for config/profile TUI

### 9. **Test Strategy Was Risky** ❌
- **Original plan**: "Remove tests to focus on rewrite"
- **Correction**: Keep and migrate tests incrementally; 1000+ lines of tests validate behavior

### 10. **Config::load/save Has I/O - Wrong Location** ❌
- **Issue**: Config has file I/O methods but plan put Config in library
- **Correction**: 
  - Library: `Config` struct definition, validation
  - Binary: Config I/O functions (`load()`, `save()`)

---

## Corrected Architecture

### Crate: `christina` (Binary)

**Responsibility**: ALL user-facing code, I/O, rendering, event handling

**Why binary has I/O**: Config loading, file operations, terminal interaction are inherently I/O-bound

**Strict Ownership**:
1. **CLI parsing** (`cli.rs`) - argument definitions and dispatch
2. **TUI screens** (`tui/screens/`) - 6 screen implementations
3. **TUI components** (`tui/components/`) - reusable widgets
4. **Event loop** (`tui/event_loop.rs`) - async event coordination
5. **App state** (`tui/app.rs`) - central state with Elm dispatch
6. **Config I/O** (`config/io.rs`) - file loading/saving (NEW module)
7. **CLI commands** (`cli_commands/`) - non-TUI command handlers
8. **Terminal management** (`tui/terminal.rs`) - raw mode handling

**Dependencies**:
- `christina-core` (domain library)
- `ratatui`, `crossterm`, `tui-textarea` (TUI)
- `clap` (CLI)
- `tokio` (async runtime)
- `directories`, `toml`, `config` (config I/O)
- `ansi-to-tui` (diff rendering)

### Crate: `christina-core` (Library)

**Responsibility**: Domain types, pure logic, NO I/O

**Strict Ownership**:
1. **Types** (`types/`) - CommitMessage, ProviderKind, TokenCount, FilePath
2. **Config structures** (`config/`) - Config struct, Profiles, validation logic
3. **Git operations** (`git/`) - Repository, staging, committing, GPG signing
4. **LLM orchestration** (`llm/`) - Provider trait, OpenAI/Azure, orchestrator
5. **Generation** (`generation/`) - GenerationService facade
6. **Errors** (`error.rs`) - unified error types

**Dependencies**:
- `git2` (git operations)
- `llm`, `tiktoken-rs` (AI/LLM)
- `serde` (serialization)
- `thiserror`, `anyhow` (errors)
- `url`, `regex`, `compact_str` (utilities)
- `tokio` (async traits)

---

## Corrected Directory Structure

### `christina/` (Binary)

```
christina/
├── Cargo.toml
└── src/
    ├── main.rs                 # Entry point, allocator, panic handling
    ├── cli.rs                  # Clap argument definitions
    ├── config/
    │   ├── mod.rs              # Config module exports
    │   └── io.rs               # FILE I/O: load(), save(), paths (MOVED from lib)
    ├── cli_commands/
    │   ├── mod.rs              # Command exports
    │   ├── config.rs           # config get/set/list/path/tui
    │   └── profile.rs          # profile create/edit/delete/switch
    └── tui/
        ├── mod.rs              # TUI exports (theme, widgets)
        ├── app.rs              # App struct + AppMsg dispatch (was app/handlers.rs + app/mod.rs)
        ├── event_loop.rs       # Event loop (was event_loop/mod.rs + handlers + producers)
        ├── terminal.rs         # Terminal init/cleanup (was bootstrap/terminal.rs)
        ├── theme.rs            # Color constants (PRESERVE)
        ├── elm.rs              # Component trait + AppMsg (PRESERVE - well designed)
        ├── screens/            # Screen implementations (6 screens)
        │   ├── mod.rs          # Screen exports + key routing
        │   ├── staging.rs      # Staging selection screen (735 lines)
        │   ├── dashboard.rs    # Dashboard with diff preview
        │   ├── generating.rs   # Generation progress
        │   ├── review.rs       # Review generated message
        │   ├── editing.rs      # Edit message with undo/redo
        │   └── error.rs        # Error display (modal)
        ├── components/         # Reusable widgets
        │   ├── mod.rs          # Component exports
        │   ├── file_list.rs    # File list widget
        │   └── diff_renderer.rs # Diff preview with tool detection (PRESERVE)
        ├── widgets/            # Low-level widgets
        │   ├── mod.rs
        │   └── toast.rs        # Toast notification system (PRESERVE)
        ├── config_tui/         # Configuration TUI (PRESERVE complexity)
        │   ├── mod.rs
        │   ├── app.rs
        │   ├── runner.rs
        │   └── screen.rs
        └── profiles_tui/       # Profile management TUI
            ├── mod.rs
            ├── app.rs
            ├── runner.rs
            ├── screen.rs
            └── profile_editable.rs
```

### `christina-core/` (Library) - RENAMED from current core

```
christina-core/
├── Cargo.toml
└── src/
    ├── lib.rs                  # Public API exports
    ├── error.rs                # Unified errors
    ├── types/
    │   ├── mod.rs              # Type re-exports
    │   ├── commit.rs           # CommitMessage + ValidationMode
    │   ├── provider.rs         # ProviderKind enum
    │   ├── model.rs            # ModelName newtype
    │   ├── tokens.rs           # TokenCount newtype
    │   └── file.rs             # FilePath newtype
    ├── config/
    │   ├── mod.rs              # Config struct DEFINITION (no I/O)
    │   ├── profile.rs          # ProviderProfile, Profiles
    │   └── validation.rs       # Validation logic (MOVED from settings.rs)
    ├── git/
    │   ├── mod.rs              # Git exports
    │   ├── repo.rs             # Repository struct (was repository.rs)
    │   ├── operations.rs       # Stage/unstage/commit + GPG signing
    │   ├── diff.rs             # Diff generation, StagedDiff
    │   └── status.rs           # GitFile, GitFileStatus
    ├── llm/
    │   ├── mod.rs              # LLM exports
    │   ├── provider.rs         # Provider trait + impls (was provider.rs + providers/)
    │   ├── orchestrator.rs     # Map-reduce generation (PRESERVE - 1287 lines)
    │   ├── prompt.rs           # Prompt templates (was christina-core/src/prompt.rs)
    │   ├── tokenizer.rs        # Token counting
    │   └── retry.rs            # Retry policies
    └── generation/
        ├── mod.rs              # Generation exports
        └── service.rs          # GenerationService facade
```

---

## Module Migration Map

### PRESERVE (Don't Delete)

| Module | Lines | Reason |
|--------|-------|--------|
| `christina/src/app/edit_history.rs` | 171 | **CRITICAL**: Undo/redo for editing screen |
| `christina/src/tui/elm.rs` | 77 | **WELL-DESIGNED**: Component trait + AppMsg |
| `christina/src/tui/components_elm.rs` | 323 | **ROUTER**: Screen dispatch + state sync |
| `christina/src/tui/diff_renderer.rs` | ~100 | **REAL**: Delta/diff-so-fancy tool chain |
| `christina/src/tui/theme.rs` | ~80 | **STYLING**: Color palette constants |
| `christina/src/tui/widgets/toast.rs` | ~150 | **UI**: Toast notification queue |
| `christina/src/tui/form/` | ~200 | **FORMS**: Editable trait for config TUI |
| `christina/src/tui/config_tui/` | ~400 | **COMPLEX**: Multi-screen config flow |
| `christina/src/tui/profiles_tui/` | ~500 | **COMPLEX**: Profile management |
| `christina-core/src/state.rs` | 520 | **ASYNC**: Generation ID correlation |

### MERGE/CONSOLIDATE

| From | Into | Notes |
|------|------|-------|
| `christina/src/app/mod.rs` + `handlers.rs` | `christina/src/tui/app.rs` | Merge App struct with dispatch |
| `christina/src/app/init.rs` | `christina/src/tui/app.rs` | `initialize_app()` function |
| `christina/src/app/context.rs` | `christina/src/tui/app.rs` | Inline AppContext fields |
| `christina/src/app/state.rs` | `christina/src/tui/app.rs` | Inline GenerationState enum |
| `christina/src/event_loop/` (3 files) | `christina/src/tui/event_loop.rs` | ~250 lines total |
| `christina/src/bootstrap/` | `christina/src/tui/terminal.rs` | Single terminal module |
| `christina/src/config/settings.rs` | Split: `christina/src/config/io.rs` + `christina-core/src/config/mod.rs` | I/O in binary, struct in lib |
| `christina/src/config/cli.rs` | `christina/src/cli_commands/config.rs` | CLI command handlers |
| `christina/src/config/profile_cli.rs` | `christina/src/cli_commands/profile.rs` | CLI command handlers |
| `christina-git/src/` (5 files) | `christina-core/src/git/` | Merge into library |
| `christina-llm/src/` (8 files) | `christina-core/src/llm/` | Merge into library |

### DELETE

| Module | Reason |
|--------|--------|
| `christina-git/src/buffer_pool.rs` | Premature optimization, unused |
| `christina-llm/src/providers/mod.rs` | Flatten into provider.rs |
| `christina-llm/src/concurrency.rs` | Merge into orchestrator.rs |
| `christina-llm/src/providers/http.rs` | Inline into provider.rs |

---

## Corrected Data Flow

### Binary Crate (`christina`)

```rust
// christina/src/tui/app.rs
pub struct App {
    // Navigation
    pub state: AppState,              // From christina_core
    pub state_machine: StateMachine,  // From christina_core
    
    // Repository (I/O handles stay in binary)
    pub repo: Option<GitRepository>,  // From christina_core
    pub branch_name: Option<CompactString>,
    
    // Data
    pub staged_files: Vec<GitFile>,   // From christina_core
    pub unstaged_files: Vec<GitFile>,
    pub generated_message: CompactString,
    pub user_context: Option<String>,
    
    // UI State
    pub should_quit: bool,
    pub exit_message: Option<String>,
    pub spinner_frame: usize,
    pub should_redraw: bool,
    pub textarea: TextArea<'static>,
    
    // Screen-specific states (lazy init)
    pub dashboard_state: Option<DashboardState>,
    pub staging_state: Option<StagingState>,
    pub review_state: Option<ReviewState>,
    pub editing_state: Option<EditingState>,
    pub generating_state: Option<GeneratingState>,
    pub error_state: Option<ErrorState>,
    
    // Async
    pub generation: GenerationState,  // Running task handle
    
    // UI Components
    pub toasts: ToastManager,
    pub edit_history: EditHistory,    // **PRESERVED**
}

// Config I/O is in binary, not library
// christina/src/config/io.rs
pub fn load_config() -> Result<Config> { ... }
pub fn save_config(config: &Config) -> Result<()> { ... }
```

### Library Crate (`christina-core`)

```rust
// christina-core/src/config/mod.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub max_input_tokens: TokenCount,
    pub max_output_tokens: TokenCount,
    pub model_provider: ProviderKind,
    pub model: ModelName,
    pub api_key: Option<String>,
    pub api_url: Option<Url>,
    pub temperature: f32,
    pub ignore_files: Vec<String>,
    pub profiles: Profiles,
    pub diff: DiffConfig,
    pub commit_message_max_length: Option<usize>,
    pub commit_message_validation_mode: ValidationMode,
    pub use_commit_history: bool,
    pub commit_history_depth: usize,
    pub azure_api_version: Option<String>,
    pub azure_deployment_id: Option<String>,
}

// NO load() or save() methods - those are I/O

// christina-core/src/git/repo.rs
pub struct Repository {
    inner: git2::Repository,
}

impl Repository {
    pub fn discover() -> Result<Self>;
    pub fn open(path: &Path) -> Result<Self>;
    pub fn staged_files(&self) -> Result<Vec<GitFile>>;
    pub fn unstaged_files(&self) -> Result<Vec<GitFile>>;
    pub fn stage_files(&self, files: &[(PathBuf, GitFileStatus)]) -> Result<()>;
    pub fn unstage_files(&self, paths: &[PathBuf]) -> Result<()>;
    pub fn create_commit(&self, message: &CommitMessage) -> Result<Oid>; // **INCLUDES GPG**
    pub fn commit_history(&self, limit: usize) -> Result<Vec<CommitInfo>>;
    pub fn has_staged_changes(&self) -> Result<bool>;
    pub fn validate_for_commit(&self) -> Result<()>;
}

// christina-core/src/generation/service.rs
pub struct GenerationService {
    config: Config,
    provider: Arc<dyn Provider>,
}

pub struct GenerationResult {
    pub message: CommitMessage,
    pub warnings: Vec<String>,
    pub truncated: bool,
    pub salvaged: bool,
}

impl GenerationService {
    pub fn new(config: Config) -> Result<Self>;
    
    pub async fn generate(
        &self,
        repo_path: &Path,
        user_context: Option<&str>,
        progress: mpsc::Sender<GenerationProgress>,
    ) -> Result<GenerationResult>;
}

// christina-core/src/state.rs
pub struct StateMachine {
    generation_id: u64,
}

impl StateMachine {
    pub fn new() -> Self;
    pub fn next_generation_id(&mut self) -> u64;
    pub fn can_transition(&self, from: &AppState, to: &AppState) -> Result<(), TransitionError>;
}
```

---

## Anti-Patterns Being Eliminated (Corrected)

### 1. **Over-Fragmentation** ✅
**Before**: 4 crates, 40+ modules, some 50-line modules  
**After**: 2 crates, ~25 modules, minimum 100 lines per module

**Deletions justified**:
- `buffer_pool.rs` - 0 usages, premature optimization
- `bootstrap/` module - 20 lines of terminal setup
- `event_loop/` directory - 3 files for 250 lines

### 2. **Leaky Abstractions** ✅
**Before**: Binary imported `git2` directly for error handling  
**After**: Binary only uses `christina_core::git::Repository`

### 3. **Misplaced I/O** ✅
**Before**: `Config::load()` and `Config::save()` in library crate  
**After**: Config struct in library, I/O functions in binary

### 4. **Duplicate State Representations** ✅
**Before**: `AppState` (enum) + `Screen` (conceptual) + multiple "context" types  
**After**: Single `AppState` enum from library used throughout

---

## Hard Trade-offs Explicitly Acknowledged

### 1. **Config I/O Split**
- **Trade-off**: Config struct definition in library, I/O in binary
- **Why**: Library should be hermetic, but loading needs directories/filesystem
- **Risk**: Two imports needed (`christina_core::Config` + `christina::config::load`)
- **Mitigation**: Re-export in binary: `pub use christina::config::{Config, load_config}`

### 2. **EditHistory Location**
- **Trade-off**: EditHistory is pure logic but only used by editing screen
- **Decision**: Keep in binary crate as part of TUI state
- **Rationale**: Not reusable domain logic; tightly coupled to text editing UX

### 3. **StateMachine in Library**
- **Trade-off**: StateMachine validates transitions but needs AppState from library
- **Decision**: Keep in library; binary calls `can_transition()` before state changes
- **Rationale**: Transition rules are domain logic, not UI logic

### 4. **GPG Signing in Library**
- **Trade-off**: GPG requires spawning external process (gpg binary)
- **Decision**: Keep in library's `git::Repository::create_commit()`
- **Rationale**: Signing is part of git commit operation; caller shouldn't care how

### 5. **Elm Architecture Preservation**
- **Trade-off**: Adds ~400 lines vs direct event handling
- **Decision**: Preserve Component trait + AppMsg pattern
- **Rationale**: Clean separation of pure state updates from side effects; easy to test

---

## Migration Phases (Revised)

### Phase 1: Consolidate Library (Week 1)

**Goal**: Create unified `christina-core` library

1. Merge `christina-git/` into `christina-core/src/git/`
   - Move `repository.rs` → `git/repo.rs`
   - Move chunking logic → `git/chunk.rs`
   - **Preserve GPG signing logic**
   
2. Merge `christina-llm/` into `christina-core/src/llm/`
   - Flatten `providers/` into `provider.rs`
   - **Preserve orchestrator.rs (1,287 lines)**
   - Move retry.rs, tokenizer.rs
   
3. Move types to `christina-core/src/types/`
   - Flatten `types/` files (commit.rs not commit_message.rs)
   
4. Create `christina-core/src/generation/service.rs`
   - Facade combining diff processing + LLM orchestration
   
5. Update `christina-core/Cargo.toml`
   - Add `git2`, `llm`, `tiktoken-rs` deps
   - Remove circular deps

**Quality gate**: `cd christina-core && cargo check` passes

### Phase 2: Refactor Binary (Week 1-2)

**Goal**: Simplify binary crate, use unified library

1. Create `christina/src/config/io.rs`
   - Move `Config::load()` from settings.rs
   - Move `Config::save_to_global()` from settings.rs
   - Move path resolution logic
   
2. Create `christina/src/tui/app.rs`
   - Merge `app/mod.rs` + `handlers.rs` + `init.rs`
   - Flatten `AppContextData` into App struct fields
   - **Preserve EditHistory usage**
   
3. Create `christina/src/tui/event_loop.rs`
   - Merge `event_loop/` directory
   - Replace inline generation with `GenerationService`
   
4. Update screens
   - Keep all 6 screens (staging, dashboard, generating, review, editing, error)
   - **Preserve Elm Component implementations**
   - **Preserve diff_renderer usage in dashboard**
   
5. Update `Cargo.toml`
   - Remove `christina-git`, `christina-llm` deps
   - Keep `christina-core` path dep
   - Add `ansi-to-tui` for diff rendering

**Quality gate**: `cd christina && cargo check` passes

### Phase 3: Delete Old Crates (Week 2)

1. Delete `christina-git/` directory
2. Delete `christina-llm/` directory  
3. Remove old `christina-core/` (if created new one)
4. Update workspace `Cargo.toml`:
   ```toml
   members = ["christina", "christina-core"]
   ```

### Phase 4: Quality Gates (Week 2)

```bash
# Run until clean
just check    # cargo check with zero warnings
just clippy   # cargo clippy with zero warnings
```

### Phase 5: Verification (Week 3)

**Functional testing**:
- [ ] Config commands: get, set, list, path, tui
- [ ] Profile commands: create, edit, delete, switch, list
- [ ] TUI launches from git repo
- [ ] Staging screen: navigate, select, stage files
- [ ] Dashboard: view files, diff preview, user context
- [ ] Generation: trigger, progress display, cancellation
- [ ] Review: accept, edit, regenerate
- [ ] Editing: type, undo (Ctrl+Z), redo (Ctrl+Y), save
- [ ] Commit: creates commit with proper message
- [ ] GPG signing: works if commit.gpgsign=true
- [ ] Error handling: displays errors properly

**Regression testing**:
- [ ] Large diffs (>100KB) handled
- [ ] Binary files detected and excluded
- [ ] Delta/diff-so-fancy rendering works
- [ ] Toast notifications display
- [ ] Multi-select mode in dashboard

---

## Success Criteria (Revised)

1. ✅ Exactly 2 crates: `christina` (binary), `christina-core` (library)
2. ✅ Zero compiler warnings (`just check` passes)
3. ✅ Zero clippy warnings (`just clippy` passes)
4. ✅ **All existing tests pass** (not removed!)
5. ✅ Binary uses library for ALL domain logic
6. ✅ Library has zero terminal/UI dependencies
7. ✅ **EditHistory preserved and functional**
8. ✅ **GPG signing preserved and functional**
9. ✅ **Elm architecture preserved (Component trait, AppMsg)**
10. ✅ **Diff rendering with tool detection preserved**
11. ✅ All 6 TUI screens functional
12. ✅ Config I/O works (load/save)
13. ✅ Profile management works
14. ✅ Generation pipeline works end-to-end

---

## Risk Areas

### High Risk
- **GPG signing**: Complex external process interaction; test thoroughly
- **Generation cancellation**: Async task abortion; ensure no resource leaks
- **Config migration**: Existing user configs must still load

### Medium Risk  
- **Diff tool chain**: Delta/diff-so-fancy detection; verify on systems without tools
- **State synchronization**: Screen state <-> App data sync; check for stale data

### Low Risk
- **Theme changes**: Pure visual updates; easy to verify
- **Module moves**: Compiler will catch most issues

---

## Conclusion

This corrected plan acknowledges the actual complexity of the codebase:

- **Preserves well-designed systems**: Elm architecture, EditHistory, diff rendering
- **Correctly identifies I/O boundaries**: Config I/O in binary, structs in library  
- **Explicitly handles GPG**: Major feature not mentioned in first draft
- **Maintains test coverage**: No "remove tests" suggestion
- **Uses consistent naming**: `christina-core` throughout

The consolidation is still valuable - 4 crates → 2 crates - but done with accurate understanding of what must be preserved.
