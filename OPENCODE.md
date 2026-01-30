Architectural Analysis & Redesign Proposal
1. High-Level Architectural Rationale
The current 4-crate structure suffers from over-fragmentation and boundary erosion. The christina-git and christina-llm crates are thin abstraction layers that don't justify their overhead—they're essentially wrappers around git2 and llm/tiktoken-rs with minimal added semantic value. Meanwhile, the christina crate has become a god module mixing TUI rendering, event loops, orchestration, CLI parsing, configuration management, and application state.
Proposed Architecture (2 crates):
┌─────────────────────────────────────────────────────────────┐
│                      christina (binary)                      │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
│  │     CLI     │  │     TUI     │  │    Event Loop       │  │
│  │   (clap)    │  │  (ratatui)  │  │  (async/await)      │  │
│  └─────────────┘  └─────────────┘  └─────────────────────┘  │
└────────────────────┬────────────────────────────────────────┘
                     │ depends on
                     ▼
┌─────────────────────────────────────────────────────────────┐
│                  christina-core (library)                    │
│  ┌────────────┐  ┌────────────┐  ┌───────────────────────┐  │
│  │   Domain   │  │    Git     │  │   LLM Integration     │  │
│  │   Types    │  │  Engine    │  │  (Orchestration)      │  │
│  │            │  │            │  │                       │  │
│  │ - State    │  │ - Repository│ │ - Provider           │  │
│  │ - Profiles │  │ - Diff      │  │ - Tokenization       │  │
│  │ - Messages │  │ - Chunking  │  │ - Map-Reduce         │  │
│  └────────────┘  └────────────┘  └───────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
Rationale:
- Binary crate responsibility: Entry point, CLI/TUI interfaces, event coordination, user interaction. No domain logic.
- Library crate responsibility: All domain logic, git operations, LLM orchestration, types, and configuration models.
- Clear boundary: UI concerns (rendering, input handling, events) vs. domain concerns (git, LLM, state machines, business rules).
---
2. Proposed Workspace Layout
/home/yehezkieldio/Documents/Scratchpad/christina-vibe/
├── Cargo.toml                    # Workspace definition (2 members)
├── Cargo.lock
├── justfile
├── README.md
├── AGENTS.md
├── LICENSE-*
│
├── crates/
│   ├── christina/                # Binary crate (TUI application)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── main.rs           # Entry point, panic handling
│   │       ├── cli.rs            # Clap CLI definition (minimal)
│   │       ├── app.rs            # App orchestration (thin)
│   │       ├── events.rs         # Event loop + async coordination
│   │       ├── tui/
│   │       │   ├── mod.rs        # TUI module exports
│   │       │   ├── screens/      # One module per screen (refactored)
│   │       │   │   ├── mod.rs    # Screen router
│   │       │   │   ├── staging.rs    # ~400 lines (extracted logic)
│   │       │   │   ├── dashboard.rs  # ~400 lines
│   │       │   │   ├── review.rs     # ~400 lines
│   │       │   │   ├── generating.rs # ~200 lines
│   │       │   │   ├── editing.rs    # ~300 lines
│   │       │   │   └── error.rs      # ~150 lines
│   │       │   ├── components/   # Reusable UI components
│   │       │   │   ├── mod.rs
│   │       │   │   ├── file_list.rs
│   │       │   │   ├── diff_view.rs
│   │       │   │   ├── commit_preview.rs
│   │       │   │   ├── form.rs
│   │       │   │   ├── toast.rs
│   │       │   │   └── theme.rs
│   │       │   └── input.rs      # Input handling abstraction
│   │       └── assets/           # Static assets (themes, etc.)
│   │
│   └── christina-core/           # Library crate (domain)
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs            # Public API exports
│           │
│           ├── config.rs         # Configuration types + loading (merged)
│           ├── error.rs          # Error types (consolidated)
│           ├── state.rs          # AppState, StateMachine
│           │
│           ├── domain/
│           │   ├── mod.rs
│           │   ├── commit.rs     # CommitMessage, validation
│           │   ├── file.rs       # GitFile, FilePath, status
│           │   ├── diff.rs       # DiffChunk, FileDiff
│           │   ├── profile.rs    # ProviderProfile, Profiles
│           │   ├── provider.rs   # ProviderKind, ModelName
│           │   └── token.rs      # TokenCount, Tokenizer trait
│           │
│           ├── git/
│           │   ├── mod.rs
│           │   ├── repo.rs       # GitRepository (was repository.rs)
│           │   ├── diff.rs       # Diff processing, chunking
│           │   ├── chunking.rs   # Recursive algorithms
│           │   ├── parsing.rs    # Diff parsing
│           │   └── buffer.rs     # Buffer pool
│           │
│           ├── llm/
│           │   ├── mod.rs
│           │   ├── orchestrator.rs   # AIOrchestrator, Map-Reduce
│           │   ├── provider.rs       # Provider enum + impls
│           │   ├── tokenizer.rs      # TokenizerService
│           │   ├── concurrency.rs    # RequestLimiter
│           │   ├── retry.rs          # RetryPolicy
│           │   ├── prompts.rs        # Prompt templates (moved from prompt.rs)
│           │   └── providers/        # Provider implementations
│           │       ├── openai.rs
│           │       ├── azure.rs
│           │       └── http.rs
│           │
│           └── generation/       # NEW: Commit generation pipeline
│               ├── mod.rs
│               ├── pipeline.rs   # Orchestrates git+LLM+history
│               ├── progress.rs   # Progress events (UI-agnostic)
│               └── history.rs    # Commit history context
│
└── tests/                        # Integration tests
    ├── git_integration.rs
    ├── llm_integration.rs
    └── e2e.rs
---
3. Specific Recommendations
A. Delete the christina-git and christina-llm crates
Why: These are dumping-ground crates with thin abstractions. The git layer is a direct wrapper around git2. The LLM layer is essentially just the orchestration logic that belongs in core domain. They create artificial boundaries that force you to expose internal types as public API surface.
Migration:
- Move christina-git/src/repository.rs → christina-core/src/git/repo.rs
- Move christina-git/src/diff_processor.rs → christina-core/src/git/diff.rs
- Move christina-git/src/chunking.rs → christina-core/src/git/chunking.rs
- Move christina-git/src/parsing.rs → christina-core/src/git/parsing.rs
- Move christina-git/src/buffer_pool.rs → christina-core/src/git/buffer.rs
- Move christina-llm/src/orchestrator.rs → christina-core/src/llm/orchestrator.rs
- Move christina-llm/src/provider.rs → christina-core/src/llm/provider.rs
- Move christina-llm/src/tokenizer.rs → christina-core/src/llm/tokenizer.rs
- Move christina-llm/src/concurrency.rs → christina-core/src/llm/concurrency.rs
- Move christina-llm/src/retry.rs → christina-core/src/llm/retry.rs
- Move christina-llm/src/providers/ → christina-core/src/llm/providers/
B. Consolidate configuration into christina-core
Current problem: Configuration types are split between:
- christina-core/src/profile.rs (ProviderProfile, Profiles)
- christina/src/config/settings.rs (Config with layered loading)
- christina/src/config/diff_tool.rs (DiffTool, DiffConfig)
Solution: Create single christina-core/src/config.rs module containing:
// Unified configuration
pub struct Config {
    pub active_profile: String,
    pub profiles: Profiles,
    pub diff: DiffConfig,
    pub limits: TokenLimits,
    pub history: HistoryConfig,
}
pub struct Profiles(HashMap<String, ProviderProfile>);
pub struct ProviderProfile {
    pub provider: ProviderKind,
    pub model: ModelName,
    pub api_key: SecretString, // Use secrecy crate
    pub api_url: Option<Url>,
    pub azure_deployment: Option<String>,
    pub max_input_tokens: TokenCount,
    pub max_output_tokens: TokenCount,
}
pub struct DiffConfig {
    pub external_tool: Option<String>,
    pub ignore_files: Vec<String>,
}
Why: Configuration is domain logic—it's the schema of how the application can be tuned. It doesn't belong in the UI crate. The TUI only consumes the loaded configuration.
C. Extract generation pipeline from christina/src/generate.rs
Current problem: generate.rs mixes:
- Config-to-profile conversion
- UI-specific event sending
- API key handling (UI prompt)
- Git diff retrieval
- Token budgeting
- History context loading
- LLM orchestration call
Solution: Create christina-core/src/generation/ module:
// christina-core/src/generation/pipeline.rs
pub struct GenerationPipeline {
    repo: GitRepository,
    profile: ProviderProfile,
    history: Option<CommitHistory>,
}
pub struct GenerationProgress {
    pub stage: GenerationStage,
    pub token_count: Option<TokenCount>,
    pub message: Option<String>,
}
pub enum GenerationStage {
    LoadingDiff,
    ProcessingChunks(u32, u32), // (completed, total)
    Generating,
    Complete,
}
impl GenerationPipeline {
    pub async fn run(
        &self,
        on_progress: impl Fn(GenerationProgress),
    ) -> Result<CommitMessage, GenerationError>;
}
The TUI/event loop provides the progress callback; the core handles the actual pipeline.
D. Split massive TUI screen files
Current problem: Files like dashboard.rs (25K lines), staging.rs (11K lines), review.rs (12K lines) are god modules mixing rendering, state management, event handling, and business logic.
Solution: Each screen module should be ~200-500 lines max:
// christina/src/tui/screens/staging.rs
pub struct StagingScreen {
    file_list: FileListComponent,
    diff_preview: DiffPreviewComponent,
    help_bar: HelpBarComponent,
}
impl Screen for StagingScreen {
    fn render(&self, frame: &mut Frame, area: Rect);
    fn handle_input(&mut self, key: KeyEvent) -> ScreenResult;
    fn on_mount(&mut self, data: &AppData);
}
Extract shared components:
- FileListComponent: Reusable file list with selection, filtering, scrolling
- DiffPreviewComponent: Diff rendering with syntax highlighting
- CommitPreviewComponent: Message preview with validation
- FormComponent: Generic form with fields, validation, navigation
- HelpBarComponent: Context-sensitive key bindings
E. Merge AppState from TUI with domain state
Current: christina-core/src/state.rs has AppState enum; TUI has its own UI state management.
Solution: Single source of truth in christina-core:
// christina-core/src/state.rs
pub enum AppState {
    StagingSelection { selected: Vec<FilePath> },
    Dashboard { 
        diff: StagedDiff,
        can_commit: bool,
    },
    Generating {
        started_at: Instant,
        progress: GenerationProgress,
    },
    Review {
        message: CommitMessage,
        can_edit: bool,
    },
    Editing {
        message: CommitMessage,
        cursor: CursorPosition,
    },
    Error(AppError),
}
pub struct StateMachine {
    state: AppState,
    history: Vec<AppState>,
}
impl StateMachine {
    pub fn transition(&mut self, action: Action) -> Result<(), InvalidTransition>;
    pub fn current(&self) -> &AppState;
    pub fn can(&self, action: Action) -> bool;
}
The TUI simply renders the current state and dispatches actions to the state machine.
F. Create UI-agnostic progress/events
Current problem: christina/src/event_loop/mod.rs has UI-specific Event enum with TUI concerns leaking into the async coordination.
Solution: Two-layer event system:
// christina-core/src/generation/progress.rs (domain)
pub enum GenerationEvent {
    Started,
    DiffLoaded { token_count: TokenCount },
    ChunkProcessed { completed: u32, total: u32 },
    IntentExtracted { themes: Vec<String> },
    MessageGenerated { message: CommitMessage },
    Failed { error: GenerationError },
}
// christina/src/events.rs (UI layer)
pub enum AppEvent {
    Generation(GenerationEvent),
    Input(KeyEvent),
    Tick,
    Resize(u16, u16),
    Toast(String),
}
G. Move prompt templates from core to LLM module
Current: christina-core/src/prompt.rs contains prompt templates.
Better location: christina-core/src/llm/prompts.rs since they're LLM-specific implementation details, not domain concepts.
---
4. Shared vs. UI-Specific Logic
| Shared (christina-core) | UI-Specific (christina) |
|------------------------------|------------------------------|
| Git repository operations | Terminal initialization/cleanup |
| Diff parsing and chunking | Input key event handling |
| LLM provider abstraction | Screen layout and rendering |
| Token counting/budgeting | Component composition |
| Commit message validation | User notifications (toasts) |
| Configuration loading | External diff tool spawning |
| State machine transitions | Event loop coordination |
| Retry and concurrency logic | Progress bar rendering |
| Prompt templates | Color theme application |
| Generation pipeline orchestration | Help text display |
Key principle: If logic doesn't require knowing it's running in a TUI, it belongs in christina-core. The christina crate is the "adapter" that connects core domain to terminal reality.
---
5. Consolidation Opportunities
A. Type Consolidation
Current duplication:
- christina-core/src/types/commit_message.rs + validation logic scattered
- christina-core/src/types/provider_kind.rs + christina-llm/src/provider.rs ProviderKind
Consolidate: Single domain/ module with:
- commit.rs: CommitMessage with validation
- provider.rs: ProviderKind, ModelName, ProviderProfile
- file.rs: FilePath, GitFile, GitFileStatus
- diff.rs: DiffChunk, FileDiff
- token.rs: TokenCount, TokenBudget
B. Error Consolidation
Current: christina-core/src/error.rs has comprehensive error types, but TUI adds its own error handling.
Consolidate: Single error hierarchy in christina-core/src/error.rs:
pub enum AppError {
    Git(GitError),
    LLM(LLMError),
    Config(ConfigError),
    Generation(GenerationError),
    Validation(ValidationError),
}
impl AppError {
    pub fn is_transient(&self) -> bool;
    pub fn user_message(&self) -> String;
    pub fn should_retry(&self) -> bool;
}
TUI only converts AppError to display strings—no error logic in UI layer.
C. Profile/Config Unification
Current: ProviderProfile (core) and Config (TUI) with overlapping fields.
Consolidate: Single Config struct with Profiles as a field:
pub struct Config {
    pub profiles: Profiles,           // HashMap<String, ProviderProfile>
    pub active_profile: String,       // Key into profiles
    pub diff: DiffConfig,
    pub history: HistoryConfig,
}
impl Config {
    pub fn active(&self) -> Option<&ProviderProfile>;
    pub fn load() -> Result<Self, ConfigError>;  // Layered loading
    pub fn save(&self) -> Result<(), ConfigError>;
}
D. Tokenizer Unification
Current: Tokenizer trait in core, TokenizerService impl in LLM crate.
Consolidate: Single implementation in christina-core/src/llm/tokenizer.rs implementing the trait. No need for trait if only one implementation—just use struct directly unless testing requires mocking.
E. Diff Processing Consolidation
Current: DiffProcessor (christina-git) + generate.rs has its own diff retrieval logic.
Consolidate: Single DiffEngine in christina-core/src/git/diff.rs:
pub struct DiffEngine {
    repo: GitRepository,
    tokenizer: Tokenizer,
    config: DiffConfig,
}
impl DiffEngine {
    pub fn get_staged(&self) -> Result<StagedDiff, GitError>;
    pub fn chunk_for_budget(&self, diff: &StagedDiff, budget: TokenBudget) -> Vec<DiffChunk>;
    pub fn is_binary(&self, path: &FilePath) -> bool;
}
---
6. Summary of Moves/Deletes/Merges
| Current Location | Action | New Location |
|---------------------|------------|------------------|
| christina-git/ entire crate | Delete | Move contents to christina-core/src/git/ |
| christina-llm/ entire crate | Delete | Move contents to christina-core/src/llm/ |
| christina-core/src/prompt.rs | Move | christina-core/src/llm/prompts.rs |
| christina-core/src/git/ (diff.rs, file.rs) | Merge | Into christina-core/src/domain/diff.rs, file.rs |
| christina/src/config/settings.rs | Move | christina-core/src/config.rs (merged) |
| christina/src/config/diff_tool.rs | Move | christina-core/src/config.rs |
| christina/src/config/cli.rs | Delete | Inline into christina/src/cli.rs |
| christina/src/generate.rs | Refactor | Split: pipeline → christina-core/src/generation/, UI events → christina/src/app.rs |
| christina/src/tui/screens/*.rs | Split | Extract components to christina/src/tui/components/ |
| christina/src/tui/components_elm.rs | Delete | Replace with screen-specific renderers |
| christina/src/app/context.rs | Delete | Merge into App struct or use AppData from core |
| christina/src/app/edit_history.rs | Move | christina-core/src/edit_history.rs (domain logic) |
| christina/src/tui/form/ | Refactor | Extract to christina/src/tui/components/form.rs |
| christina/src/tui/profiles/ | Refactor | Use generic form component, profile-specific logic in core |
| christina/src/tui/config/ | Refactor | Use generic form component |
---
This redesign achieves:
1. 2-crate workspace with clear binary/library separation
2. Domain purity in christina-core—no UI dependencies, testable in isolation
3. Thin UI layer in christina—only rendering, input, and event coordination
4. Eliminated fragmentation—no more thin wrapper crates
5. Consolidated configuration—single source of truth
6. Decomposed god modules—screens split into components, logic extracted to core
7. Explicit boundaries—core emits UI-agnostic events, UI translates to terminal
8. Maintainable scale—no more 25K line files, each module has single responsibility
The architecture prioritizes conceptual integrity over backward compatibility. It's a clean slate designed for growth, where adding a new screen means adding a small module in tui/screens/, and adding new LLM capabilities means extending christina-core/src/llm/ without touching UI code.
