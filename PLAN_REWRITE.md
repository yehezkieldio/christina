# Christina Workspace Architectural Rewrite Plan

## Status: HARDENED ARCHITECTURE (Post Stress-Test)

**Date**: January 2026  
**Analysis Result**: NO-GO on original plan due to 4 critical invariant violations  
**This Document**: Hardened architecture with enforced boundaries  

---

## Critical Issues Found in Stress Test

### Issue 1: Config I/O Was Conventional, Not Enforced
**Original Plan**: "Config struct in library, I/O functions in binary"  
**Problem**: Relying on convention allows drift. Future dev adds `Config::load_for_test()` to library.  
**Fix**: `Config` struct has ZERO I/O methods. `ConfigLoader` struct in binary owns ALL I/O.

### Issue 2: Repository Was Concrete, Not Abstracted
**Original Plan**: Binary uses `GitRepository` directly from library  
**Problem**: Cannot support Mercurial/SVN without rewriting binary. Tests must mock git2.  
**Fix**: Library defines `Repository` trait. Binary uses `Box<dyn Repository>`.

### Issue 3: State Mutation Authority Was Distributed
**Original Plan**: Screens directly mutate `app.data.base.staged_files`  
**Problem**: Two sources of truth. Screens drift from App state. Race conditions.  
**Fix**: `StateMachine` owns ALL state mutations. Screens send `AppMsg`, don't mutate global state.

### Issue 4: Library Had Direct stderr Output
**Original Plan**: Orchestrator kept as-is with `eprintln!` debug statements  
**Problem**: Library not hermetic. Web API mode impossible. Test output polluted.  
**Fix**: Injected `Logger` trait. Library uses `&dyn Logger`, binary provides implementation.

### Issue 5: StateMachine Didn't Own Screen States
**Original Plan**: Screen states stored in `App.data`, manually cleared on transition  
**Problem**: Developer must remember to clear 6 different fields. Guaranteed bugs.  
**Fix**: `StateMachine` owns all `Option<ScreenState>`. Automatic cleanup on transition.

---

## Hardened Architectural Invariants (Non-Negotiable)

### Invariant 1: Library Has Zero I/O
- No file system operations
- No environment variable access (except at binary boundary)
- No stderr/stdout writes
- No terminal access
- No network calls (except via injected traits)

**Enforcement**: Code review + clippy lints

### Invariant 2: Type System Enforces Boundaries
- `Config` cannot load itself (no `load()` method)
- `Repository` is a trait (not concrete)
- `StateMachine` owns all mutable state
- Screens receive immutable refs, return messages

**Enforcement**: Compiler

### Invariant 3: Single Authority for State
- Only `StateMachine` can mutate global state
- Screens mutate only local state
- Async tasks send events, don't mutate directly
- Git operations go through `Repository` trait

**Enforcement**: API design + code review

### Invariant 4: Dependency Direction Is Strict
```
Binary -> Library (always)
Binary -> git2, ratatui, crossterm (I/O crates)
Library -> serde, regex (pure crates)
Library -/-> std::fs, std::env (I/O)
```

**Enforcement**: Cargo.toml + CI check

---

## Hardened Crate Structure

### Crate: `christina` (Binary - I/O and TUI)

**Responsibility**: ALL I/O, user interaction, runtime coordination

```
christina/
├── Cargo.toml
└── src/
    ├── main.rs                 # Entry point only
    ├── cli.rs                  # Clap definitions
    ├── config/
    │   ├── mod.rs              # Config module
    │   ├── loader.rs           # ConfigLoader - owns ALL config I/O
    │   └── commands.rs         # CLI command handlers
    ├── repository.rs           # Repository trait implementations
    ├── logger.rs               # Logger trait implementation
    ├── tui/
    │   ├── mod.rs              # TUI exports
    │   ├── app.rs              # TuiApp - owns terminal, event loop
    │   ├── event_loop.rs       # Event loop ONLY (no generation logic)
    │   ├── terminal.rs         # Terminal management
    │   ├── theme.rs            # Colors
    │   ├── elm.rs              # Component trait + AppMsg
    │   ├── components_elm.rs   # Screen dispatcher
    │   ├── screens/            # 6 screens (pure render/update)
    │   │   ├── mod.rs
    │   │   ├── staging.rs
    │   │   ├── dashboard.rs
    │   │   ├── generating.rs
    │   │   ├── review.rs
    │   │   ├── editing.rs
    │   │   └── error.rs
    │   ├── components/         # Reusable widgets
    │   │   ├── mod.rs
    │   │   ├── file_list.rs
    │   │   └── diff_renderer.rs
    │   ├── widgets/            # Low-level widgets
    │   │   ├── mod.rs
    │   │   └── toast.rs
    │   ├── config_tui/         # Config TUI flow
    │   └── profiles_tui/       # Profile TUI flow
    └── app/
        └── edit_history.rs     # Undo/redo (UI concern)
```

**Dependencies**:
- `christina-core` (domain library)
- `ratatui`, `crossterm`, `tui-textarea` (TUI)
- `clap` (CLI)
- `tokio` (async runtime)
- `directories`, `toml`, `config` (config I/O)
- `ansi-to-tui` (diff rendering)
- `git2` (git operations - wrapped)

### Crate: `christina-core` (Library - Pure Domain Logic)

**Responsibility**: Domain types, pure logic, ZERO I/O

```
christina-core/
├── Cargo.toml
└── src/
    ├── lib.rs                  # Public API exports
    ├── types/                  # Core domain types
    │   ├── mod.rs              # Type re-exports
    │   ├── commit.rs           # CommitMessage + ValidationMode
    │   ├── provider.rs         # ProviderKind enum
    │   ├── model.rs            # ModelName newtype
    │   ├── tokens.rs           # TokenCount newtype
    │   └── file.rs             # FilePath newtype
    ├── config/
    │   └── mod.rs              # Config struct (NO I/O methods!)
    ├── state/                  # State management
    │   ├── mod.rs              # StateMachine - owns ALL state
    │   ├── machine.rs          # Transition logic
    │   └── generation.rs       # Generation tracking
    ├── git/
    │   ├── mod.rs              # Repository trait + types
    │   ├── repo.rs             # GitRepository impl
    │   ├── operations.rs       # Stage/unstage/commit
    │   └── diff.rs             # Diff generation
    ├── llm/
    │   ├── mod.rs              # LLM exports
    │   ├── provider.rs         # Provider trait + impls
    │   ├── orchestrator.rs     # Map-reduce generation (NO eprintln!)
    │   ├── prompt.rs           # Prompt templates
    │   └── tokenizer.rs        # Token counting
    ├── generation/
    │   └── service.rs          # GenerationService facade
    ├── error.rs                # Domain errors (semantic only)
    └── logging.rs              # Logger trait definition
```

**Dependencies**:
- `serde` (serialization)
- `thiserror`, `anyhow` (errors)
- `url`, `regex`, `compact_str` (utilities)
- `tokio` (async traits only)
- `llm`, `tiktoken-rs` (AI/LLM)
- `git2` (git operations)

**Explicitly NOT in dependencies**:
- `directories` (file paths - I/O)
- `toml` (parsing - I/O)
- `config` (config loading - I/O)
- `ratatui`, `crossterm` (terminal - I/O)

---

## Critical API Changes

### 1. Config Loading (Enforced Separation)

```rust
// christina-core/src/config/mod.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub max_input_tokens: TokenCount,
    pub max_output_tokens: TokenCount,
    pub model_provider: ProviderKind,
    // ... fields only, NO methods that do I/O
}

impl Config {
    // Pure validation only
    pub fn validate(&self) -> Result<(), ValidationError>;
    pub fn apply_profile(&mut self, profile: &ProviderProfile);
    
    // NO load(), NO save(), NO env var access
}

// christina/src/config/loader.rs
pub struct ConfigLoader;

impl ConfigLoader {
    pub fn load() -> Result<Config> {
        // File I/O, env vars, directory resolution
    }
    
    pub fn save(config: &Config) -> Result<()> {
        // File I/O
    }
    
    pub fn load_from_path(path: &Path) -> Result<Config> {
        // For tests
    }
}
```

### 2. Repository Abstraction (Trait-Based)

```rust
// christina-core/src/git/mod.rs
pub trait Repository: Send + Sync {
    fn workdir(&self) -> Option<&Path>;
    fn get_staged_files(&self) -> Result<Vec<GitFile>>;
    fn get_unstaged_files(&self) -> Result<Vec<GitFile>>;
    fn stage_files(&self, files: &[(PathBuf, GitFileStatus)]) -> Result<()>;
    fn unstage_files(&self, paths: &[PathBuf]) -> Result<()>;
    fn create_commit(&self, message: &CommitMessage) -> Result<Oid>;
    fn has_staged_changes(&self) -> Result<bool>;
    fn validate_for_commit(&self) -> Result<()>;
    fn get_commit_history(&self, limit: usize) -> Result<Vec<CommitInfo>>;
}

// Git implementation in library
pub struct GitRepository { inner: git2::Repository }
impl Repository for GitRepository { ... }

// christina/src/repository.rs - Binary side
pub fn create_repository() -> Result<Box<dyn Repository>> {
    Ok(Box::new(GitRepository::discover()?))
}
```

### 3. State Machine (Owns ALL State)

```rust
// christina-core/src/state/machine.rs
pub struct StateMachine {
    current: AppState,
    generation_id: u64,
    // ALL screen states owned here
    staging_state: Option<StagingState>,
    dashboard_state: Option<DashboardState>,
    generating_state: Option<GeneratingState>,
    review_state: Option<ReviewState>,
    editing_state: Option<EditingState>,
    error_state: Option<ErrorState>,
    // Global data
    staged_files: Vec<GitFile>,
    unstaged_files: Vec<GitFile>,
    generated_message: CompactString,
    // ... other global state
}

impl StateMachine {
    pub fn transition(&mut self, to: AppState) -> Result<TransitionResult, TransitionError> {
        // Validate
        self.can_transition(&self.current, &to)?;
        
        // Automatic cleanup of old state
        match self.current {
            AppState::Generating => self.generating_state = None,
            AppState::Review => self.review_state = None,
            AppState::Editing => self.editing_state = None,
            AppState::Error => self.error_state = None,
            _ => {}
        }
        
        // Lazy initialization of new state
        match to {
            AppState::Staging if self.staging_state.is_none() => {
                self.staging_state = Some(StagingState::new(self.staged_files.clone()));
            }
            AppState::Dashboard if self.dashboard_state.is_none() => {
                self.dashboard_state = Some(DashboardState::new(self.staged_files.clone()));
            }
            AppState::Generating if self.generating_state.is_none() => {
                let id = self.next_generation_id();
                self.generating_state = Some(GeneratingState::new(id));
            }
            AppState::Review if self.review_state.is_none() => {
                let msg = CommitMessage::try_from(self.generated_message.to_string())?;
                self.review_state = Some(ReviewState::new(msg));
            }
            AppState::Editing if self.editing_state.is_none() => {
                let msg = self.generated_message.to_string();
                self.editing_state = Some(EditingState::new(msg));
            }
            AppState::Error if self.error_state.is_none() => {
                let err = self.error_message.clone().unwrap_or_default();
                self.error_state = Some(ErrorState::new(err, !self.staged_files.is_empty()));
            }
            _ => {}
        }
        
        self.current = to;
        Ok(TransitionResult { previous: self.current })
    }
    
    // State access (immutable only)
    pub fn current_state(&self) -> AppState;
    pub fn staging_state(&self) -> Option<&StagingState>;
    pub fn staging_state_mut(&mut self) -> Option<&mut StagingState>;
    // ... etc for all states
    
    // Global state mutations (controlled)
    pub fn update_staged_files(&mut self, files: Vec<GitFile>) {
        self.staged_files = files;
        // Invalidate derived states
        self.dashboard_state = None;
        self.staging_state = None;
    }
    
    pub fn set_generated_message(&mut self, message: CompactString) {
        self.generated_message = message;
        self.review_state = None; // Force rebuild
    }
}
```

### 4. Logger Injection (No Direct stderr)

```rust
// christina-core/src/logging.rs
pub trait Logger: Send + Sync {
    fn debug(&self, msg: &str);
    fn info(&self, msg: &str);
    fn warn(&self, msg: &str);
    fn error(&self, msg: &str);
}

// No-op logger for library defaults
pub struct NoopLogger;
impl Logger for NoopLogger {
    fn debug(&self, _msg: &str) {}
    fn info(&self, _msg: &str) {}
    fn warn(&self, _msg: &str) {}
    fn error(&self, _msg: &str) {}
}

// christina/src/logger.rs
pub struct StderrLogger;
impl Logger for StderrLogger {
    fn debug(&self, msg: &str) { eprintln!("[DEBUG] {}", msg); }
    // ... etc
}

// Usage in orchestrator
pub struct AIOrchestrator {
    provider: Arc<dyn Provider>,
    logger: Arc<dyn Logger>,
    // ...
}

impl AIOrchestrator {
    pub fn new(provider: Arc<dyn Provider>) -> Self {
        Self {
            provider,
            logger: Arc::new(NoopLogger), // Default to no-op
            // ...
        }
    }
    
    pub fn with_logger(mut self, logger: Arc<dyn Logger>) -> Self {
        self.logger = logger;
        self
    }
    
    async fn map_phase(&self, chunks: &[DiffChunk]) -> Result<...> {
        self.logger.debug(&format!("Starting map phase with {} chunks", chunks.len()));
        // ...
    }
}
```

### 5. Binary App Structure (Thin Coordinator)

```rust
// christina/src/tui/app.rs
pub struct TuiApp {
    // I/O handles
    terminal: TerminalHandle,
    repository: Box<dyn Repository>,
    config: Config,
    
    // Domain logic (from library)
    state_machine: StateMachine,
    generation_service: GenerationService,
    
    // UI-only state
    should_quit: bool,
    exit_message: Option<String>,
    textarea: TextArea<'static>,
    toasts: ToastManager,
    edit_history: EditHistory,
    
    // Async
    generation_tx: mpsc::Sender<Event>,
}

impl TuiApp {
    pub fn new(terminal: TerminalHandle) -> Result<Self> {
        let config = ConfigLoader::load()?;
        let repo = create_repository()?;
        let state_machine = StateMachine::new();
        let generation_service = GenerationService::new(config.clone())
            .with_logger(Arc::new(StderrLogger));
        
        Ok(Self {
            terminal,
            repository: repo,
            config,
            state_machine,
            generation_service,
            should_quit: false,
            exit_message: None,
            textarea: TextArea::default(),
            toasts: ToastManager::new(),
            edit_history: EditHistory::new(),
            generation_tx,
        })
    }
    
    // Handle messages from screens (single authority)
    pub fn handle_app_msg(&mut self, msg: AppMsg) {
        match msg {
            AppMsg::StageFile(path) => self.handle_stage_file(path),
            AppMsg::Navigate(state) => self.transition_to(state),
            AppMsg::GenerateMessage => self.start_generation(),
            // ... etc
        }
    }
    
    fn transition_to(&mut self, state: AppState) {
        if let Err(e) = self.state_machine.transition(state) {
            self.toasts.error(format!("Invalid transition: {}", e));
            return;
        }
        self.should_redraw = true;
    }
    
    fn handle_stage_file(&mut self, path: FilePath) {
        // Use repository trait
        if let Err(e) = self.repository.stage_files(&[(path.into(), GitFileStatus::Added)]) {
            self.toasts.error(format!("Failed to stage: {}", e));
        } else {
            // Update state through StateMachine
            let files = self.repository.get_staged_files().unwrap_or_default();
            self.state_machine.update_staged_files(files);
            self.toasts.success("File staged");
        }
    }
    
    async fn start_generation(&mut self) {
        let id = self.state_machine.start_generation();
        let config = self.config.clone();
        let user_context = self.state_machine.user_context().cloned();
        let repo_path = self.repository.workdir().map(|p| p.to_path_buf());
        
        // Spawn async task using library service
        let tx = self.generation_tx.clone();
        tokio::spawn(async move {
            let result = self.generation_service.generate(
                repo_path,
                user_context.as_deref(),
                tx.clone(),
            ).await;
            
            match result {
                Ok(generation_result) => {
                    let _ = tx.send(Event::GenerationComplete {
                        message: generation_result.message,
                        warning_summary: generation_result.warning_summary(),
                        generation_id: id,
                    }).await;
                }
                Err(e) => {
                    let _ = tx.send(Event::GenerationError {
                        error: e.to_string(),
                        generation_id: id,
                    }).await;
                }
            }
        });
    }
}
```

---

## Migration Phases (Hardened)

With continuous and autonomous AI coding agents, this should be done in few minutes to hours.

### Phase 1: Library Hardening

1. **Create Repository trait**
   - Define trait in `christina-core/src/git/mod.rs`
   - Move `GitRepository` to `christina-core/src/git/repo.rs`
   - Remove direct git2 usage from binary

2. **Remove I/O from Config**
   - Remove `load()`, `save_to_global()` from `Config`
   - Remove env var reading from `Config`
   - Keep struct definition and validation only

3. **Add Logger trait**
   - Define `Logger` trait in library
   - Replace all `eprintln!` in orchestrator with logger calls
   - Default to no-op logger

4. **StateMachine owns screen states**
   - Move `staging_state`, `dashboard_state`, etc. into `StateMachine`
   - Implement automatic cleanup on transition
   - Implement lazy initialization

**Quality Gate**: `cd christina-core && cargo check` with zero warnings

### Phase 2: Binary Hardening

1. **Create ConfigLoader**
   - Move all config I/O to `christina/src/config/loader.rs`
   - Update all call sites to use `ConfigLoader`

2. **Implement Repository trait in binary**
   - Create `christina/src/repository.rs`
   - Function to create `Box<dyn Repository>`
   - Update `App` to use trait object

3. **Create StderrLogger**
   - Implement `Logger` trait for stderr output
   - Inject into `GenerationService` in `App::new()`

4. **Migrate to StateMachine ownership**
   - Remove state fields from `App`
   - Update all screen access to go through `state_machine`
   - Ensure screens only mutate via `AppMsg`

**Quality Gate**: `cd christina && cargo check` with zero warnings

### Phase 3: Delete Old Crates

1. Delete `christina-git/` directory
2. Delete `christina-llm/` directory
3. Update workspace `Cargo.toml`

### Phase 4: Quality Gates

```bash
just check    # cargo check with zero warnings
just clippy   # cargo clippy with zero warnings
```

### Phase 5: Verification

All original test cases plus:
- [ ] Config I/O has zero methods on `Config` struct
- [ ] `Repository` is a trait, not concrete type in binary
- [ ] No `eprintln!` in library crates
- [ ] State mutations only through `StateMachine`
- [ ] Web API can be built using library (compile check)

---

## Success Criteria (Hardened)

1. ✅ Exactly 2 crates: `christina` (binary), `christina-core` (library)
2. ✅ Zero compiler warnings (`just check` passes)
3. ✅ Zero clippy warnings (`just clippy` passes)
4. ✅ All existing tests pass
5. ✅ **Config has ZERO I/O methods** (enforced by types)
6. ✅ **Repository is a trait** (can swap VCS implementation)
7. ✅ **Library has zero terminal/UI dependencies**
8. ✅ **Library has zero stderr/stdout writes**
9. ✅ **StateMachine owns ALL mutable state**
10. ✅ All 6 TUI screens functional
11. ✅ Config I/O works via `ConfigLoader`
12. ✅ Profile management works
13. ✅ Generation pipeline works end-to-end
14. ✅ GPG signing preserved
15. ✅ EditHistory preserved

---

## Risk Mitigation

### High Risk: Trait Object Overhead
**Concern**: `Box<dyn Repository>` has virtual call overhead  
**Mitigation**: Measure before optimizing. Git operations are I/O bound, not CPU bound.

### Medium Risk: StateMachine Complexity
**Concern**: Centralized state is more complex than distributed  
**Mitigation**: Well-tested transition table. Compiler enforces state access patterns.

### Low Risk: Logger Injection Ceremony
**Concern**: Every library struct needs logger field  
**Mitigation**: Use `Arc<dyn Logger>` - cheap to clone, no lifetime complexity.

---

## Conclusion

This hardened architecture ensures:

1. **Library is truly hermetic** - Can be used by TUI, CLI, web API, or tests
2. **Boundaries are enforced** - Type system prevents I/O in library
3. **Single authority** - StateMachine prevents race conditions and drift
4. **Future-proof** - Repository trait allows new VCS, Logger allows new outputs

**GO with hardened architecture.**
