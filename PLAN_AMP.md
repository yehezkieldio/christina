# Static Audit & Completion Readiness Report

**Generated:** 2026-02-01
**Scope:** Full workspace (`christina`, `christina-core`)
**Reference:** `old_backup/` for behavioral baseline
**Author:** Amp CLI

---

## Executive Summary

### Overall Completion Status: **~85% Complete**

The codebase is architecturally sound and substantially functional. The core domain model in `christina-core` is well-designed with proper type safety, and the TUI application in `christina` follows a clean Elm-like architecture. However, several subsystems are incomplete or exhibit behavioral gaps compared to the `old_backup/` reference implementation.

### Primary Blocking Defects

1. **Crate consolidation incomplete** — `christina-git` and `christina-llm` from `old_backup/` were inlined into `christina/src/io/`, but the `GitRepository` abstraction with its semantic operations was lost.

2. **Duplicate state definitions** — Screen state structs (`GeneratingState`, `ReviewState`, etc.) exist in both `christina-core/src/app/screens/` and `christina/src/tui/screens/`, with different fields and semantics.

3. **Elm architecture model inconsistency** — `christina-core` defines a pure `Model` with `Msg`/`Cmd` for Elm architecture, but this is not used by the actual TUI runtime in `christina`.

4. **Missing integration tests** — No tests exist in the workspace (only unit tests). `old_backup/christina-git/tests/integration_test.rs` and `old_backup/christina-llm/tests/integration_test.rs` have no equivalents.

5. **Secret resolution unimplemented** — `SecretRef` (env var, keyring) defined in `christina-core/src/config/secret.rs` is never resolved to `SecretString` at runtime.

### Systemic Failure Modes

- **API key handling** — The system can start without a valid API key and will fail at generation time rather than at startup validation.
- **Repository handle threading** — `git2::Repository` is not `Send`, requiring re-opening in background tasks. This is handled but not abstracted, leading to repeated boilerplate.

---

## Detailed Findings

### 1. Crate Architecture & Module Organization

#### 1.1 Lost `GitRepository` Abstraction

**Location:** `christina/src/io/git/adapter.rs`
**Reference:** `old_backup/christina-git/src/repository.rs`

**What is missing:**
The `old_backup/` version provided a `GitRepository` struct that encapsulated all git operations with semantic methods:
- `get_staged_diff()` → Returns `StagedDiff` with iterator over deltas
- `stage_files()` → Atomic staging with validation
- `create_commit()` → With GPG signing support
- `get_commit_history()` → Filtered commit history for LLM context

The current implementation exports free functions (`get_staged_files`, `unstage_files`, `create_commit`) that take `&Repository` as a parameter. This is less cohesive and loses the encapsulation.

**Why it is a problem:**
- No single entry point for git operations
- GPG signing logic from `old_backup/` is absent
- The `CommitInfo` struct and `get_commit_history()` are duplicated inline in `generate.rs` rather than in the git module

**Behavioral delta:**
- GPG signed commits are not supported in the current codebase
- Commit history retrieval is duplicated (once in `adapter.rs` logic, once in `generate.rs`)

---

#### 1.2 Duplicate Screen State Definitions

**Locations:**
- `christina-core/src/app/screens/*.rs`
- `christina/src/tui/screens/*.rs`

**Specific duplications:**

| State | `christina-core` | `christina/tui` | Mismatch |
|-------|------------------|-----------------|----------|
| `GeneratingState` | `progress_message`, `spinner_frame` | `spinner_idx`, `stage` | Different fields, different semantics |
| `ReviewState` | `action`, `show_diff` | `generated_message`, `review_action`, `selected_action`, `show_diff_preview` | Substantially different |
| `EditingState` | `content`, `cursor_line`, `cursor_column` | `message`, `cursor`, `history`, `synced_version` | Substantially different |
| `ErrorState` | `message`, `can_retry` | `error_message`, `can_retry`, `candidate_message`, `selected_action` | Extended in TUI |
| `DashboardState` | `generated_message`, `edit_history`, `cursor_position` | Full TUI state with `list_state`, `staged_files`, etc. | Completely different |
| `StagingState` | `selected_indices`, `multi_select_mode`, `search_query` | Full TUI state with `list_state`, `unstaged_files`, etc. | Completely different |

**Why it is a problem:**
- Confusion about which type to use where
- `christina-core` screen states appear unused
- The Elm `Model` and `update()` in `christina-core` reference screen states that don't match what the TUI uses
- Dead code in `christina-core/src/app/screens/`

**Impact:**
The `christina-core/src/app/update.rs` `update()` function operates on a `Model` that is never constructed by the actual runtime. The TUI uses `TuiSessionData` and `TuiUiState` instead.

---

#### 1.3 Elm Architecture Model Not Used

**Location:** `christina-core/src/app/`
**Reference usage:** `christina/src/tui/elm.rs`, `christina/src/event_loop/`

**What is defined but not exercised:**
- `christina-core/src/app/model.rs` defines `Model` with `Route`, `Screens`, `GitState`, `GenerationStatus`
- `christina-core/src/app/msg.rs` defines `Msg` variants for state transitions
- `christina-core/src/app/cmd.rs` defines `Cmd` for side effect requests
- `christina-core/src/app/update.rs` defines `update(model, msg) -> Vec<Cmd>`

**What the TUI actually uses:**
- `christina/src/tui/elm.rs` defines a separate `AppMsg` enum
- `christina/src/app/mod.rs` defines `App` with `TuiUiState`, `TuiSessionData`, `AppContextData`
- State transitions happen via `App::transition_to()` and `App::handle_app_msg()`
- The `update()` function from `christina-core` is never called

**Why it is a problem:**
- The Elm architecture in `christina-core` is structurally complete but behaviorally inert
- Two parallel message/update systems exist
- Tests in `christina-core/src/app/update.rs` pass but don't reflect actual runtime behavior

**Architectural decision required:**
Either:
1. Wire the core `update()` into the TUI runtime, or
2. Remove the unused Elm model from `christina-core`

---

### 2. Configuration & Secret Handling

#### 2.1 Secret Resolution Not Implemented

**Location:** `christina-core/src/config/secret.rs`
**Usage:** `christina/src/config/settings.rs`

**What is defined:**
```rust
pub enum SecretRef {
    EnvVar(String),    // e.g., "OPENAI_API_KEY"
    Keyring(String),   // e.g., "christina.openai"
    Literal(String),   // Raw value (for testing)
}
```

**What is missing:**
No function exists to resolve `SecretRef` → `SecretString`. The `ProviderProfile<S>` is generic over the secret type, but:
- `ConfigFile` uses `ProviderProfile<SecretRef>`
- `ResolvedConfig` uses `ProviderProfile<SecretString>`
- No resolution function converts between them

**Current workaround:**
`Config` in `christina/src/config/settings.rs` stores `api_key: Option<String>` directly, bypassing the `SecretRef` abstraction entirely. The profile system stores `Secret::Value(String)` which is always a literal.

**Why it is a problem:**
- Users cannot use `env:OPENAI_API_KEY` or `keyring:christina.openai` as documented pattern
- The `SecretRef::EnvVar` and `SecretRef::Keyring` variants are dead code
- Environment variable handling is hardcoded in `Config::load()` rather than using the abstraction

---

#### 2.2 Config File Not Loaded from Local Directory

**Location:** `christina/src/config/settings.rs`, lines 127-148

**What is implied:**
The comment says local config `./christina.toml` is Layer 2, but the code only loads:
- Layer 3: Global config (`~/.config/christina/config.toml`)
- Layer 1: Environment variables

The local config file is mentioned in comments but never loaded:
```rust
// Layer 2: Local config file (./christina.toml) - ONLY safe fields (security)
// Build config without local file first to get trusted values
```

**Why it is a problem:**
- Per-project configuration is not supported despite being documented in the struct comment
- Users cannot have project-specific profiles

---

### 3. LLM Integration

#### 3.1 Provider Trait Removed

**Location:** `christina/src/io/llm/provider.rs`
**Reference:** `old_backup/christina-llm/src/provider.rs`

**What changed:**
`old_backup/` had:
```rust
#[async_trait]
pub trait Provider: Send + Sync {
    async fn generate(&self, messages: &[ChatMessage]) -> Result<String, CompletionError>;
}
```

Current implementation uses an enum `Provider` with variants for each provider type and match-based dispatch.

**Behavioral equivalence:** Functionally equivalent, but:
- Cannot add providers at runtime
- Testing requires `Provider::Mock` variant rather than trait object

This is acceptable for the use case but represents an architectural deviation.

---

#### 3.2 Retry Policy Not Connected to Transient Error Detection

**Location:** `christina/src/io/llm/orchestrator.rs`
**Reference:** `christina-core/src/error.rs` (`IsTransient` trait)

**What is defined:**
- `IsTransient` trait in `christina-core/src/error.rs`
- `CompletionError::is_transient()` method
- `RetryPolicy` in `christina/src/io/llm/retry.rs`

**What is missing:**
The `IsTransient` trait is implemented but the `retry_policy` field on `AIOrchestrator` is constructed but its usage in the actual retry loop is not visible in the code. The orchestrator does have retry logic (with timeouts), but it's not clear that `is_transient()` is checked before retrying.

**Potential issue:**
Non-transient errors (e.g., `Unauthorized`) may be retried unnecessarily.

---

### 4. Git Operations

#### 4.1 Staged Diff String Construction Duplicated

**Locations:**
- `christina/src/event_loop/mod.rs` lines 142-203 (inline in `try_start_generation`)
- `christina/src/io/git/adapter.rs` (staged files with diff content)

**What is duplicated:**
The event loop constructs the staged diff string by:
1. Re-opening repository in spawn_blocking
2. Building diff via `diff_tree_to_tree`
3. Printing to string

Meanwhile, `get_staged_files()` in adapter.rs already captures `diff_content` per file.

**Why it is a problem:**
- The staged files already have diff content, but it's not used
- Diff is computed twice in the generation flow
- Logic is not centralized

---

#### 4.2 GPG Signing Not Implemented

**Reference:** `old_backup/christina-git/src/repository.rs`

**What existed:**
```rust
GitError::GpgConfigInvalid(String),
GitError::GpgSigningFailed(String),
```

These error variants exist in `christina-core/src/error.rs` but:
- `create_commit()` in `christina/src/io/git/adapter.rs` does not handle GPG signing
- No GPG configuration detection exists
- The `is_signing_error()` method is defined but never called

**Impact:**
Users with `commit.gpgsign=true` will get unsigned commits or confusing errors.

---

### 5. TUI Implementation

#### 5.1 Edit History Snapshot Logic

**Location:** `christina/src/tui/screens/editing.rs`
**Reference:** `christina/src/app/edit_history.rs`

The `EditHistory` exists and is initialized, but:
- `should_save_snapshot()` is called in `components_elm.rs`
- Undo/redo operations are defined in `EditingState`
- Integration appears complete

**Status:** Appears complete.

---

#### 5.2 Toast Manager Timeout Handling

**Location:** `christina/src/tui/widgets/toast.rs`

**What is implemented:**
- `ToastManager::update()` called on every tick
- Toasts have `created_at` timestamp
- Visibility filtering by duration

**Status:** Appears complete.

---

### 6. Type System & Domain Model

#### 6.1 CommitMessage Validation Strictness

**Location:** `christina-core/src/types/commit_message.rs`

**Current behavior:**
- Regex requires `type(scope): description` or `type: description`
- Scope must be lowercase alphanumeric with hyphens/underscores
- No period at end is NOT enforced (only mentioned in prompts)

**Potential issue:**
The LLM prompt says "no period at the end" but `CommitMessage::validate()` does not check this.

---

#### 6.2 TokenCount Saturation Semantics

**Location:** `christina-core/src/types/token_count.rs`

**What is implemented:**
- `TokenCount::new_saturating()` clamps to valid range
- Arithmetic operations saturate rather than panic

**Status:** Well-designed, no issues found.

---

### 7. Test Coverage

#### 7.1 Missing Integration Tests

**Reference:**
- `old_backup/christina-git/tests/integration_test.rs` — 40+ tests
- `old_backup/christina-llm/tests/integration_test.rs` — 15+ tests

**Current state:**
No integration tests exist. All tests are unit tests within modules.

**Impact:**
- End-to-end flows are untested
- Git operations against real repositories are untested
- LLM API mocking covers happy path but not edge cases

---

#### 7.2 Orchestrator Tests Use Mock Provider

**Location:** `christina/src/io/llm/orchestrator.rs` tests

All orchestrator tests use `Provider::mock()` which returns static responses. This is appropriate for unit tests but means:
- Real HTTP error handling is untested
- Timeout behavior is untested
- Rate limiting response handling is untested

---

## Behavioral Gaps vs `old_backup/`

### 3.1 Lost Features

| Feature | `old_backup/` | Current | Status |
|---------|--------------|---------|--------|
| GPG signed commits | `create_commit_with_gpg()` | Not implemented | **Missing** |
| Keyring secret storage | `SecretRef::Keyring` | Defined but not resolved | **Incomplete** |
| Environment variable secrets | `SecretRef::EnvVar` | Hardcoded in Config::load | **Degraded** |
| Commit history for context | `get_commit_history()` | Inline in generate.rs | **Duplicated** |
| Repository wrapper | `GitRepository` struct | Free functions | **Architecture change** |
| Integration tests | Comprehensive | None | **Missing** |

### 3.2 Behavioral Equivalence

| Flow | `old_backup/` | Current | Match |
|------|--------------|---------|-------|
| Single-chunk direct generation | ✓ | ✓ | ✓ |
| Multi-chunk map-reduce | ✓ | ✓ | ✓ |
| Hierarchical theme extraction | ✓ | ✓ | ✓ |
| Partial failure handling | ✓ | ✓ | ✓ |
| User context injection | ✓ | ✓ | ✓ |
| Commit history context | ✓ | ✓ | ✓ |
| Retry with backoff | ✓ | ✓ | ✓ |
| Rate limiting | ✓ | ✓ | ✓ |

---

## Completion Blockers

### Must Fix Before Production

1. **Remove or wire up `christina-core` Elm model** — Dead code creates maintenance burden and confusion.

2. **Consolidate screen state definitions** — Choose one source of truth for each screen's state.

3. **Implement secret resolution** — Or remove `SecretRef::EnvVar`/`Keyring` variants and document that only literals are supported.

4. **Add GPG signing support** — Users with `commit.gpgsign=true` will have broken workflows.

5. **Centralize diff string construction** — Remove duplication between event loop and adapter.

### Should Fix

6. **Add integration tests** — Port tests from `old_backup/` to verify end-to-end behavior.

7. **Implement local config loading** — Or remove the documentation claiming it's supported.

8. **Validate API key at startup** — Fail fast rather than at generation time.

---

## Architectural Risk Assessment

### High Risk Areas

1. **Event loop → generation flow** — Complex async state management with `AbortOnDrop`, `spawn_blocking`, and repository re-opening. Single point of failure for the core use case.

2. **Dual Elm architecture** — Confusion between `christina-core` model and TUI runtime model increases probability of bugs during maintenance.

3. **Secret handling** — Incomplete abstraction means future changes to support keyring will require touching multiple files.

### Medium Risk Areas

4. **Provider dispatch** — Enum-based dispatch is fine but adding new providers requires modifying the enum.

5. **Token budget calculation** — Uses hard limits but doesn't account for model-specific variations.

### Low Risk Areas

6. **Type system** — Well-designed newtypes prevent invalid states.

7. **Error handling** — Comprehensive error types with proper categorization.

8. **TUI rendering** — Clean separation between state and view.

---

## Appendix: File-by-File Audit Notes

### `christina-core`

| File | Status | Notes |
|------|--------|-------|
| `lib.rs` | ✓ | Clean re-exports |
| `error.rs` | ✓ | Well-designed, GPG variants unused |
| `state.rs` | ✓ | StateMachine complete with tests |
| `profile.rs` | ✓ | Validation complete |
| `tokenizer.rs` | ✓ | Trait well-designed |
| `prompt.rs` | ✓ | Complete prompt templates |
| `ids.rs` | ✓ | Simple newtype |
| `app/model.rs` | ⚠️ | Unused by runtime |
| `app/msg.rs` | ⚠️ | Unused by runtime |
| `app/cmd.rs` | ⚠️ | Unused by runtime |
| `app/update.rs` | ⚠️ | Tests pass but function unused |
| `app/screens/*.rs` | ⚠️ | Duplicated in TUI, unused |
| `config/secret.rs` | ⚠️ | Resolution not implemented |
| `config/config_file.rs` | ✓ | Serde definitions complete |
| `config/resolved.rs` | ⚠️ | Never constructed from ConfigFile |
| `llm/*.rs` | ✓ | Request/response types complete |
| `git/*.rs` | ✓ | Types complete |
| `types/*.rs` | ✓ | Well-designed newtypes |

### `christina`

| File | Status | Notes |
|------|--------|-------|
| `main.rs` | ✓ | Clean entry point |
| `cli.rs` | ✓ | Clap definitions complete |
| `generate.rs` | ⚠️ | Inline `CommitInfo`, duplicates adapter logic |
| `config/settings.rs` | ⚠️ | Secret resolution bypassed |
| `config/cli.rs` | ✓ | CLI commands complete |
| `app/mod.rs` | ✓ | App struct well-organized |
| `app/handlers.rs` | ✓ | AppMsg dispatch complete |
| `app/init.rs` | ✓ | Initialization complete |
| `event_loop/mod.rs` | ⚠️ | Diff construction duplicated |
| `io/git/adapter.rs` | ⚠️ | No GPG, no abstraction |
| `io/llm/provider.rs` | ✓ | Enum dispatch works |
| `io/llm/orchestrator.rs` | ✓ | Core generation logic complete |
| `tui/screens/*.rs` | ✓ | TUI screens complete |
| `tui/elm.rs` | ✓ | AppMsg + Component trait |

---

## Conclusion

The codebase is substantially complete for its core use case (AI-powered commit message generation). The primary issues are:

1. **Architectural debt** from incomplete refactoring (`christina-core` Elm model unused)
2. **Feature gaps** compared to `old_backup/` (GPG, secret resolution, integration tests)
3. **Duplication** between core and TUI (screen states, diff construction)

A focused effort to consolidate the Elm architecture, implement secret resolution, and add integration tests would bring this codebase to production readiness.
