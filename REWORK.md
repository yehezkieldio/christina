# Christina Architectural Redesign Plan

## 1. High-Level Architectural Rationale

The current architecture suffers from "Library-ification" of application code. Crates like `christina-git` and `christina-llm` are not generic libraries; they are specific data layers for the application. Separating them creates friction (workspace dependency management, public visibility requirements) without providing reusability.

We will consolidate the workspace into exactly two crates, adhering to a strict **Headless Core** architecture:

1.  **`christina-core` (The Engine)**
    *   **Concept:** A complete, headless version of the application.
    *   **Responsibility:** It owns the data, the state machine, the git repository, the LLM connection, and the configuration.
    *   **Constraint:** Zero dependencies on UI libraries (`ratatui`, `crossterm`). It returns plain Rust data types (`Structs`, `Enums`, `Strings`).
    *   **Goal:** You could theoretically write a new CLI or a GUI wrapping this crate without changing a single line of code in it.

2.  **`christina` (The Interface)**
    *   **Concept:** The View and Controller.
    *   **Responsibility:** Argument parsing, Event Loop, TUI Rendering, Input handling.
    *   **Constraint:** Zero domain logic. It translates User Events -> Core Actions, and Core State -> UI Widgets.

## 2. Proposed Workspace Layout

This layout flattens deeply nested modules and consolidates logic by domain.

```text
/
├── Cargo.toml                      # Workspace definition
├── justfile                        # Command runner
├── christina-core/                 # [CRATE] The Headless Application
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs                  # Exports Session, Config, Error
│       ├── session.rs              # The "God Object" (Controller of the Logic)
│       ├── error.rs                # Unified application Error enum
│       ├── prelude.rs              # Common imports (Result, Session, etc.)
│       ├── config/                 # Configuration Loading & Definition
│       │   ├── mod.rs
│       │   ├── file.rs             # File I/O for config
│       │   ├── theme.rs            # Theme definitions (data only)
│       │   └── settings.rs         # The Settings Struct
│       ├── git/                    # Git Operations (absorbed christina-git)
│       │   ├── mod.rs
│       │   ├── repository.rs       # Main GitRepository struct
│       │   ├── diff.rs             # Diff parsing and generation
│       │   ├── status.rs           # Status checking logic
│       │   └── operations.rs       # Commit, Stage, Unstage
│       ├── llm/                    # AI Orchestration (absorbed christina-llm)
│       │   ├── mod.rs
│       │   ├── orchestrator.rs     # Manages the generation loop
│       │   ├── prompt.rs           # Prompt templates and construction
│       │   ├── tokenizer.rs        # Token counting logic
│       │   └── backend.rs          # Provider implementations (OpenAI, etc.)
│       └── model/                  # Pure Data Types (Shared Vocabulary)
│           ├── mod.rs
│           ├── state.rs            # AppState (Idle, Review, etc.)
│           ├── commit.rs           # CommitMessage struct
│           └── files.rs            # FilePath, GitFile structs
│
└── christina/                      # [CRATE] The TUI/CLI Binary
    ├── Cargo.toml
    └── src/
        ├── main.rs                 # Entry: Setup logs, init Session, run Loop
        ├── loop.rs                 # Main Event Loop (Crossterm events)
        ├── cli/                    # CLI Command Handling
        │   ├── mod.rs
        │   ├── args.rs             # Clap definitions
        │   └── handlers.rs         # Subcommand dispatch (e.g. config editor)
        └── tui/                    # The View Layer
            ├── mod.rs
            ├── context.rs          # UI-specific Context (ToastManager, etc.)
            ├── state.rs            # UI-specific State (ScrollPos, Selection)
            ├── theme.rs            # Ratatui Style definitions (maps Core theme)
            ├── renderer.rs         # Main render(f, app) function
            ├── layout.rs           # Screen layout constraints
            ├── components/         # Reusable low-level widgets
            │   ├── mod.rs
            │   ├── status_bar.rs
            │   ├── help_popup.rs
            │   └── input_box.rs
            └── screens/            # Full-screen views
                ├── mod.rs
                ├── dashboard.rs    # Main file list view
                ├── diff.rs         # Diff viewer
                ├── editor.rs       # Commit message editor
                └── generating.rs   # AI loading screen
```

## 3. Detailed Migration Strategy

### Phase 1: Consolidation (`christina-core`)

We will dismantle `christina-git` and `christina-llm` and move them into `christina-core`.

1.  **Move & Rename:**
    *   `christina-git/src/repository.rs` -> `christina-core/src/git/repository.rs`
    *   `christina-git/src/diff_processor.rs` -> `christina-core/src/git/diff.rs`
    *   `christina-llm/src/orchestrator.rs` -> `christina-core/src/llm/orchestrator.rs`
    *   `christina-llm/src/providers/*` -> `christina-core/src/llm/backend.rs` (Consolidate multiple files if small, or keep submodules if large).

2.  **Move Config:**
    *   `christina/src/config/*` -> `christina-core/src/config/*`.
    *   *Rationale:* The core engine needs to know the API keys and diff tool settings to function.

3.  **The `Session` Struct (The New Context):**
    *   Instead of `AppContextData`, we define `Session`.
    *   It is the single source of truth.
    ```rust
    // christina-core/src/session.rs
    pub struct Session {
        pub repo: Option<GitRepository>,
        pub config: Config,
        pub orchestrator: AIOrchestrator,
        pub state: AppState, // The workflow state (e.g., are we reviewing?)
        pub data: SessionData, // Staged files, unstaged files
    }

    impl Session {
        // Core Actions
        pub fn refresh_state(&mut self) -> Result<()> { ... }
        pub fn stage_file(&mut self, path: &Path) -> Result<()> { ... }
        pub async fn generate_message(&mut self) -> Result<String> { ... }
    }
    ```

### Phase 2: The UI Layer (`christina`)

1.  **Refactor `App`:**
    *   `App` becomes a wrapper that couples the `Core` (Session) with the `View` (UiState).
    ```rust
    // christina/src/app.rs
    pub struct App {
        pub core: christina_core::Session,
        pub ui: UiState,
    }
    ```

2.  **`UiState` Separation:**
    *   `UiState` contains *only* transient interface state:
        *   `selected_index`: Which item is highlighted.
        *   `scroll_offset`: Where the view is scrolled.
        *   `active_tab`: Dashboard vs. Changes.
        *   `input_buffer`: What the user is typing (if not handled by `tui-textarea`).
        *   `toasts`: Temporary messages.

## 4. Shared vs. UI-Specific Logic

| Concept | Location | Type |
| :--- | :--- | :--- |
| **Commit Message** | `core::model::commit` | Struct with subject/body fields. |
| **Message Validation** | `core::model::commit` | Logic to check length/format. |
| **Message Editing** | `tui::screens::editor` | Textarea widget handling. |
| **Git Diff (Raw)** | `core::git::diff` | `Vec<Hunk>` or similar data structure. |
| **Git Diff (Rendered)** | `tui::screens::diff` | Syntax highlighting/coloring for TUI. |
| **API Keys** | `core::config` | Loaded from disk/env. |
| **Keybindings** | `christina::loop` | Match `KeyEvent` -> `Session` method call. |

## 5. Pragmatic Rust Improvements

1.  **Unified Error Type:**
    *   Use `thiserror` in `christina-core/src/error.rs`.
    *   Expose `pub type Result<T> = std::result::Result<T, Error>;` in `prelude.rs`.
    *   Remove `anyhow` from library code; use it only in `main.rs`.

2.  **Public Fields for Data Structs:**
    *   Don't over-encapsulate data objects (DTOs).
    *   If a struct is just data (like `GitFile`), make fields `pub` instead of writing `get_path()`.

3.  **Flat Hierarchy:**
    *   Avoid folder structures like `src/tui/config/app/runner.rs`.
    *   Flatten to `src/tui/runner.rs` or `src/tui/state.rs`.

4.  **Async/Sync Split:**
    *   Keep `christina-core`'s Git operations synchronous (git2 is sync).
    *   Keep LLM operations `async`.
    *   The `christina` event loop handles the bridge (spawn generic tasks for LLM).