# TUI Integration Outline & Dossier

## Overview

This document provides a comprehensive outline for re-integrating TUI (Terminal User Interface) functionality into Christina. The TUI was removed to simplify the codebase and focus on CLI functionality, but this document preserves the design and implementation details for future reference.

## Removed Components

### Directory Structure

```
christina/src/
├── tui/                      # Main TUI module (REMOVED)
│   ├── mod.rs               # TUI entry points and main coordination
│   ├── components_elm.rs    # Elm-like component architecture
│   ├── context.rs           # Shared TUI context and state
│   ├── diff_executor.rs     # Git diff execution and processing
│   ├── diff_renderer.rs     # Diff rendering with syntax highlighting
│   ├── elm.rs               # Elm architecture implementation
│   ├── layout.rs            # Layout management and calculations
│   ├── theme.rs             # Color themes and styling
│   ├── config/              # Configuration UI components
│   ├── form/                # Form input components
│   ├── profiles/            # Profile management UI
│   ├── screens/             # Main TUI screens (dashboard, etc.)
│   └── widgets/             # Reusable UI widgets
├── app/                      # TUI application state (REMOVED)
│   ├── mod.rs               # App state and initialization
│   ├── state.rs             # Main application state
│   ├── context.rs           # Application context
│   ├── edit_history.rs      # Edit history management
│   ├── handlers.rs          # Event handlers
│   └── init.rs              # Initialization logic
├── event_loop/              # TUI event loop (REMOVED)
│   ├── mod.rs               # Event loop coordination
│   ├── events.rs            # Event type definitions
│   ├── handlers.rs          # Event handling
│   └── producers.rs         # Event producers (keyboard, mouse, etc.)
└── bootstrap/               # Terminal initialization (REMOVED)
    ├── mod.rs
    └── terminal.rs          # Terminal setup and cleanup
```

### Dependencies (Removed from Cargo.toml)

```toml
[dependencies]
ratatui = { version = "0.29.0", default-features = false, features = ["crossterm"] }
tui-textarea = { version = "0.7.0", default-features = false, features = ["ratatui", "crossterm"] }
ansi-to-tui = "7.0.0"
which = "8.0.0"
compact_str = { workspace = true }
```

### CLI Flags and Commands (Removed)

- `--tui`: Global flag to launch TUI mode
- `config tui`: Open configuration TUI
- `profile tui`: Open profile management TUI

## Architecture

### Elm Architecture Pattern

The TUI used an Elm-like architecture for component composition:

1. **Model**: Component state
2. **Update**: State transitions based on events
3. **View**: Rendering logic

Each screen was implemented as an Elm component with:
- `init()`: Initialize component state
- `update()`: Handle events and update state
- `view()`: Render component to terminal

### Event Loop

The event loop used a producer-consumer pattern:

```rust
// Event producers (crossterm keyboard/mouse events)
producers::spawn(tx.clone());

// Event loop
loop {
    let event = rx.recv().await?;
    app.update(event)?;
    terminal.draw(|f| app.view(f))?;
}
```

### Key Components

#### Dashboard Screen

The main TUI screen showing:
- Commit generation status
- Diff preview (with syntax highlighting)
- Token usage statistics
- Recent commit history
- Profile selector

#### Configuration Editor

Multi-tab form-based editor:
- **General Tab**: Core AI settings (provider, model, API key, tokens)
- **Advanced Tab**: Fine-tuning options (temperature, validation, diff tools)

Features:
- Real-time validation
- Field-specific help text
- Secret field masking
- Profile integration

#### Profile Manager

CRUD interface for profiles:
- List all profiles
- Create/edit/delete profiles
- Switch active profile
- Duplicate profiles

#### Form System

Generic form component for structured input:
- Field validation
- Type-specific editors (text, number, float, boolean, secret)
- Help text display
- Navigation between fields

### Terminal Management

```rust
pub struct TerminalHandle {
    terminal: Terminal<CrosstermBackend<io::Stdout>>,
}

impl TerminalHandle {
    pub fn init() -> Result<Self> {
        // Enter alternate screen
        // Enable raw mode
        // Enable mouse capture
        // Hide cursor
    }

    pub fn cleanup(&mut self, exit_msg: Option<String>) -> Result<()> {
        // Restore terminal state
        // Show cursor
        // Disable raw mode
        // Leave alternate screen
        // Print exit message
    }
}
```

## Integration Points

### Config System

The TUI integrated deeply with the config system through the `Editable` trait:

```rust
pub trait Editable {
    fn fields(&self) -> Vec<FieldDef>;
    fn get_field(&self, key: &str) -> Option<String>;
    fn set_field(&mut self, key: &str, value: &str) -> Result<()>;
    fn validate(&self) -> Result<()>;
}
```

This allowed:
- Automatic form generation from config
- Field-level validation
- Dynamic field visibility (e.g., Azure-specific fields)

### Event System

The TUI used a centralized event system for:
- Progress updates during generation
- Token count updates
- UI events (keyboard, mouse)
- Background task notifications

```rust
pub enum Event {
    GenerationProgress { stage: String, generation_id: u64 },
    TokenCountUpdate { token_count: TokenCount, generation_id: u64 },
    // ... UI events
}
```

This event system is now minimal (only used by CLI for progress display).

### Diff Rendering

The TUI had sophisticated diff rendering:

1. **Diff Executor**: Execute git diff and capture output
2. **Diff Processor**: Parse diff and apply syntax highlighting
3. **Diff Renderer**: Render to terminal with ANSI colors

Support for multiple diff tools:
- delta
- difftastic
- diff-so-fancy
- git (native)
- basic (plain)

## Re-integration Strategy

### Phase 1: Foundation

1. **Restore Dependencies**
   ```toml
   ratatui = { version = "0.29.0", features = ["crossterm"] }
   tui-textarea = { version = "0.7.0", features = ["ratatui", "crossterm"] }
   ansi-to-tui = "7.0.0"
   ```

2. **Restore Terminal Bootstrap**
   - Implement `TerminalHandle` for terminal setup/cleanup
   - Add panic handler for terminal restoration
   - Ensure proper cleanup on exit

3. **Restore Event Loop**
   - Restore `Event` enum with full event types
   - Implement event producers (keyboard, mouse, tick)
   - Implement event loop with proper error handling

### Phase 2: Core UI Components

1. **Restore Elm Architecture**
   - Restore `elm.rs` with core traits
   - Restore `components_elm.rs` with base components
   - Implement basic component composition

2. **Restore Layout System**
   - Restore `layout.rs` for responsive layouts
   - Implement constraint-based sizing
   - Add screen-specific layouts

3. **Restore Theme System**
   - Restore `theme.rs` with color schemes
   - Implement light/dark themes
   - Add user-configurable themes

### Phase 3: Application State

1. **Restore App Module**
   - Restore `app/state.rs` with main state
   - Restore `app/context.rs` for shared context
   - Restore `app/edit_history.rs` for undo/redo

2. **Restore Event Handlers**
   - Restore `app/handlers.rs` for event processing
   - Implement keyboard shortcuts
   - Implement mouse interactions

### Phase 4: Screens and Features

1. **Restore Dashboard Screen**
   - Restore `screens/dashboard/` with main UI
   - Implement diff preview panel
   - Implement status display
   - Implement profile selector

2. **Restore Configuration Editor**
   - Restore `config/` TUI components
   - Implement multi-tab form
   - Restore `Editable` trait integration
   - Implement field validation UI

3. **Restore Profile Manager**
   - Restore `profiles/` TUI components
   - Implement profile list view
   - Implement profile editor
   - Implement profile switching

### Phase 5: Advanced Features

1. **Restore Diff Rendering**
   - Restore `diff_executor.rs` and `diff_renderer.rs`
   - Integrate with diff tools
   - Implement syntax highlighting
   - Add scrolling and navigation

2. **Restore Form System**
   - Restore `form/` components
   - Implement generic field editors
   - Implement validation display
   - Implement help text display

3. **Restore Widgets**
   - Restore `widgets/` reusable components
   - Implement list widgets
   - Implement input widgets
   - Implement status widgets

### Phase 6: CLI Integration

1. **Restore CLI Flags**
   - Add `--tui` global flag
   - Add `config tui` command
   - Add `profile tui` command

2. **Restore Entry Points**
   - Restore `run_tui()` function in main.rs
   - Implement TUI mode detection
   - Implement proper error handling

## Technical Considerations

### Ownership and Lifetimes

The TUI heavily used `RefCell` and `Rc` for shared mutable state. When re-implementing:

1. **Minimize Interior Mutability**: Use message passing where possible
2. **Use Channels**: Prefer channels over shared state for component communication
3. **Event-Driven Updates**: Drive all state changes through events

### Error Handling

The TUI needs robust error handling:

1. **Terminal Cleanup**: Always cleanup terminal on error
2. **Panic Handler**: Install panic handler to restore terminal
3. **User Feedback**: Show error messages in-UI, not panics

### Performance

Key performance considerations:

1. **Rendering**: Only redraw on events, not on timer
2. **Diff Processing**: Cache processed diffs
3. **Syntax Highlighting**: Use incremental parsing
4. **Event Throttling**: Throttle high-frequency events (mouse)

### Testing

TUI testing strategy:

1. **Unit Tests**: Test component update logic
2. **Integration Tests**: Test screen composition
3. **Manual Testing**: Use golden tests for rendering

## Migration Path from Git History

To restore TUI functionality:

```bash
# Find the commit before TUI removal
git log --oneline --all | grep -i "remove.*tui"

# Create a new branch from before removal
git checkout <commit-before-removal>
git checkout -b restore-tui

# Cherry-pick or merge the TUI code
# Then rebase onto current main
git rebase main
```

## Dependencies and Constraints

### External Dependencies

- **ratatui**: Core TUI framework, active maintenance
- **crossterm**: Cross-platform terminal manipulation
- **tui-textarea**: Multiline text editor widget

### Compatibility

- Works on: Linux, macOS, Windows
- Requires: Terminal with ANSI color support
- Optional: True color support for better diffs

## Open Questions

1. **TUI vs CLI**: Should TUI be default or opt-in?
2. **Configuration**: Should TUI have separate config from CLI?
3. **Profiles**: How to handle profile switching in TUI vs CLI?
4. **Diff Tools**: Should TUI support all diff tools or subset?

## References

- [Ratatui Documentation](https://ratatui.rs/)
- [Elm Architecture](https://guide.elm-lang.org/architecture/)
- [Crossterm Documentation](https://docs.rs/crossterm/)

## Notes

- The TUI was fully functional before removal
- All tests were passing
- Git history preserves complete implementation
- Removal was for simplification, not due to bugs
- Re-integration is expected and supported
