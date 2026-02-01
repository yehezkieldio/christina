# Christina Architectural Redesign - Learnings

## Task: Remove git2 Dependency from christina-core

### Approach Taken
Successfully removed git2 as a required dependency from christina-core while maintaining the ability to convert git2 errors to typed domain error variants.

### Solution Architecture

1. **Typed Error Variants**: Replaced `GitError::Git2(#[from] git2::Error)` with semantic variants:
   - `NotFound` - for resources not found
   - `AuthFailed` - for authentication errors  
   - `Locked` - for locked repositories
   - `Other(String)` - for other git errors with details

2. **Feature Flag**: Made git2 an optional dependency in christina-core via feature flag:
   - Default: no git2 dependency
   - With `git2-support` feature: provides `From<git2::Error>` implementation
   - christina-git enables this feature to automatically convert git2 errors

3. **Error Conversion Strategy**:
   - Using feature-gated `#[cfg(feature = "git2-support")]` blocks avoids orphan rule violations
   - Git2 dependency stays in christina-git where it's actually used
   - christina-core remains Sans-IO when used without the feature

### Key Changes Made

- `christina-core/Cargo.toml`: git2 moved to optional dependency with feature gate
- `christina-core/src/error.rs`: Replaced Git2 variant with typed variants, added conditional From impl
- `christina/src/event_loop/mod.rs`: Updated error matching to use new typed variants
- `christina-git/Cargo.toml`: Enabled git2-support feature for christina-core

### Error Code Mapping

The From implementation maps git2 error codes to typed variants:
- `NotFound` → git2::ErrorCode::NotFound
- `AuthFailed` → git2::ErrorCode::Auth
- `Locked` → git2::ErrorCode::Locked
- `Other(msg)` → all other codes

### What Worked Well

1. Feature flag approach allows selective dependency inclusion
2. Typed error variants are more semantic than generic git2 errors
3. Error handling in event_loop is now explicit and readable
4. christina-core can be used independently without git2

### What Would Be Different Next Time

- Start with feature flags from the beginning to avoid orphan rule issues
- The newtype wrapper pattern could also work but feature flags are cleaner
- Consider if error conversion needs to be in core or could be at boundary

## Phase 2.1 - Config Foundation Types

**Date**: 2026-02-01

### Created New Config Types

1. **Secret types** (`christina-core/src/config/secret.rs`):
   - `Secret<S>`: Generic secret container with single `Value(S)` variant
   - `SecretRef`: On-disk secret reference (EnvVar, Keyring, Literal)
   - `SecretString`: Runtime secret with redacted Debug output
   - All types implement necessary traits (Clone, PartialEq, Eq, Serialize, Deserialize)
   - Security: SecretString::eq always returns false for safety

2. **ConfigFile** (`christina-core/src/config/config_file.rs`):
   - On-disk representation using `ProviderProfile<SecretRef>`
   - Includes all config fields: active_profile, profiles, commit_message_max_length, ignore_files, include_file_diffs
   - Default implementation provides sensible defaults (lock files ignored by default)

3. **ResolvedConfig** (`christina-core/src/config/resolved.rs`):
   - Runtime representation using `ProviderProfile<SecretString>`
   - Helper methods: `get_active_profile()`, `get_profile(name)`
   - commit_message_max_length is usize (not Option) with default 72

### Made ProviderProfile Generic

- Updated `ProviderProfile<S = String>` with generic secret type
- Added `temperature: Option<f32>` field (was missing)
- Maintained backward compatibility with `ProviderProfile::new()` for `String` type
- Updated `Profiles<S>` to be generic as well
- All impl blocks now generic: `impl<S> ProviderProfile<S>` and `impl<S> Profiles<S>`

### Fixes Applied

- Added `Serialize` and `Deserialize` derives to `Secret<S>`
- Fixed pre-existing clippy warning in azure_endpoint.rs (`map_or` → `is_some_and`)
- All tests pass (146 tests in christina-core)

### Architecture Notes

- Two-phase model: ConfigFile (on-disk) → ResolvedConfig (runtime)
- Type-safe secret handling prevents accidental exposure
- Generic parameter enables different secret types at different stages
- Breaking change: Consumers will need to specify secret type when using ProviderProfile


## Phase 2.2: Model and Screen States Creation (2026-02-01)

Successfully created the canonical Model structure in christina-core to replace TuiSessionData.

### Implementation

**Created Files:**
- `christina-core/src/app/mod.rs` - Module structure with exports
- `christina-core/src/ids.rs` - GenerationId newtype
- `christina-core/src/app/model.rs` - Core Model, Route, Screens, GitState, GenerationStatus, Toast types
- `christina-core/src/app/screens/` - Six screen state files (staging, dashboard, review, editing, generating, error)

**Key Design Decisions:**

1. **Persistent Screens**: Screens are NOT Optional - they persist across navigation
   - Eliminates `Option<ScreenState>` pattern from TuiSessionData
   - Allows screens to retain ephemeral UI state (scroll, cursor) when navigating away/back
   - Cleaner navigation logic without unwrap/expect

2. **Pure GenerationStatus**: No tokio types in core
   - Uses `GenerationId` for tracking instead of `JoinHandle<()>`
   - Variants: Idle, Running, Completed, Failed
   - Keeps christina-core runtime-agnostic

3. **Canonical GitState**: Single source of truth
   - Replaces duplication between TuiSessionData.base and DataState
   - Fields: files, staged, unstaged, branch, repo_root
   - No mixing with UI concerns

4. **Screen State Structure**: Simple data-only structs
   - No methods yet (deferred to later phases)
   - Appropriate derives: Debug, Clone, Default where applicable
   - Core types imported from existing christina-core modules

**Toast System**: Added to Model
- `Toast` struct with message, severity, created_at
- `ToastSeverity` enum: Info, Warning, Error
- Replaces TUI-specific ToastManager in core

**Type Refinements:**

- `ReviewState`: Manual Default impl (ReviewAction::Accept)
- `GenerationStatus`: Used `#[default]` attribute on Idle variant
- `DashboardState`: Derived Default instead of manual impl

### Testing

- `cargo check -p christina-core` ✓
- `cargo test -p christina-core` ✓ (146 tests passed)
- `cargo clippy -p christina-core -- -D warnings` ✓

### Next Steps

Phase 2.3 will adapt TuiSessionData to use Model, maintaining compatibility while transitioning to the new structure.

