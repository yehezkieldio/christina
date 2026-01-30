# Christina Workspace Architectural Rewrite Plan

## Executive Summary

This document proposes a complete architectural consolidation of the Christina workspace from 4 fragmented crates into **exactly 2 crates** with clean, opinionated boundaries. The current structure exhibits classic signs of over-engineering: mechanical module splits, leaky abstractions, duplicated concepts, and AI-generated ceremony.

IMPORTANT: This plan is designed for AI coding agents to do as a long-horizon task in matters of minutes to hours, with little to no human oversight, aside from keeping the machine alive. AI coding agents are given absolute freedom to choose how to implement the plan, as long as the end result matches the specifications herein.

**Total Lines of Code**: ~5,500  
**Current Crates**: 4 (christina, christina-core, christina-git, christina-llm)  
**Proposed Crates**: 2 (christina, christina-core)  
**Target**: Hermetic, normalized, aesthetically coherent architecture

---

## 1. The Two Crates

### 1.1 `christina` - The Application Crate

**Responsibility**: All user-facing code. CLI parsing, TUI rendering, event handling, and the main entry point.

**Strict Ownership**:
- CLI argument parsing and command dispatch
- TUI screens, components, and event loop
- Terminal initialization and cleanup
- Configuration file I/O (loading/saving)
- User interaction flows

**Dependencies**:
- `christina-core` (single library dependency)
- External: `clap`, `ratatui`, `crossterm`, `tokio`, `anyhow`, `directories`, `toml`, `config`

**Key Rule**: This crate contains **no business logic**. It only orchestrates calls to `christina-core` and renders results.

### 1.2 `christina-core` - The Domain Library

**Responsibility**: All domain logic, git operations, LLM orchestration, and core types. Pure logic with no I/O except through explicit dependencies.

**Strict Ownership**:
- Git repository operations (discovery, diff generation, staging, committing)
- LLM provider abstraction and map-reduce orchestration
- Prompt building and tokenization
- Domain types (CommitMessage, ProviderKind, etc.)
- Configuration data structures (NOT I/O)
- Error types

**Dependencies**:
- External: `git2`, `llm`, `tiktoken-rs`, `tokio`, `serde`, `thiserror`, `anyhow`, `url`, `regex`, `compact_str`, `parking_lot`

**Key Rule**: This crate knows **nothing about TUI, CLI, or terminal**. It operates on data structures and returns results.

---

## 2. Architectural Rationale

### 2.1 Separation of Concerns

```
┌─────────────────────────────────────────────────────────────┐
│                        christina                             │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
│  │     CLI     │  │  TUI/App    │  │   Config I/O        │  │
│  │   (clap)    │  │  (ratatui)  │  │  (load/save files)  │  │
│  └─────────────┘  └─────────────┘  └─────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
                            │
                            │ uses
                            ▼
┌─────────────────────────────────────────────────────────────┐
│                      christina-lib                           │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
│  │    Git      │  │     LLM     │  │   Domain Types      │  │
│  │ Operations  │  │Orchestration│  │  (business logic)   │  │
│  └─────────────┘  └─────────────┘  └─────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

### 2.2 Why Two Crates?

1. **Clear Boundaries**: UI vs Logic separation prevents the current mixing
2. **Compilation Speed**: Library changes don't require rebuilding the TUI
3. **Reusability**: Library could theoretically be used by other tools
4. **Mental Model**: Developers know immediately where code belongs

### 2.3 What Was Wrong with Four Crates?

- **christina-core**: Dumping ground for "shared" code. Contained types, errors, state machine, profiles - incoherent bundle.
- **christina-git**: Artificial separation. Git operations need types from core, creating circular dependency risks.
- **christina-llm**: Over-abstracted. Provider logic is ~300 lines, doesn't need its own crate.
- **Boundary Erosion**: Binary crate reached into all three library crates, creating spaghetti dependencies.

---

## 3. Proposed Directory Structure

Below is the proposed new directory structure with key files and modules. When working on the rewrite, files will be moved/merged according to this plan, make sure no files or functionality are lost. This is CRITICAL.

### 3.1 `christina/` (Application)

```
christina/
├── Cargo.toml
└── src/
    ├── main.rs           # Entry point, allocator setup, panic handling
    ├── cli.rs            # Clap argument definitions only
    ├── config/
    │   ├── mod.rs        # Config loading/saving coordination
    │   ├── file.rs       # TOML file I/O operations
    │   └── env.rs        # Environment variable parsing
    ├── tui/
    │   ├── mod.rs        # TUI module exports
    │   ├── app.rs        # App struct, state coordination (thin wrapper)
    │   ├── event_loop.rs # Event loop implementation
    │   ├── screens/
    │   │   ├── mod.rs    # Screen trait, navigation
    │   │   ├── dashboard.rs
    │   │   ├── staging.rs
    │   │   ├── generating.rs
    │   │   ├── review.rs
    │   │   └── editing.rs
    │   ├── components/   # Reusable TUI widgets
    │   │   ├── mod.rs
    │   │   ├── file_list.rs
    │   │   ├── commit_form.rs
    │   │   └── toast.rs
    │   └── theme.rs      # Colors, styles
    └── cli_commands/     # Non-TUI command handlers
        ├── mod.rs
        ├── config.rs     # `christina config get/set`
        └── profile.rs    # `christina profile create/edit`
```

**Key Changes**:
- Flattened TUI structure: removed `elm/`, `form/`, `profiles/`, `config/` submodules
- Merged `event_loop/` into single file (was 4 files for simple loop)
- Removed `bootstrap/` - terminal setup is 20 lines, doesn't need module
- Removed `generate.rs` - moved to library as `GenerationService`

### 3.2 `christina-core/` (Domain Library)

```
christina-core/
├── Cargo.toml
└── src/
    ├── lib.rs            # Public API exports
    ├── error.rs          # Unified error types (GitError, LlmError, etc.)
    ├── types/
    │   ├── mod.rs        # Type re-exports
    │   ├── commit.rs     # CommitMessage newtype
    │   ├── provider.rs   # ProviderKind enum
    │   ├── model.rs      # ModelName newtype
    │   ├── tokens.rs     # TokenCount newtype
    │   └── file.rs       # FilePath newtype
    ├── config/
    │   ├── mod.rs        # Config struct definition (NO I/O)
    │   ├── profile.rs    # ProviderProfile, Profiles
    │   └── validation.rs # Config validation logic
    ├── git/
    │   ├── mod.rs        # Git operations exports
    │   ├── repo.rs       # Repository struct, all git operations
    │   ├── diff.rs       # Diff generation, StagedDiff
    │   ├── status.rs     # File status types (GitFileStatus)
    │   └── chunk.rs      # DiffChunk, diff splitting
    ├── llm/
    │   ├── mod.rs        # LLM exports
    │   ├── provider.rs   # Provider trait, OpenAI/Azure impls
    │   ├── orchestrator.rs # Map-reduce generation logic
    │   ├── prompt.rs     # Prompt templates, PromptBuilder
    │   ├── tokenizer.rs  # Token counting
    │   └── retry.rs      # Retry policies
    └── generation/
        ├── mod.rs        # Generation coordination
        └── service.rs    # Main GenerationService facade
```

**Key Changes**:
- Consolidated all domain logic into single crate
- Flattened module hierarchy (no more `types/commit_message.rs`, just `types/commit.rs`)
- Removed `christina-git/` - merged into `git/` module
- Removed `christina-llm/` - merged into `llm/` module
- Unified error types in single file instead of scattered re-exports

---

## 4. Module Consolidation Guide

### 4.1 Modules to DELETE

| Current Location | Reason |
|-----------------|--------|
| `christina/src/app/context.rs` | Merged into `App` struct directly |
| `christina/src/app/edit_history.rs` | Not used in current UI |
| `christina/src/app/handlers.rs` | Inline into event_loop.rs |
| `christina/src/app/init.rs` | Inline into `App::new()` |
| `christina/src/app/state.rs` | Merged into `App` struct |
| `christina/src/bootstrap/` | Inline terminal setup |
| `christina/src/config/cli.rs` | Move to `cli_commands/config.rs` |
| `christina/src/config/profile_cli.rs` | Move to `cli_commands/profile.rs` |
| `christina/src/config/diff_tool.rs` | Merge into config/mod.rs |
| `christina/src/config/settings.rs` | Rename to config/mod.rs in lib |
| `christina/src/event_loop/events.rs` | Inline event types into mod.rs |
| `christina/src/event_loop/handlers.rs` | Inline into event_loop.rs |
| `christina/src/event_loop/producers.rs` | Inline into event_loop.rs |
| `christina/src/generate.rs` | Move to `christina-lib/src/generation/` |
| `christina/src/tui/components_elm.rs` | Merge into components/mod.rs |
| `christina/src/tui/config/` | Merge into single `screens/config.rs` |
| `christina/src/tui/context.rs` | Merge into app.rs |
| `christina/src/tui/diff_executor.rs` | Inline where used |
| `christina/src/tui/diff_renderer.rs` | Inline where used |
| `christina/src/tui/elm.rs` | Not needed with simpler architecture |
| `christina/src/tui/form/` | Simplify to single `components/commit_form.rs` |
| `christina/src/tui/layout.rs` | Inline helper functions |
| `christina/src/tui/profiles/` | Merge into single `screens/profiles.rs` |
| `christina/src/tui/screens/error.rs` | Inline as modal in app.rs |
| `christina-core/src/git/mod.rs` | Move to `christina-lib/src/git/` |
| `christina-core/src/git/file.rs` | Merge into `git/status.rs` |
| `christina-core/src/git/diff.rs` | Move to `christina-lib/src/git/chunk.rs` |
| `christina-core/src/state.rs` | Inline `StateMachine` into app.rs |
| `christina-core/src/tokenizer.rs` | Move to `christina-lib/src/llm/tokenizer.rs` |
| `christina-git/src/buffer_pool.rs` | Delete - premature optimization |
| `christina-git/src/chunking.rs` | Merge into `git/chunk.rs` |
| `christina-git/src/diff_processor.rs` | Merge into `generation/service.rs` |
| `christina-git/src/parsing.rs` | Merge into `git/diff.rs` |
| `christina-git/src/repository.rs` | Rename to `git/repo.rs` |
| `christina-llm/src/concurrency.rs` | Merge into `llm/orchestrator.rs` |
| `christina-llm/src/provider.rs` | Rename to `llm/provider.rs` |
| `christina-llm/src/providers/` | Flatten into `llm/provider.rs` |
| `christina-llm/src/retry.rs` | Rename to `llm/retry.rs` |
| `christina-llm/src/tokenizer.rs` | Rename to `llm/tokenizer.rs` |

### 4.2 Modules to RENAME

| From | To | Reason |
|------|-----|--------|
| `christina/src/config/settings.rs` | `christina-lib/src/config/mod.rs` | Config is a domain type, not app concern |
| `christina-core/src/error.rs` | `christina-lib/src/error.rs` | Unified errors in lib |
| `christina-core/src/profile.rs` | `christina-lib/src/config/profile.rs` | Profiles are config |
| `christina-core/src/prompt.rs` | `christina-lib/src/llm/prompt.rs` | Prompts are LLM concern |
| `christina-core/src/types/` | `christina-lib/src/types/` | Keep but flatten files |
| `christina-git/src/repository.rs` | `christina-lib/src/git/repo.rs` | Consistent naming |
| `christina-llm/src/orchestrator.rs` | `christina-lib/src/llm/orchestrator.rs` | Keep name, move location |

### 4.3 Key Data Structures

**App State (christina/src/tui/app.rs)**:
```rust
pub struct App {
    pub state: Screen,           // Current screen enum
    pub repo: Option<RepositoryInfo>, // Git repo path + branch
    pub staged_files: Vec<GitFile>,
    pub unstaged_files: Vec<GitFile>,
    pub generated_message: Option<CommitMessage>,
    pub error_message: Option<String>,
    pub should_quit: bool,
    // Generation state
    pub generation: GenerationState,
    // UI state  
    pub spinner_frame: usize,
    pub toast_manager: ToastManager,
}

pub enum GenerationState {
    Idle,
    Running { task: AbortOnDrop, id: u64 },
}

pub enum Screen {
    Staging,
    Dashboard,
    Generating(GeneratingScreen),
    Review(ReviewScreen),
    Editing(EditingScreen),
}
```

**Config (christina-core/src/config/mod.rs)**:
```rust
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
    pub diff_tool: DiffTool,
    pub use_commit_history: bool,
    pub commit_history_depth: usize,
    // Azure-specific
    pub azure_api_version: Option<String>,
    pub azure_deployment_id: Option<String>,
}
```

**GenerationService (christina-lib/src/generation/service.rs)**:
```rust
pub struct GenerationService {
    provider: Arc<dyn Provider>,
    config: Config,
}

impl GenerationService {
    pub fn new(config: Config) -> Result<Self>;
    
    pub async fn generate(
        &self,
        diff: String,
        user_context: Option<String>,
        progress: mpsc::Sender<GenerationEvent>,
    ) -> Result<GenerationResult>;
}
```

---

## 5. Anti-Patterns Being Eliminated

### 5.1 Over-Fragmentation

**Before**: 4 crates, 40+ modules, 5,500 lines  
**After**: 2 crates, 15 modules, ~4,500 lines (deletion of ceremony)

**Examples**:
- `event_loop/` was 3 files for a 100-line loop
- `tui/form/` was 4 files for a simple editable form
- `providers/` was 3 files for 200 lines of HTTP calls

### 5.2 Duplicate State

**Before**: 
- `AppState` enum in core + screen-specific states in binary
- `AppContextData` + `TuiSessionData` + `DataState` + `UiState`

**After**:
- Single `App` struct with flat field layout
- Screen-specific data in enum variants (Generating, Review, etc.)

### 5.3 Parallel Abstractions

**Before**:
- `ProviderProfile` (core) vs `Config` (binary) - same fields, different types
- `GitFile` (core) vs `FilePatch` (git) - similar purpose, different structures
- `TokenCount` newtype but other primitives bare

**After**:
- `Config` is the single source of truth
- `GitFile` is the canonical file representation
- Newtypes for all domain types (CommitMessage, ModelName, FilePath)

### 5.4 "Context" Dumping Grounds

**Before**:
- `AppContextData` held repo, config, branch
- `DataState` held file lists, toasts
- `UiState` held spinner, redraw flag

**After**:
- All merged into single `App` struct with clear field names
- No "context" types - explicit fields only

### 5.5 Leaky Layers

**Before**:
- Binary crate imported `git2` directly for error handling
- Event loop spawned git operations directly
- TUI screens knew about `DiffChunk` structure

**After**:
- Binary only uses `christina_lib::git::Repository`
- All git operations encapsulated in library
- TUI only deals with `String` diffs and `CommitMessage` results

---

## 6. Phased Migration Plan

1. Backup current workspace to backup folder by moving all crates there.
2. Remove or omit tests codes to focus on full rewrite (tests will be re-added later).

### Phase 1: Create christina-core

1. Create `christina-core/` directory
2. Copy consolidated files from analysis:
   - `src/error.rs` - unified errors
   - `src/types/` - all domain types
   - `src/config/` - Config, Profile, validation
   - `src/git/` - Repository, diff operations
   - `src/llm/` - Provider, orchestrator, prompts
   - `src/generation/` - GenerationService
3. Update `Cargo.toml` with all dependencies
4. Make it compile standalone

### Phase 2: Migrate christina Binary 

1. Rewrite `christina/src/main.rs`:
   - Simplified to just CLI parse + dispatch
   - Remove direct git2 imports
   
2. Rewrite `christina/src/tui/app.rs`:
   - Flatten state structure
   - Use `christina_lib::` types exclusively
   
3. Rewrite `christina/src/tui/event_loop.rs`:
   - Single file, ~150 lines
   - Use `GenerationService` instead of inline generation
   
4. Rewrite screens:
   - Simplify to 5 screen modules
   - Remove elm architecture
   
5. Update `Cargo.toml`:
   - Remove `christina-core`, `christina-git`, `christina-llm` deps
   - Add `christina-lib` path dependency

### Phase 3: Delete Old Crates (Week 2)

1. Delete `christina-core/`
2. Delete `christina-git/`
3. Delete `christina-llm/`
4. Update workspace `Cargo.toml`:
   ```toml
   [workspace]
   members = ["christina", "christina-lib"]
   ```

### Phase 4: Quality Gates (Week 2)

Run and fix until clean:
```bash
just check   # cargo check with zero warnings
just clippy  # cargo clippy with zero warnings  
```

### Phase 5: Verification (Week 3)

1. Manual testing:
   - Config commands work
   - Profile management works
   - TUI launches and navigates
   - Generation pipeline works
   - Commits can be created
   
2. Regression testing:
   - Large diffs handled correctly
   - Binary files detected
   - Error states displayed properly
   - GPG signing still works

---

## 7. Specific Code Transformations

### 7.1 Error Handling Consolidation

**Before** (`christina-core/src/error.rs`):
```rust
#[derive(Debug, Error)]
pub enum GitError { ... }

#[derive(Debug, Error)]
pub enum CompletionError { ... }

#[derive(Debug, Error)]
pub enum ProviderError { ... }

#[derive(Debug, Error)]
pub enum AppError { ... }  // Wrapper enum
```

**After** (`christina-lib/src/error.rs`):
```rust
#[derive(Debug, Error)]
pub enum Error {
    #[error("git error: {0}")]
    Git(#[from] git2::Error),
    
    #[error("LLM error: {0}")]
    Llm(String),
    
    #[error("configuration error: {0}")]
    Config(String),
    
    #[error("generation failed: {0}")]
    Generation(String),
    
    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, Error>;
```

### 7.2 Config Simplification

**Before** (`christina/src/config/settings.rs` - 1,125 lines):
- Custom get/set by string key
- Profile synchronization logic
- Editable trait implementation
- Extensive test boilerplate

**After** (`christina-lib/src/config/mod.rs` - ~300 lines):
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config { ... }

impl Config {
    pub fn load() -> Result<Self> { ... }
    pub fn save(&self) -> Result<()> { ... }
    pub fn active_profile(&self) -> Option<&ProviderProfile> { ... }
    pub fn apply_profile(&mut self, name: &str) -> Result<()> { ... }
}

// Validation in separate module
pub fn validate(config: &Config) -> Result<()> { ... }
```

### 7.3 Generation Pipeline Simplification

**Before** (spread across `generate.rs`, `orchestrator.rs`, `diff_processor.rs`):
- Event loop spawned async task
- Direct git operations in background
- Complex progress reporting

**After** (`christina-lib/src/generation/service.rs`):
```rust
pub struct GenerationService { ... }

pub struct GenerationResult {
    pub message: CommitMessage,
    pub warnings: Vec<String>,
    pub truncated: bool,
}

impl GenerationService {
    pub async fn generate(
        &self,
        repo_path: &Path,
        user_context: Option<&str>,
    ) -> Result<GenerationResult> { ... }
}
```

Called from event loop:
```rust
// In christina/src/tui/event_loop.rs
let service = GenerationService::new(config)?;
let result = service.generate(&repo_path, user_context.as_deref()).await?;
app.generated_message = Some(result.message);
```

### 7.4 Git Operations Consolidation

**Before** (scattered across `christina-core/src/git/`, `christina-git/src/`):
- Types in core, operations in git crate
- Re-exports and wrapper types
- `StagedDiff` in git crate, `DiffChunk` in core

**After** (`christina-lib/src/git/repo.rs`):
```rust
pub struct Repository {
    inner: git2::Repository,
}

impl Repository {
    pub fn discover() -> Result<Self>;
    pub fn open(path: &Path) -> Result<Self>;
    pub fn staged_diff(&self) -> Result<Diff>;
    pub fn unstaged_files(&self) -> Result<Vec<GitFile>>;
    pub fn stage_files(&self, files: &[PathBuf]) -> Result<()>;
    pub fn unstage_files(&self, files: &[PathBuf]) -> Result<()>;
    pub fn commit(&self, message: &CommitMessage) -> Result<Oid>;
    pub fn commit_history(&self, limit: usize) -> Result<Vec<CommitInfo>>;
}

pub struct Diff {
    content: String,
    files: Vec<FilePath>,
}

pub struct GitFile {
    pub path: FilePath,
    pub status: FileStatus,
    pub content: String,
}
```

---

## 8. Success Criteria

The rewrite is complete when:

1. **Exactly 2 crates** exist in the workspace
2. **Zero compiler warnings** (`just check` passes)
3. **Zero clippy warnings** (`just clippy` passes)
5. **No circular dependencies** between modules
6. **Binary crate has zero direct git2/llm imports** - all through library
7. **Library crate has zero terminal/UI imports**
8. **All functionality preserved** (manual QA checklist)

---

## 9. Risk Management

- Breaking Changes: You are allowed to do radical refactors or rewrites of functions as long as the end-user functionality remains the same.
