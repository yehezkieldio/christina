# Christina Architectural Redesign

**Status**: Authoritative Blueprint for Implementation  
**Scope**: Full greenfield redesign from 4 fragmented crates → 2 hermetic crates  
**Compatibility**: None required (pre-alpha project)  
**Execution**: Designed for autonomous AI agent implementation

---

## Executive Summary

Christina is being redesigned from **4 fragmented crates** (~18.5k LOC) exhibiting over-fragmentation, duplicated state, parallel abstractions, and leaky boundaries into **exactly 2 crates** with strict, Rust-native architectural discipline.

### Current Problems (Validated by Analysis)

1. **Config/Profile Duplication**: `Config` struct has inline provider fields + `profiles: Profiles` field (redundant state). Conversion methods `Config::apply_profile()`, `Config::to_profile()`, `Provider::from_profile()` translate between duplicated representations.

2. **State Ownership Fragmented**: `TuiSessionData` wraps `DataState` and stores 6 `Option<ScreenState>` fields. `StateMachine` validates transitions but doesn't own screen states. Per-screen state duplicated/synced bidirectionally with `DataState.base` fields (selected_indices, multi_select_mode).

3. **Provider Abstraction Split**: `ProviderKind` enum in core, `Provider` enum in llm crate with same variants. Azure URL parsing duplicated.

4. **Git Models Reimplemented**: `GitFile`/`GitFileStatus` canonical in core, `GitRepository` in christina-git reimplements parallel types with conversion logic.

5. **Thin Wrapper Crates**: christina-git (2,249 LOC) and christina-llm (3,476 LOC) are mechanical splits providing minimal abstraction value while increasing cognitive load.

### What Works (Must Preserve)

- **Elm Architecture**: Clean `Component` trait, pure `update()` functions, `AppMsg` for side effects, central `App::handle_app_msg()` dispatcher
- **StateMachine Transition Validation**: Well-tested state flow validation (520 LOC of tests)
- **Config/Profile TUI Flows**: Self-contained modular TUI runners with callbacks
- **AbortOnDrop Pattern**: Prevents orphaned background tasks

---

## The Two Crates

### `christina-core` — The Sans-IO Engine

**Responsibility**: Pure domain logic with **zero I/O dependencies**.

**Core Principle**: Sans-IO architecture. All state machines, domain types, and business logic live here. No tokio, no reqwest, no git2 in runtime dependencies (git2 allowed in dev-dependencies for tests only).

**What Goes Here**:
- **Canonical Model**: Single source of truth for application state
- **Elm Architecture Core**: `Msg` enum, `Cmd` enum, `update()` pure state transitions
- **Domain Types**: `CommitMessage`, `FilePath`, `TokenCount`, `ModelName`, `GitFile`, `DiffChunk`
- **Config/Profile System**: `ConfigFile` (serialization), `ResolvedConfig` (runtime), `ProviderProfile<S>` (generic over secrets)
- **LLM Domain Logic**: `LlmRequest`, `LlmResponse`, `ProviderSpec`, prompt builders, token budgets, retry policy decisions (pure)
- **Git Domain Logic**: `GitFile`, `GitFileStatus`, `DiffChunk`, pure diff parsing (text → Vec<DiffChunk>)
- **State Machine**: `AppState` enum, transition validation, `Route` enum for screen routing
- **Error Types**: `AppError`, `CompletionError`, `GitError`, `ProviderError` (canonical)

**Dependencies** (Cargo.toml):
```toml
[dependencies]
anyhow = { workspace = true }
thiserror = { workspace = true }
serde = { workspace = true }
compact_str = { workspace = true }
regex = { workspace = true }
url = { workspace = true }
tiktoken-rs = { version = "0.9", default-features = false }  # Pure, no tokio

[dev-dependencies]
git2 = { workspace = true }  # ONLY for tests, not runtime
tempfile = { workspace = true }
```

**CI Enforcement**:
```bash
# Fail build if I/O deps leak into core
cargo tree -p christina-core | grep -E '(tokio|async-std|reqwest|git2)' && exit 1
```

---

### `christina` — The I/O Shell

**Responsibility**: All user-facing code (CLI/TUI), I/O execution, runtime orchestration.

**Core Principle**: Thin shell around `christina-core`. Executes `Cmd` outputs from core, produces `Msg` inputs to core. No business logic.

**What Goes Here**:
- **main.rs**: Binary entry point, tokio runtime initialization
- **cli/**: Clap argument parsing, subcommand routing
- **tui/**: Terminal setup, event loop, Ratatui views, input handling
- **runtime/**: `cmd_exec.rs` (executes `Cmd` → produces `Msg`), `AbortOnDrop` wrapper
- **io/**: All I/O adapters
  - `io/config_io.rs`: Load/save config files, read env vars, keyring access
  - `io/secrets.rs`: Resolve `SecretRef` → `SecretString` at runtime
  - `io/git/*.rs`: git2 adapters returning `core::git::RepoSnapshot`
  - `io/llm/*.rs`: HTTP clients for OpenAI, Azure, Groq, etc. (execute `core::llm::LlmRequest`)

**Dependencies** (Cargo.toml):
```toml
[dependencies]
christina-core = { path = "../christina-core" }
tokio = { workspace = true }
git2 = { workspace = true }
llm = { version = "1.3.7", features = ["rustls-tls", "azure_openai", "groq", "openai"] }
ratatui = { version = "0.29" }
clap = { version = "4.5" }
config = { version = "0.15" }
directories = "6.0"
# ... (all I/O-related deps)
```

---

## Architectural Decisions

### 1. Config/Profile Consolidation

**Problem**: Config has inline provider fields + `profiles: Profiles` field (two sources of truth).

**Solution**: Single source of truth in `christina-core`, two-phase model.

#### Phase 1: Serialization (`ConfigFile`)

Located in `christina-core/src/config/config_file.rs`.

```rust
/// On-disk representation (serde-friendly)
#[derive(Serialize, Deserialize)]
pub struct ConfigFile {
    pub active_profile: Option<String>,
    pub profiles: HashMap<String, ProviderProfile<SecretRef>>,
    pub commit_message_max_length: Option<usize>,
    pub ignore_files: Vec<String>,
    // ... (safe fields only)
}
```

#### Phase 2: Runtime (`ResolvedConfig`)

Located in `christina-core/src/config/resolved.rs`.

```rust
/// Runtime configuration with resolved secrets
pub struct ResolvedConfig {
    pub active_profile: Option<String>,
    pub profiles: HashMap<String, ProviderProfile<SecretString>>,
    pub commit_message_max_length: usize,  // Resolved from default
    pub ignore_files: Vec<String>,
}
```

#### Secret Handling (Generic Over Secret Type)

Located in `christina-core/src/config/secret.rs`.

```rust
/// Generic secret container
pub enum Secret<S> {
    Value(S),
}

/// On-disk reference (env var or keyring)
pub enum SecretRef {
    EnvVar(String),      // "OPENAI_API_KEY"
    Keyring(String),     // "christina.openai"
}

/// Runtime secret (redacted in Debug)
pub struct SecretString(String);

impl Debug for SecretString {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.write_str("[REDACTED:secret]")
    }
}
```

#### Provider Profiles (Enum Per Provider)

Located in `christina-core/src/config/profile.rs`.

```rust
/// Provider-specific configuration (generic over secrets)
pub enum ProviderProfile<S> {
    OpenAi {
        model: ModelName,
        base_url: Option<Url>,
        api_key: Secret<S>,
        max_input_tokens: TokenCount,
        max_output_tokens: TokenCount,
    },
    AzureOpenAi {
        endpoint: AzureEndpoint,  // newtype with TryFrom<Url>
        api_version: String,
        deployment: String,
        api_key: Secret<S>,
        max_input_tokens: TokenCount,
        max_output_tokens: TokenCount,
    },
}
```

**Azure Endpoint Parsing** (core, pure):
```rust
/// Newtype enforcing validated Azure endpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AzureEndpoint(Url);

impl TryFrom<Url> for AzureEndpoint {
    type Error = AzureEndpointError;
    fn try_from(url: Url) -> Result<Self, Self::Error> {
        // Normalize, validate format
        // Extract endpoint/api_version/deployment if in URL
    }
}
```

#### Config Loading (I/O in bin)

Located in `christina/src/io/config_io.rs`.

```rust
/// Load config from disk + env vars → ResolvedConfig
pub fn load_config() -> Result<ResolvedConfig> {
    let file: ConfigFile = load_toml("~/.config/christina/config.toml")?;
    let env_overrides = load_env_vars()?;
    let resolved = resolve_secrets(&file)?;  // SecretRef → SecretString
    let merged = apply_env_overrides(resolved, env_overrides)?;
    core::config::validate_config(merged)  // Pure validation in core
}

fn resolve_secrets(config: &ConfigFile) -> Result<ResolvedConfig> {
    // Read env vars, query keyring, resolve SecretRef → SecretString
}
```

**DELETE**:
- `christina/src/config/settings.rs::Config` duplicate provider fields (max_input_tokens, model_provider, api_key, azure_*, etc.)
- `Config::apply_profile()`, `Config::to_profile()` conversion methods
- `christina-llm/src/provider.rs::Provider::from_profile()` (replaced by pure construction in core)

---

### 2. State Ownership Model

**Problem**: State ownership fragmented across `App`, `TuiSessionData`, `DataState.base`, `StateMachine`, 6 `Option<ScreenState>` fields.

**Solution**: Single canonical `Model` in `christina-core`, `App` in bin owns runtime state only.

#### Core: Canonical Model

Located in `christina-core/src/app/model.rs`.

```rust
/// Single source of truth for application state
pub struct Model {
    pub route: Route,
    pub screens: Screens,
    pub git: GitState,
    pub generation: GenerationStatus,
    pub toasts: Vec<Toast>,
    pub user_context: Option<String>,
}

/// Current screen/route (persistent navigation state)
pub enum Route {
    StagingSelection,
    Dashboard,
    Generating,
    Review,
    Editing,
    Error,
}

/// Per-screen state (screens persist across navigation)
pub struct Screens {
    pub staging: StagingState,
    pub dashboard: DashboardState,
    pub review: ReviewState,
    pub editing: EditingState,
    pub generating: GeneratingState,
    pub error: ErrorState,
}

/// Git-related state (canonical)
pub struct GitState {
    pub files: Vec<GitFile>,
    pub staged: Vec<FilePath>,
    pub branch: String,
    pub repo_root: PathBuf,
}

/// Pure generation tracking (no I/O)
pub enum GenerationStatus {
    Idle,
    Running { id: GenerationId },
    Completed { id: GenerationId, message: CommitMessage },
    Failed { id: GenerationId, error: String },
}
```

**Design Decision**: Screens **persist** across navigation (enum `Route` + struct `Screens`). This eliminates `Option<ScreenState>` and allows screens to retain ephemeral UI state (scroll position, cursor) when navigating away/back.

**Alternative Considered** (if memory is tight):
```rust
/// Screens drop when inactive (active screen only)
pub enum ActiveScreen {
    Dashboard(DashboardState),
    Review(ReviewState),
    Editing(EditingState),
    // ...
}
```
**Rejected**: Loses ephemeral UI state on navigation. Use persistent `Screens` struct.

#### Bin: Runtime State

Located in `christina/src/runtime/state.rs`.

```rust
/// Runtime-only state (I/O, async tasks)
pub struct RuntimeState {
    pub tasks: HashMap<GenerationId, AbortOnDrop<JoinHandle<()>>>,
    pub terminal: TerminalHandle,
    pub tick_rate: Duration,
}

/// App container (thin wrapper)
pub struct App {
    pub model: core::Model,           // Canonical domain state
    pub runtime: RuntimeState,        // I/O state
    pub app_context: AppContextData,  // Resources (repo handle, config)
}
```

#### AppContextData (Resource Holder)

Located in `christina/src/app/context.rs`.

```rust
/// Resources shared across handlers (NOT domain state)
pub struct AppContextData {
    pub repo: Repository,         // git2::Repository handle
    pub config: ResolvedConfig,   // Loaded config
    pub branch: String,           // Cached branch name
}
```

**NOT a dumping ground**. Only holds resources needed for I/O operations.

#### Eliminating Duplication

**DELETE**:
- `christina/src/tui/context.rs`: `UiState`, `DataState` (replaced by `core::Model`)
- `christina/src/app/state.rs`: `TuiUiState`, `TuiSessionData` (wrappers eliminated)
- `DataState.base` fields: selected_indices, multi_select_mode (move to screen state)
- Bidirectional sync code in `components_elm.rs` (selection state lives in ONE place: `StagingState` or `DashboardState`)

**MOVE**:
- `GenerationState::Running` → `core::GenerationStatus::Running{id}`
- Async `JoinHandle` tracking → `RuntimeState.tasks: HashMap<GenerationId, AbortOnDrop>`

---

### 3. Provider Architecture

**Problem**: `ProviderKind` enum in core, `Provider` enum in llm crate (parallel abstractions).

**Solution**: Core owns configuration and request/response types. Bin owns HTTP clients.

#### Core: Provider Configuration & Requests

Located in `christina-core/src/llm/`.

```rust
// Provider configuration (resolved from ProviderProfile)
pub struct ProviderSpec {
    pub kind: ProviderKind,
    pub model: ModelName,
    pub endpoint: ProviderEndpoint,
    pub max_tokens: TokenCount,
    pub temperature: f32,
}

pub enum ProviderEndpoint {
    OpenAi { base_url: Url },
    AzureOpenAi { endpoint: AzureEndpoint, api_version: String, deployment: String },
}

// Request/Response (pure, no I/O)
pub struct LlmRequest {
    pub provider: ProviderSpec,
    pub messages: Vec<ChatMessage>,
    pub max_tokens: TokenCount,
    pub temperature: f32,
}

pub struct LlmResponse {
    pub content: String,
    pub tokens_used: TokenCount,
}
```

#### Core: Cmd/Msg for I/O

Located in `christina-core/src/app/cmd.rs`.

```rust
/// Commands emitted by pure update() (side effects requested)
pub enum Cmd {
    StartLlmRequest { request: LlmRequest, id: GenerationId },
    RefreshGitStatus,
    StageFiles { paths: Vec<FilePath> },
    CommitMessage { message: CommitMessage },
    // ...
}

/// Messages fed back into update() (I/O results)
pub enum Msg {
    LlmResponseReceived { id: GenerationId, response: Result<LlmResponse, CompletionError> },
    GitStatusRefreshed { snapshot: RepoSnapshot },
    FilesStaged { paths: Vec<FilePath> },
    // ...
}
```

#### Bin: LLM Inference (Adapters)

Located in `christina/src/io/llm/*.rs`.

```rust
// christina/src/io/llm/openai.rs
pub async fn execute_openai_request(request: &core::llm::LlmRequest) -> Result<core::llm::LlmResponse> {
    // code
}
```

IMPORTANT: We strictly use `llm` crate for LLM inference, no custom HTTP clients or reqwest code. See original implementation in christina-llm for reference.

#### Bin: Command Executor

Located in `christina/src/runtime/cmd_exec.rs`.

```rust
pub async fn execute_cmd(cmd: core::Cmd, ctx: &AppContextData) -> Vec<core::Msg> {
    match cmd {
        Cmd::StartLlmRequest { request, id } => {
            let result = match request.provider.kind {
                ProviderKind::OpenAI => io::llm::openai::execute_openai_request(&request).await,
                ProviderKind::Azure => io::llm::azure_openai::execute_azure_request(&request).await,
            };
            vec![Msg::LlmResponseReceived { id, response: result }]
        }
        Cmd::RefreshGitStatus => {
            let snapshot = io::git::status(&ctx.repo)?;
            vec![Msg::GitStatusRefreshed { snapshot }]
        }
        // ...
    }
}
```

**DELETE**:
- `christina-llm/src/provider.rs::Provider` enum (replaced by `ProviderSpec` in core + adapters in bin)
- `Provider::from_profile()` (replaced by pure `ProviderSpec` construction in core)
- Azure URL parsing in llm crate (moved to `core::config::AzureEndpoint`)

---

### 4. Git Operations

**Problem**: Git models duplicated (`GitFile` in core, `FilePatch` in christina-git with conversion logic).

**Solution**: Core owns canonical git domain types. Bin has git2 adapter returning core types.

#### Core: Git Domain Models

Located in `christina-core/src/git/`.

```rust
/// Canonical git file representation
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitFile {
    pub path: FilePath,
    pub status: GitFileStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitFileStatus {
    Untracked,
    Modified,
    Added,
    Deleted,
    Renamed,
}

/// Canonical diff representation
#[derive(Debug, Clone)]
pub struct DiffChunk {
    pub header: String,
    pub old_start: u32,
    pub old_lines: u32,
    pub new_start: u32,
    pub new_lines: u32,
    pub lines: Vec<DiffLine>,
}

/// Repository snapshot (returned by git adapter)
pub struct RepoSnapshot {
    pub files: Vec<GitFile>,
    pub branch: String,
    pub root: PathBuf,
}
```

#### Core: Pure Diff Parsing (Optional)

Located in `christina-core/src/git/diff_parse.rs`.

If diff parsing is **pure** (unified diff text → `Vec<DiffChunk>` with no git2 dependency), place in core:

```rust
/// Parse unified diff format into structured chunks
pub fn parse_unified_diff(diff_text: &str) -> Result<Vec<DiffChunk>, DiffParseError> {
    // Pure text parsing, no git2 types
}
```

If parsing **requires git2 types** (e.g., libgit2's diff structs), keep in bin adapter.

#### Bin: git2 Adapter

Located in `christina/src/io/git/adapter.rs`.

```rust
/// Return core types from git2 operations
pub fn status(repo: &Repository) -> Result<core::git::RepoSnapshot> {
    let statuses = repo.statuses(None)?;
    let files = statuses.iter().map(|entry| {
        core::git::GitFile {
            path: core::FilePath::from(entry.path().unwrap()),
            status: convert_status(entry.status()),  // git2::Status → core::GitFileStatus
        }
    }).collect();
    
    Ok(core::git::RepoSnapshot {
        files,
        branch: get_branch_name(repo)?,
        root: repo.workdir().unwrap().to_path_buf(),
    })
}

fn convert_status(status: git2::Status) -> core::git::GitFileStatus {
    // Conversion logic (no duplication, single direction)
}
```

**DELETE**:
- `christina-git/` entire crate (logic folded into core + bin adapter)
- `christina-git/src/repository.rs::FilePatch`, `FileStatus` (parallel types eliminated)
- Bidirectional conversion methods (only bin → core conversion needed)

---

### 5. Elm Architecture Preservation

**Current Implementation (Keep)**:
- `Component` trait in `christina/src/tui/elm.rs`
- Per-screen `Msg` enums, `update()` methods
- `AppMsg` for side effects
- Central `App::handle_app_msg()` dispatcher

**Changes for 2-Crate Model**:

#### Core: Global Update Function

Located in `christina-core/src/app/update.rs`.

```rust
/// Pure state transition (Elm Architecture)
pub fn update(model: &mut Model, msg: Msg) -> Vec<Cmd> {
    match model.route {
        Route::Dashboard => screens::dashboard::update(&mut model.screens.dashboard, msg),
        Route::Review => screens::review::update(&mut model.screens.review, msg),
        // ...
    }
}
```

Each screen module (`christina-core/src/app/screens/*.rs`) exports:
```rust
pub fn update(state: &mut DashboardState, msg: Msg) -> Vec<Cmd> {
    // Pure state transitions, emit Cmd for side effects
}

pub fn view(state: &DashboardState) -> DashboardViewModel {
    // Pure view model (data for rendering, no ratatui types)
}
```

#### Bin: View Rendering

Located in `christina/src/tui/view/*.rs`.

```rust
// Render DashboardViewModel using ratatui
pub fn render_dashboard(vm: &core::app::DashboardViewModel, frame: &mut Frame) {
    // Ratatui rendering code
}
```

**Separation**: Core produces view models (data), bin renders with ratatui (I/O).

---

### 6. Module Trees

#### `christina-core/src/`

```
christina-core/src/
├── lib.rs
├── app/
│   ├── mod.rs
│   ├── model.rs         (Model, Route, Screens, GitState, GenerationStatus)
│   ├── msg.rs           (Msg enum - inputs to update)
│   ├── cmd.rs           (Cmd enum - outputs from update)
│   ├── update.rs        (update() entry point, routes to screens)
│   ├── state_machine.rs (StateMachine transition validation)
│   └── screens/
│       ├── dashboard.rs (DashboardState, update, view model)
│       ├── staging.rs
│       ├── review.rs
│       ├── editing.rs
│       ├── generating.rs
│       └── error.rs
├── config/
│   ├── mod.rs
│   ├── config_file.rs   (ConfigFile serde struct)
│   ├── resolved.rs      (ResolvedConfig runtime struct)
│   ├── profile.rs       (ProviderProfile<S> enum)
│   ├── secret.rs        (Secret<S>, SecretRef, SecretString)
│   └── validation.rs    (Pure config validation)
├── llm/
│   ├── mod.rs
│   ├── request.rs       (LlmRequest, LlmResponse)
│   ├── provider_spec.rs (ProviderSpec, ProviderEndpoint)
│   ├── prompt.rs        (Prompt builders)
│   ├── tokens.rs        (Token budgets, tiktoken-rs)
│   └── retry.rs         (RetryPolicy decisions - pure)
├── git/
│   ├── mod.rs
│   ├── models.rs        (GitFile, GitFileStatus, RepoSnapshot)
│   ├── diff_chunk.rs    (DiffChunk, DiffLine)
│   └── diff_parse.rs    (Pure diff parsing if no git2 dep)
├── types/
│   ├── mod.rs
│   ├── commit_message.rs
│   ├── file_path.rs
│   ├── model_name.rs
│   ├── provider_kind.rs
│   └── token_count.rs
├── error.rs             (AppError, CompletionError, GitError, ProviderError)
└── ids.rs               (GenerationId newtype)
```

**Lines of Code Estimate**: ~7,000 LOC (consolidated from christina-core 2,329 + parts of llm/git crates)

---

#### `christina/src/`

```
christina/src/
├── main.rs              (Tokio runtime, CLI routing)
├── cli/
│   ├── mod.rs
│   ├── args.rs          (Clap definitions)
│   └── subcommands.rs   (init, generate, config)
├── runtime/
│   ├── mod.rs
│   ├── state.rs         (RuntimeState, App)
│   ├── cmd_exec.rs      (Execute Cmd → produce Msg)
│   └── abort.rs         (AbortOnDrop wrapper)
├── io/
│   ├── config_io.rs     (Load/save config files, env vars)
│   ├── secrets.rs       (Resolve SecretRef → SecretString)
│   ├── git/
│   │   ├── mod.rs
│   │   ├── adapter.rs   (git2 → core::git::RepoSnapshot)
│   │   ├── stage.rs     (Staging operations)
│   │   └── commit.rs    (Commit operations)
│   └── llm/
│       ├── mod.rs
│       ├── openai.rs    (HTTP client for OpenAI)
│       ├── azure_openai.rs
│       ├── groq.rs
│       └── client_common.rs
├── tui/
│   ├── mod.rs
│   ├── terminal.rs      (Terminal setup/cleanup)
│   ├── event_loop.rs    (Main TUI event loop)
│   ├── input.rs         (Crossterm input handling)
│   ├── view/
│   │   ├── dashboard.rs (Render core::DashboardViewModel)
│   │   ├── staging.rs
│   │   ├── review.rs
│   │   ├── editing.rs
│   │   └── generating.rs
│   ├── config/          (Config TUI module)
│   │   ├── mod.rs
│   │   ├── runner.rs
│   │   ├── screen.rs
│   │   └── update.rs
│   └── profiles/        (Profile TUI module)
│       ├── mod.rs
│       ├── runner.rs
│       └── screen.rs
└── app/
    ├── mod.rs
    ├── context.rs       (AppContextData resource holder)
    └── handlers.rs      (handle_app_msg dispatcher - legacy compat if needed)
```

**Lines of Code Estimate**: ~11,000 LOC (binary + I/O adapters + TUI rendering)

---

## Migration Plan

Note: Rewrite in this case, is could be move + adapt or code restructuring.

### Phase 0: Preparation

1. **Backup existing crates** into `backup/` folder for reference:
   ```bash
   mkdir -p backup
   cp -r christina christina-core christina-git christina-llm backup/
   ```

2. **Create new crate structure**:
   ```bash
   # Keep workspace Cargo.toml, update members
   # Regenerate christina/ and christina-core/ with new Cargo.toml
   ```

### Phase 1: Core Domain Types (Low Risk)

**Goal**: Establish canonical types in `christina-core` with zero I/O.

**Tasks**:
1. Rewrite `christina-core/src/types/` 
2. Rewrite `christina-core/src/error.rs` unchanged
3. Create `christina-core/src/ids.rs` for `GenerationId` newtype
4. Delete `christina-git/src/` parallel git types
5. Rewrite `christina-core/src/git/` 

**Verification**: `cargo check -p christina-core` passes, no I/O deps.

### Phase 2: Config/Profile Consolidation (High Risk)

**Goal**: Single source of truth for configuration.

**Tasks**:
1. Create `christina-core/src/config/secret.rs` (`Secret<S>`, `SecretRef`, `SecretString`)
2. Create `christina-core/src/config/profile.rs` (`ProviderProfile<S>` enum)
3. Create `christina-core/src/config/config_file.rs` (`ConfigFile` serde struct)
4. Create `christina-core/src/config/resolved.rs` (`ResolvedConfig`)
5. Create `christina-core/src/config/validation.rs` (pure validation)
6. Rewrite Azure endpoint parsing to `core::config::AzureEndpoint` newtype
7. **Delete** `christina/src/config/settings.rs::Config` duplicate provider fields
8. Create `christina/src/io/config_io.rs` (load/save/resolve)

**Breaking Changes**:
- All existing config loading code breaks
- Profile CLI commands need rewrite to use new types
- Config TUI needs update to use `ProviderProfile<S>` enum

**Verification**: Profile loading tests pass, config persistence works.

### Phase 3: State Consolidation (High Risk)

**Goal**: Single `Model` in core, eliminate duplication.

**Tasks**:
1. Create `christina-core/src/app/model.rs` (`Model`, `Route`, `Screens`)
2. Rewrite screen state structs to `christina-core/src/app/screens/*.rs`
3. **Delete** `christina/src/tui/context.rs` (`UiState`, `DataState`)
4. **Delete** `christina/src/app/state.rs` (`TuiUiState`, `TuiSessionData` wrappers)
5. Rewrite selection state to `StagingState` (single owner)
6. Delete bidirectional sync code in `components_elm.rs`
7. Create `christina/src/runtime/state.rs` (`RuntimeState`, `App`)

**Breaking Changes**:
- Every file referencing `TuiSessionData` breaks (~50+ files)
- Screen initialization logic needs rewrite
- State access patterns change throughout

**Verification**: TUI compiles, state transitions work, no duplication.

### Phase 4: Elm Architecture Update (Medium Risk)

**Goal**: Core owns `update()`, bin owns `view()`.

**Tasks**:
1. Create `christina-core/src/app/msg.rs` (global `Msg` enum)
2. Create `christina-core/src/app/cmd.rs` (global `Cmd` enum)
3. Create `christina-core/src/app/update.rs` (global `update()` entry point)
4. Rewrite screen `update()` functions to core screens/*.rs, return `Vec<Cmd>`
5. Create `christina/src/runtime/cmd_exec.rs` (execute `Cmd` → `Msg`)
6. Update TUI event loop to call `core::update()`, execute `Cmd`, feed `Msg`

**Breaking Changes**:
- `AppMsg` enum replaced by `Cmd`
- `App::handle_app_msg()` replaced by `cmd_exec::execute_cmd()`
- All screen update functions change signature

**Verification**: Event loop works, Cmd/Msg flow validated.

### Phase 5: Provider Architecture (Medium Risk)

**Goal**: Core owns config, bin owns provider.

**Tasks**:
1. Create `christina-core/src/llm/` (request, provider_spec, prompt, tokens, retry)
2. **Delete** `christina-llm/src/provider.rs::Provider` enum
3. Create `christina/src/io/llm/*.rs` (HTTP client adapters)
4. Update `cmd_exec.rs` to route LLM requests to adapters
5. Rewrite prompt builders to `christina-core/src/llm/prompt.rs`

**Breaking Changes**:
- `AIOrchestrator` needs rewrite to use `LlmRequest`/`LlmResponse`
- Provider selection logic moves to core

**Verification**: LLM generation works end-to-end.

### Phase 6: Git Adapter (Low Risk)

**Goal**: Bin adapter returns core types.

**Tasks**:
1. Create `christina/src/io/git/adapter.rs` (git2 → `core::git::RepoSnapshot`)
2. **Delete** `christina-git/` entire crate
3. Update git operations to use adapter

**Verification**: Git status, staging, commit work.

### Phase 7: Polish & Verification

**Tasks**:
1. Run `cargo check --workspace`
2. Run `cargo clippy --workspace`
3. Run `cargo test --workspace`
4. Verify CI check: `cargo tree -p christina-core | grep tokio` fails (good)
5. Test TUI end-to-end
6. Test CLI commands

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

## Effort Estimate

**Critical Path**: Phase 2 → Phase 3 → Phase 4 (config + state + Elm must be sequential).
**Parallelizable**: Phase 5 (provider) and Phase 6 (git) can happen in parallel after Phase 4.

---

## Success Criteria

### Functional Requirements

- [ ] All existing CLI commands work (`christina init`, `christina generate`)
- [ ] TUI works end-to-end (staging → generation → review → commit)
- [ ] Config/profile management works (load, save, profiles TUI)
- [ ] LLM generation works with all providers (OpenAI, Azure)
- [ ] Git operations work (status, stage, commit)
- [ ] Tests pass (`cargo test --workspace`)

### Architectural Requirements

- [ ] `christina-core` has **zero I/O dependencies** (verified by CI check)
- [ ] `cargo tree -p christina-core | grep -E '(tokio|reqwest|git2)' && exit 1` passes
- [ ] No duplicate state representations (Config, Provider, GitFile)
- [ ] Single `Model` in core (no `TuiSessionData` wrapper)
- [ ] Elm Architecture preserved (pure `update()`, `Cmd`/`Msg` flow)
- [ ] No bidirectional sync code (state lives in ONE place)

### Code Quality

- [ ] `cargo check --workspace` passes
- [ ] `cargo clippy --workspace` passes with zero warnings
- [ ] All tests pass
- [ ] No `#[allow(dead_code)]` on public API items
- [ ] No `unwrap()` or `expect()` in production code (workspace lints enforced)

---

## Implementation Notes for AI Agents

### Do Not Skip

1. **Backup first**: `backup/` folder preserves context if needed.
4. **Test coverage**: Do not delete existing tests, adapt them to new structure.

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

## Conclusion

This redesign eliminates **4 crates of fragmentation** into **2 hermetic crates** with strict boundaries enforced by Cargo.toml, CI checks, and the type system. No OOP patterns, no architectural ceremony—just Rust-native design with Sans-IO core and thin I/O shell.

The resulting architecture is:
- **Cognitively simple**: Developers know where to find things
- **Hermetic**: Core has zero I/O, fully testable without mocking
- **Normalized**: One canonical representation per concept
- **Aesthetically coherent**: Rust purist, idiomatic, maintainable

Implementation authority: **Full autonomy**. AI agents may adjust implementation details as long as architectural invariants hold.
