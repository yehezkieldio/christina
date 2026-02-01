# Christina Production Completion Plan

## TL;DR

> **Quick Summary**: Fix 7 critical bugs blocking production readiness, ranging from infinite recursion crashes to security vulnerabilities in API key handling.
>
> **Deliverables**:
> - Fixed infinite recursion in profile validation
> - Fixed cursor movement bug in form editing
> - Runtime validation for FilePath (not debug-only)
> - Proper API key parsing (env:/keyring: syntax support)
> - ProviderSpec validation enforcement
> - LlmRequest input validation
> - Proper HTTP status code error detection
>
> **Estimated Effort**: Medium (~4-6 hours)
> **Parallel Execution**: NO - sequential due to dependencies
> **Critical Path**: Task 1 → Task 2 → Task 3 → Task 4 → Task 5 → Task 6 → Task 7

---

## Context

### Original Request
Create a comprehensive production completion plan for the Christina Rust codebase (AI-powered git commit message generator) that prioritizes fixes by severity, groups related work, and specifies exact files and line numbers.

### Codebase Structure
- Workspace with 2 crates: `christina` (TUI app) and `christina-core` (core types)
- Event loop architecture with Elm-style message passing
- State machine for screen transitions (Staging → Dashboard → Generating → Review → Editing)
- LLM orchestration with map/reduce pipeline for large diffs
- Git integration via git2 crate
- Configuration with layered loading (defaults → global → local → env)

### Quality Gates (from justfile)
- `just check` - cargo check
- `just clippy` - clippy with -D warnings
- `just test` - cargo nextest run

---

## Work Objectives

### Core Objective
Fix all critical and high-priority bugs to achieve production-ready status, ensuring the application is stable, secure, and correctly validates all inputs.

### Concrete Deliverables
1. Fixed `profile_editable.rs` infinite recursion (line 135-137)
2. Fixed `form/state.rs` cursor movement bug (lines 132-139)
3. Runtime FilePath validation with Result-returning constructor
4. API key parsing that supports env:VAR and keyring:KEY syntax
5. ProviderSpec validation ensuring ProviderKind matches ProviderEndpoint
6. LlmRequest validation for temperature, max_tokens, and messages
7. Fixed error heuristics using proper HTTP status code detection

### Definition of Done
- [ ] All quality gates pass (`just check`, `just clippy`, `just test`)
- [ ] No infinite recursion in any validation path
- [ ] All user inputs are validated at runtime
- [ ] API keys are never stored as plaintext in config files
- [ ] Error classification correctly identifies 5xx vs 4xx errors
- [ ] All new code has accompanying unit tests

### Must Have
- All critical bugs fixed (infinite recursion, cursor bug)
- Runtime validation for all user-facing types
- Secure API key handling
- Proper error classification

### Must NOT Have (Guardrails)
- NO breaking changes to public API without deprecation path
- NO removal of existing functionality
- NO changes to serialization format that break existing configs
- NO introduction of new dependencies without justification
- NO suppression of clippy warnings with `#[allow(...)]` attributes

---

## Verification Strategy

### Test Decision
- **Infrastructure exists**: YES (cargo nextest)
- **User wants tests**: YES (TDD-style for new validation code)
- **Framework**: Built-in `cargo test` with `nextest` runner

### Test Setup
Tests already exist in the codebase. Each fix must include:
1. Unit tests for new validation logic
2. Integration tests for fixed behavior
3. Regression tests for the specific bugs

### Automated Verification
Each TODO includes executable verification via:
- `cargo test` for unit tests
- `cargo check` and `cargo clippy` for compilation
- Manual TUI testing for interactive components

---

## Execution Strategy

### Sequential Execution Required
Due to dependencies between components (e.g., FilePath validation affects multiple modules), tasks must be executed sequentially.

```
Wave 1 (Critical - Must Complete First):
├── Task 1: Fix infinite recursion in profile_editable.rs
└── Task 2: Fix cursor movement bug in form/state.rs

Wave 2 (High Priority):
├── Task 3: Fix FilePath runtime validation
├── Task 4: Fix API key persistence (SecretRef parsing)
└── Task 5: Add ProviderSpec validation

Wave 3 (Medium Priority):
├── Task 6: Add LlmRequest validation
└── Task 7: Fix error heuristics in error.rs

Wave 4 (Final Verification):
└── Task 8: Run full test suite and quality gates
```

### Dependency Matrix

| Task | Depends On | Blocks | Can Parallelize With |
|------|------------|--------|---------------------|
| 1 | None | 2 | None |
| 2 | 1 | 3 | None |
| 3 | 2 | 4, 5 | None |
| 4 | 3 | 6 | 5 |
| 5 | 3 | 6 | 4 |
| 6 | 4, 5 | 7 | None |
| 7 | 6 | 8 | None |
| 8 | 7 | None | None |

---

## TODOs

### Phase 1: Critical Bug Fixes (Must Complete First)

---

- [ ] 1. Fix infinite recursion in profile_editable.rs

  **What to do**:
  - Fix the `validate()` method at lines 135-137 in `christina/src/tui/profiles/profile_editable.rs`
  - Current code: `fn validate(&self) -> Result<()> { self.validate() }` - calls itself infinitely
  - Replace with proper validation logic that checks all required fields

  **How to change it**:
  ```rust
  // Current (BROKEN):
  fn validate(&self) -> Result<()> {
      self.validate()  // Infinite recursion!
  }

  // Fixed:
  fn validate(&self) -> Result<()> {
      // Validate required fields
      if self.name.trim().is_empty() {
          return Err(anyhow!("Profile name cannot be empty"));
      }
      if self.model.as_str().trim().is_empty() {
          return Err(anyhow!("Model cannot be empty"));
      }
      // Azure-specific validation
      if self.provider == ProviderKind::Azure {
          if self.azure_deployment_id.as_ref().map(|s| s.trim().is_empty()).unwrap_or(true) {
              return Err(anyhow!("Azure deployment ID is required for Azure provider"));
          }
      }
      Ok(())
  }
  ```

  **Must NOT do**:
  - Do NOT just remove the validate() method - it must perform actual validation
  - Do NOT add async or complex logic - keep it simple and synchronous
  - Do NOT change the function signature (must return `Result<()>`)

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: None required - straightforward fix

  **Parallelization**:
  - **Can Run In Parallel**: NO
  - **Parallel Group**: Sequential
  - **Blocks**: Task 2
  - **Blocked By**: None (can start immediately)

  **References**:
  - `christina/src/tui/profiles/profile_editable.rs:135-137` - The broken validate() method
  - `christina/src/tui/form/editable.rs` - Editable trait definition
  - `christina-core/src/profile.rs` - ProviderProfile struct definition

  **Acceptance Criteria**:
  - [ ] `cargo test` passes for profile_editable module
  - [ ] Running `christina profile create` no longer causes stack overflow
  - [ ] Validation correctly rejects empty profile names
  - [ ] Validation correctly requires Azure deployment_id when provider is Azure

  **Commit**: YES
  - Message: `fix(profile): fix infinite recursion in validate()`
  - Files: `christina/src/tui/profiles/profile_editable.rs`
  - Pre-commit: `cargo test profile_editable`

---

- [ ] 2. Fix cursor movement bug in form/state.rs

  **What to do**:
  - Fix the `move_cursor_right()` method at lines 132-139 in `christina/src/tui/form/state.rs`
  - Current code uses `.char_indices().nth(1)` which skips the immediate next character
  - Should use `.next()` to get the first character after current position

  **How to change it**:
  ```rust
  // Current (BROKEN):
  pub fn move_cursor_right(&mut self) {
      if self.mode == FormMode::Editing && self.edit_cursor < self.edit_buffer.len() {
          self.edit_cursor = self.edit_buffer[self.edit_cursor..]
              .char_indices()
              .nth(1)  // BUG: Gets the SECOND char, not the first!
              .map(|(i, _)| self.edit_cursor + i)
              .unwrap_or(self.edit_buffer.len());
      }
  }

  // Fixed:
  pub fn move_cursor_right(&mut self) {
      if self.mode == FormMode::Editing && self.edit_cursor < self.edit_buffer.len() {
          self.edit_cursor = self.edit_buffer[self.edit_cursor..]
              .chars()
              .next()  // Get the first char after current position
              .map(|c| self.edit_cursor + c.len_utf8())
              .unwrap_or(self.edit_buffer.len());
      }
  }
  ```

  **Must NOT do**:
  - Do NOT change the public API signature
  - Do NOT modify other cursor movement methods unless they have the same bug
  - Do NOT use byte-based indexing without accounting for UTF-8 multi-byte chars

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: None required

  **Parallelization**:
  - **Can Run In Parallel**: NO
  - **Parallel Group**: Sequential
  - **Blocks**: Task 3
  - **Blocked By**: Task 1

  **References**:
  - `christina/src/tui/form/state.rs:132-139` - The broken move_cursor_right() method
  - `christina/src/tui/form/state.rs:122-129` - move_cursor_left() for comparison (uses .last() correctly)

  **Acceptance Criteria**:
  - [ ] `cargo test form::state` passes
  - [ ] Add unit test: moving right in "abc" from position 0 goes to position 1 (not 2)
  - [ ] Add unit test: moving right through multi-byte UTF-8 chars works correctly
  - [ ] Manual TUI test: cursor moves one character at a time in form fields

  **Commit**: YES
  - Message: `fix(form): fix cursor movement skipping characters`
  - Files: `christina/src/tui/form/state.rs`
  - Pre-commit: `cargo test form::state`

---

### Phase 2: High Priority Hardening

---

- [ ] 3. Fix FilePath runtime validation

  **What to do**:
  - Replace `debug_assert!` with runtime validation in `christina-core/src/types/file_path.rs`
  - Current code at lines 13-17 only validates in debug builds
  - Add `try_new()` constructor that returns `Result<Self, FilePathError>`
  - Keep `new()` for backward compatibility but make it panic on invalid input (document this)

  **How to change it**:
  ```rust
  // Add error type:
  #[derive(Debug, thiserror::Error)]
  #[error("FilePath must be relative, got absolute path: {0}")]
  pub struct FilePathError(String);

  // Replace constructor:
  impl FilePath {
      /// Create a new FilePath, validating it's relative.
      /// Panics in debug builds if path is absolute (for development feedback).
      /// Returns error in all builds for invalid paths.
      pub fn try_new(path: impl Into<CompactString>) -> Result<Self, FilePathError> {
          let compact = path.into();
          if compact.starts_with('/') {
              return Err(FilePathError(compact.to_string()));
          }
          Ok(Self(compact))
      }

      /// Create a new FilePath.
      /// # Panics
      /// Panics in debug builds if path is absolute.
      pub fn new(path: impl Into<CompactString>) -> Self {
          let compact = path.into();
          debug_assert!(
              !compact.starts_with('/'),
              "FilePath must be relative, got: {}",
              compact
          );
          Self(compact)
      }
  }
  ```

  **Must NOT do**:
  - Do NOT change the existing `new()` signature (maintain backward compatibility)
  - Do NOT silently accept absolute paths - must error or panic
  - Do NOT use `std::path::Path::is_absolute()` - it has platform-specific behavior

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: None required

  **Parallelization**:
  - **Can Run In Parallel**: NO
  - **Parallel Group**: Sequential
  - **Blocks**: Task 4, Task 5
  - **Blocked By**: Task 2

  **References**:
  - `christina-core/src/types/file_path.rs:10-19` - Current constructor with debug_assert!
  - `christina-core/src/types/file_path.rs:32-42` - From<String> and From<&str> impls that use new()

  **Acceptance Criteria**:
  - [ ] `cargo test file_path` passes
  - [ ] `FilePath::try_new("/absolute")` returns Err
  - [ ] `FilePath::try_new("relative/path")` returns Ok
  - [ ] Existing tests still pass
  - [ ] Add test for Windows-style absolute paths (e.g., "C:\\path")

  **Commit**: YES
  - Message: `fix(types): add runtime validation to FilePath`
  - Files: `christina-core/src/types/file_path.rs`
  - Pre-commit: `cargo test file_path`

---

- [ ] 4. Fix API key persistence (SecretRef parsing)

  **What to do**:
  - Fix API key handling in `profile_cli.rs` (lines 180, 243) and `settings.rs` (line 374)
  - Current code stores raw API keys as `Secret::Value(key)` which serializes to TOML as plaintext
  - Should parse "env:VAR_NAME" and "keyring:KEY_NAME" syntax and store as appropriate Secret variant

  **How to change it**:
  ```rust
  // In profile_cli.rs, replace:
  if let Some(key) = api_key {
      profile.api_key = Secret::Value(key);  // WRONG: stores plaintext
  }

  // With:
  if let Some(key) = api_key {
      profile.api_key = parse_secret_input(&key);
  }

  // Helper function (add to secret.rs or inline):
  fn parse_secret_input(input: &str) -> Secret<String> {
      if let Some(var_name) = input.strip_prefix("env:") {
          Secret::EnvVar(var_name.to_string())
      } else if let Some(key_name) = input.strip_prefix("keyring:") {
          Secret::Keyring(key_name.to_string())
      } else {
          // Plain value - warn in production
          #[cfg(not(debug_assertions))]
          eprintln!("WARNING: Storing API key directly in config. Consider using env: or keyring: syntax.");
          Secret::Value(input.to_string())
      }
  }
  ```

  **Files to modify**:
  1. `christina/src/config/profile_cli.rs:180` - handle_create api_key assignment
  2. `christina/src/config/profile_cli.rs:243` - handle_edit api_key assignment
  3. `christina/src/config/settings.rs:374` - Config::set api_key assignment

  **Must NOT do**:
  - Do NOT break existing configs that have plaintext keys (support migration)
  - Do NOT require env:/keyring: prefix (keep plaintext as fallback with warning)
  - Do NOT change Secret enum definition - it already supports the variants

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: None required

  **Parallelization**:
  - **Can Run In Parallel**: NO
  - **Parallel Group**: Sequential
  - **Blocks**: Task 6
  - **Blocked By**: Task 3

  **References**:
  - `christina-core/src/config/secret.rs` - Secret enum with Value, EnvVar, Keyring variants
  - `christina-core/src/config/secret.rs:108-119` - SecretRef::parse() method for reference
  - `christina/src/config/profile_cli.rs:98-102` - handle_show displays env:/keyring: syntax

  **Acceptance Criteria**:
  - [ ] `cargo test secret` passes
  - [ ] Creating profile with `--api-key env:OPENAI_KEY` stores as Secret::EnvVar
  - [ ] Creating profile with `--api-key keyring:christina.openai` stores as Secret::Keyring
  - [ ] Creating profile with `--api-key sk-...` stores as Secret::Value with warning
  - [ ] TOML serialization shows correct format for each variant

  **Commit**: YES
  - Message: `feat(config): parse env: and keyring: syntax in API keys`
  - Files: `christina/src/config/profile_cli.rs`, `christina/src/config/settings.rs`
  - Pre-commit: `cargo test secret`

---

- [ ] 5. Add ProviderSpec validation

  **What to do**:
  - Add validation to `christina-core/src/llm/provider_spec.rs` to ensure:
    1. ProviderKind matches ProviderEndpoint variant (e.g., OpenAI kind with OpenAi endpoint)
    2. URL scheme is valid (https:// required, not http://)
    3. Azure provider has required fields (api_version, deployment_id)

  **How to change it**:
  ```rust
  // Add validation method to ProviderSpec:
  impl ProviderSpec {
      pub fn validate(&self) -> Result<(), ProviderValidationError> {
          // Check kind matches endpoint
          match (&self.kind, &self.endpoint) {
              (ProviderKind::OpenAI, ProviderEndpoint::OpenAi { .. }) => (),
              (ProviderKind::Azure, ProviderEndpoint::AzureOpenAi { .. }) => (),
              (ProviderKind::Groq, ProviderEndpoint::Groq { .. }) => (),
              (kind, endpoint) => {
                  return Err(ProviderValidationError::MismatchedEndpoint {
                      kind: kind.clone(),
                      endpoint: format!("{:?}", endpoint),
                  });
              }
          }

          // Check URL scheme
          let url = match &self.endpoint {
              ProviderEndpoint::OpenAi { base_url } => base_url,
              ProviderEndpoint::Groq { base_url } => base_url,
              ProviderEndpoint::AzureOpenAi { endpoint, .. } => &endpoint.url,
          };
          
          if url.scheme() != "https" {
              return Err(ProviderValidationError::InsecureUrl(url.to_string()));
          }

          // Azure-specific validation
          if let ProviderEndpoint::AzureOpenAi { api_version, deployment_id, .. } = &self.endpoint {
              if api_version.is_empty() {
                  return Err(ProviderValidationError::MissingField("api_version".to_string()));
              }
              if deployment_id.is_empty() {
                  return Err(ProviderValidationError::MissingField("deployment_id".to_string()));
              }
          }

          Ok(())
      }
  }

  #[derive(Debug, thiserror::Error)]
  pub enum ProviderValidationError {
      #[error("Provider kind {kind:?} does not match endpoint {endpoint}")]
      MismatchedEndpoint { kind: ProviderKind, endpoint: String },
      #[error("URL must use https scheme: {0}")]
      InsecureUrl(String),
      #[error("Missing required field: {0}")]
      MissingField(String),
  }
  ```

  **Must NOT do**:
  - Do NOT change ProviderSpec or ProviderEndpoint struct definitions
  - Do NOT add async validation
  - Do NOT validate API key presence (that happens at request time)

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: None required

  **Parallelization**:
  - **Can Run In Parallel**: NO (can run parallel with Task 4 after Task 3)
  - **Parallel Group**: Wave 2
  - **Blocks**: Task 6
  - **Blocked By**: Task 3

  **References**:
  - `christina-core/src/llm/provider_spec.rs:1-34` - ProviderSpec and ProviderEndpoint definitions
  - `christina-core/src/types/mod.rs` - ProviderKind definition
  - `christina-core/src/config/mod.rs` - AzureEndpoint definition

  **Acceptance Criteria**:
  - [ ] `cargo test provider_spec` passes
  - [ ] Validation rejects OpenAI kind with Azure endpoint
  - [ ] Validation rejects http:// URLs
  - [ ] Validation rejects Azure endpoint without deployment_id
  - [ ] Validation accepts properly configured providers

  **Commit**: YES
  - Message: `feat(llm): add ProviderSpec validation`
  - Files: `christina-core/src/llm/provider_spec.rs`
  - Pre-commit: `cargo test provider_spec`

---

### Phase 3: Medium Priority Improvements

---

- [ ] 6. Add LlmRequest input validation

  **What to do**:
  - Add validation to `christina-core/src/llm/request.rs` for:
    1. Temperature range (0.0 to 2.0)
    2. max_tokens against provider limits
    3. Non-empty messages vector

  **How to change it**:
  ```rust
  // Add to LlmRequest impl:
  impl LlmRequest {
      pub fn validate(&self) -> Result<(), LlmRequestError> {
          // Validate temperature
          if self.temperature < 0.0 || self.temperature > 2.0 {
              return Err(LlmRequestError::InvalidTemperature(self.temperature));
          }

          // Validate max_tokens is reasonable (> 0)
          if self.max_tokens.get() == 0 {
              return Err(LlmRequestError::InvalidMaxTokens(self.max_tokens.get()));
          }

          // Validate messages not empty
          if self.messages.is_empty() {
              return Err(LlmRequestError::EmptyMessages);
          }

          // Validate no empty message content
          for (i, msg) in self.messages.iter().enumerate() {
              if msg.content.trim().is_empty() {
                  return Err(LlmRequestError::EmptyMessageContent(i));
              }
          }

          Ok(())
      }
  }

  #[derive(Debug, thiserror::Error)]
  pub enum LlmRequestError {
      #[error("Temperature must be between 0.0 and 2.0, got {0}")]
      InvalidTemperature(f32),
      #[error("max_tokens must be greater than 0, got {0}")]
      InvalidMaxTokens(u32),
      #[error("Messages cannot be empty")]
      EmptyMessages,
      #[error("Message at index {0} has empty content")]
      EmptyMessageContent(usize),
  }
  ```

  **Must NOT do**:
  - Do NOT add provider-specific token limits (that belongs in ProviderSpec)
  - Do NOT validate message role sequences (that may be intentional)
  - Do NOT change LlmRequest struct definition

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: None required

  **Parallelization**:
  - **Can Run In Parallel**: NO
  - **Parallel Group**: Sequential
  - **Blocks**: Task 7
  - **Blocked By**: Task 4, Task 5

  **References**:
  - `christina-core/src/llm/request.rs:45-58` - LlmRequest struct
  - `christina-core/src/types/token_count.rs` - TokenCount type

  **Acceptance Criteria**:
  - [ ] `cargo test llm::request` passes
  - [ ] Validation rejects temperature < 0.0
  - [ ] Validation rejects temperature > 2.0
  - [ ] Validation rejects max_tokens = 0
  - [ ] Validation rejects empty messages vector
  - [ ] Validation rejects messages with empty content

  **Commit**: YES
  - Message: `feat(llm): add LlmRequest input validation`
  - Files: `christina-core/src/llm/request.rs`
  - Pre-commit: `cargo test llm::request`

---

- [ ] 7. Fix error heuristics in error.rs

  **What to do**:
  - Fix the overly broad error detection in `christina-core/src/error.rs` lines 148-151
  - Current code: `msg_lower.contains("5")` matches any message with digit 5
  - Should use proper HTTP status code detection

  **How to change it**:
  ```rust
  // Current (BROKEN):
  } else if msg_lower.contains("5")
      || msg_lower.contains("server error")
      || msg_lower.contains("overloaded")
  {
      CompletionError::ServerError(msg.to_string())

  // Fixed:
  } else if msg_lower.contains("server error")
      || msg_lower.contains("overloaded")
      || is_server_error_code(&msg_lower)
  {
      CompletionError::ServerError(msg.to_string())

  // Helper function:
  fn is_server_error_code(msg: &str) -> bool {
      // Look for 5xx status codes (500-599)
      // Match patterns like "500", "503", "error 500", "status 502", etc.
      msg.split_whitespace()
          .filter_map(|word| {
              let cleaned = word.trim_matches(|c: char| !c.is_ascii_digit());
              cleaned.parse::<u16>().ok()
          })
          .any(|code| (500..600).contains(&code))
  }
  ```

  **Alternative approach** (if HTTP status codes are available):
  If the code has access to actual HTTP status codes, use those directly instead of parsing strings.

  **Must NOT do**:
  - Do NOT use regex (adds dependency for simple check)
  - Do NOT change error variant definitions
  - Do NOT remove existing checks for "server error" or "overloaded"

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: None required

  **Parallelization**:
  - **Can Run In Parallel**: NO
  - **Parallel Group**: Sequential
  - **Blocks**: Task 8
  - **Blocked By**: Task 6

  **References**:
  - `christina-core/src/error.rs:132-162` - from_api_error() method
  - `christina-core/src/error.rs:148-151` - The problematic contains("5") check

  **Acceptance Criteria**:
  - [ ] `cargo test error` passes
  - [ ] "Error 500" is classified as ServerError
  - [ ] "Error 503" is classified as ServerError
  - [ ] "Error 400" is NOT classified as ServerError (should be client error)
  - [ ] "Error 599" is classified as ServerError
  - [ ] "5 files changed" is NOT classified as ServerError

  **Commit**: YES
  - Message: `fix(error): fix overly broad server error detection`
  - Files: `christina-core/src/error.rs`
  - Pre-commit: `cargo test error`

---

### Phase 4: Final Verification

---

- [ ] 8. Run full test suite and quality gates

  **What to do**:
  - Run all quality gates to ensure production readiness
  - Fix any remaining warnings or test failures

  **Commands to run**:
  ```bash
  just check      # cargo check
  just clippy     # cargo clippy with -D warnings
  just test       # cargo nextest run
  ```

  **Must NOT do**:
  - Do NOT suppress warnings with #[allow(...)] attributes
  - Do NOT skip tests
  - Do NOT commit with failing tests

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: None required

  **Parallelization**:
  - **Can Run In Parallel**: NO
  - **Parallel Group**: Final
  - **Blocks**: None
  - **Blocked By**: Task 7

  **Acceptance Criteria**:
  - [ ] `just check` passes with zero warnings
  - [ ] `just clippy` passes with zero warnings
  - [ ] `just test` passes with all tests green
  - [ ] No compiler warnings in any crate

  **Commit**: NO (verification only)

---

## Commit Strategy

| After Task | Message | Files | Verification |
|------------|---------|-------|--------------|
| 1 | `fix(profile): fix infinite recursion in validate()` | profile_editable.rs | `cargo test profile_editable` |
| 2 | `fix(form): fix cursor movement skipping characters` | form/state.rs | `cargo test form::state` |
| 3 | `fix(types): add runtime validation to FilePath` | file_path.rs | `cargo test file_path` |
| 4 | `feat(config): parse env: and keyring: syntax in API keys` | profile_cli.rs, settings.rs | `cargo test secret` |
| 5 | `feat(llm): add ProviderSpec validation` | provider_spec.rs | `cargo test provider_spec` |
| 6 | `feat(llm): add LlmRequest input validation` | request.rs | `cargo test llm::request` |
| 7 | `fix(error): fix overly broad server error detection` | error.rs | `cargo test error` |

---

## Success Criteria

### Verification Commands
```bash
# All quality gates must pass
just check    # Expected: clean compilation
just clippy   # Expected: zero warnings
just test     # Expected: all tests pass

# Specific module tests
cargo test profile_editable
cargo test form::state
cargo test file_path
cargo test secret
cargo test provider_spec
cargo test llm::request
cargo test error
```

### Final Checklist
- [ ] All "Must Have" items completed
- [ ] All "Must NOT Have" guardrails respected
- [ ] All quality gates pass
- [ ] No infinite recursion in any code path
- [ ] All user inputs validated at runtime
- [ ] API keys properly parsed and stored
- [ ] Error classification works correctly

---

## Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Breaking existing configs | Medium | High | Maintain backward compatibility in Secret parsing |
| Test failures in unrelated code | Low | Medium | Run full test suite after each change |
| Clippy warnings from new code | Low | Low | Address all warnings immediately |
| Performance regression | Low | Low | Validation is cheap, no async added |

---

## Notes for Implementer

1. **Order matters**: Tasks must be completed in sequence due to dependencies
2. **Test-driven**: Add tests before or alongside fixes
3. **Small commits**: Each task gets its own commit for easy rollback
4. **Quality gates**: Never skip `just check` and `just clippy`
5. **Documentation**: Update inline docs if behavior changes

## Architectural Decisions

### Decision 1: Keep `FilePath::new()` for backward compatibility
**Rationale**: Many call sites use `FilePath::new()`. Changing to `try_new()` would require widespread changes. Instead, add `try_new()` as the validated alternative and keep `new()` with documentation about panicking in debug builds.

### Decision 2: Support plaintext API keys with warning
**Rationale**: Existing users may have plaintext keys in their configs. Breaking these configs would be poor UX. Instead, support plaintext but warn users to migrate to env:/keyring: syntax.

### Decision 3: Don't use regex for error detection
**Rationale**: Regex adds a dependency and compile-time overhead. Simple string parsing is sufficient for detecting 5xx status codes in error messages.

### Decision 4: Validate at construction time, not serialization
**Rationale**: Catching invalid data early prevents bugs from propagating. Validation in constructors/types is more robust than validation during serialization.
