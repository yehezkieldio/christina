 Christina Completeness Report

**Date:** 2026-02-04  
**Scope:** `christina` (TUI app) and `christina-core` (domain library)  
**Mode:** Brutal, no-mercy static analysis

---

## Executive Summary

Christina is an AI-powered conventional commit message generator with a TUI and CLI interface. The codebase demonstrates competent Rust design with good separation between core domain logic and application concerns. However, **production readiness is blocked** by several critical gaps spanning validation, error handling, configuration safety, and testing coverage.

**Verdict:** Not production-ready. Estimated 10-15 hours of focused work to close critical gaps.

---

## 1. Specification Compliance Gaps

### 1.1 Conventional Commits Specification

| Gap | Severity | Location |
|-----|----------|----------|
| **No validation of scope characters** | HIGH | `commit_message.rs` |
| Conventional Commits spec allows only alphanumeric + hyphen in scope. The `CommitMessage` type does not enforce this. Invalid scopes like `auth/user` would pass. | | |
| **No enforcement of imperative mood** | LOW | N/A |
| The spec recommends imperative mood but Christina relies on LLM to produce it. No post-generation validation. Acceptable for MVP but a gap. | | |
| **Breaking change indicator missing** | MEDIUM | Prompt/parsing |
| The spec defines `!` after type for breaking changes (e.g., `feat!:`). No support in prompt templates or message parsing. | | |

### 1.2 Claimed Features vs. Implementation

| Feature (README) | Status | Gap |
|------------------|--------|-----|
| "AI-Powered Commit Generation" | ✅ Implemented | — |
| "Conventional Commits" | ⚠️ Partial | No validation of spec compliance |
| "TUI Interface" | ✅ Implemented | — |
| "Profile Management" | ✅ Implemented | API key parsing incomplete (see §2.3) |

---

## 2. Implicit Production Requirements (Non-Spec Gaps)

### 2.1 Input Validation Gaps

| Component | Gap | Impact |
|-----------|-----|--------|
| **`FilePath::new()`** | Only uses `debug_assert!` for relative path check | In release builds, absolute paths silently pass. Production data corruption risk. |
| **`ProviderProfile` validation** | Does not validate Azure-specific fields when provider is Azure (partially fixed in `profile_editable.rs` but not in core type) | Provider creation fails at runtime with cryptic error |
| **API key empty check** | Happens late in `generate.rs:91-93` | Better to fail-fast at config load |
| **Temperature bounds** | Validated in `LlmRequest::validate()` and `ProviderSpec::validate()` but not called consistently at construction | Invalid temperature could reach provider |

**Required Fix:**
```rust
// christina-core/src/types/file_path.rs
impl FilePath {
    pub fn new(path: impl Into<CompactString>) -> Self {
        let compact = path.into();
        // MUST validate in release builds
        assert!(
            !compact.starts_with('/'),
            "FilePath must be relative, got: {}",
            compact
        );
        Self(compact)
    }
}
```
Or prefer `try_new()` everywhere and deprecate `new()`.

### 2.2 Error Handling Gaps

| Location | Issue |
|----------|-------|
| `CompletionError::from_api_error()` | Previously had `contains("5")` bug (per existing plan). Static analysis confirms the current implementation in `error.rs:165-181` does NOT have this bug—it uses specific phrases like `"server error"`, `"overloaded"`. **This is already fixed.** |
| `Provider::from_profile()` | Returns `Result` but many call sites use `?` without user-friendly context wrapping |
| `Config::load()` | Creates default profile and auto-saves; failure path returns error but may leave partial state |
| `SecretRef::resolve()` | Keyring errors include internal `keyring::Error` in message—may leak implementation details |

### 2.3 Secret/API Key Handling Gaps

| Gap | Severity | Detail |
|-----|----------|--------|
| **Plaintext API keys in config** | HIGH | `SecretRef::Literal` allows raw API keys in TOML. No warning emitted. |
| **Env var fallback ambiguity** | MEDIUM | If user types `OPENAI_API_KEY` (without `env:` prefix), it's stored as literal. User expects env resolution. |
| **Keyring error messages** | LOW | May expose keyring backend details to user |

**Required Fix:** When parsing API key input without `env:`/`keyring:` prefix, check if it looks like an env var name (all caps, underscores) and warn user.

### 2.4 Concurrency and Cancellation

| Component | Gap |
|-----------|-----|
| `AIOrchestrator::generate_commit_message()` | Uses `futures::stream::StreamExt` for parallel map phase. No explicit cancellation token passed to provider calls. If user cancels generation, in-flight requests continue to completion (wasting API quota). |
| `AbortOnDrop` wrapper | Exists in `app/state.rs` but only aborts the top-level task, not nested futures. |

**Recommendation:** Pass a `CancellationToken` (from `tokio-util`) through the orchestrator pipeline.

---

## 3. Architectural Drift or Violations

### 3.1 Crate Boundary Violations

| Violation | Detail |
|-----------|--------|
| **`christina-core` has optional `git2` dependency** | But `git2` types don't leak into public API. ✅ Clean. |
| **`christina` imports `git2` directly** | Correct—application crate owns I/O. ✅ Clean. |
| **`Tokenizer` trait in core, impl in `christina`** | Correct separation. ✅ Clean. |

### 3.2 Module Cohesion Issues

| Module | Issue |
|--------|-------|
| `christina/src/config/settings.rs` | 1300+ lines. Mixes `Config` struct, env loading, profile management, validation, persistence, and all getters/setters. **Split into:** `config/mod.rs` (facade), `config/loader.rs`, `config/persistence.rs`, `config/profile_ops.rs`. |
| `christina/src/io/llm/orchestrator.rs` | 1800+ lines. Acceptable for complex orchestration but tests are inline and bloat the file. Extract tests to `orchestrator_tests.rs`. |
| `christina/src/tui/` | 31 files. Some screens have `mod.rs` + separate files; others don't. Inconsistent. |

### 3.3 Type Leakage

| Issue | Location |
|-------|----------|
| `git2::Repository` stored directly in `AppContextData` | `christina/src/app/context.rs:5`. This couples the app to `git2`. Better: store `Box<dyn GitRepository>` using the existing trait. |
| `url::Url` in `ProviderProfile` public API | Acceptable—`url` is a stable, widely-used crate. |

---

## 4. Naive, Fragile, or Unsound Implementations

### 4.1 Cursor Movement in Form Editor

**Location:** `christina/src/tui/form/state.rs:132-139`

```rust
pub fn move_cursor_right(&mut self) {
    if self.mode == FormMode::Editing && self.edit_cursor < self.edit_buffer.len() {
        self.edit_cursor = self.edit_buffer[self.edit_cursor..]
            .char_indices()
            .next()  // Gets (0, first_char)
            .map(|(i, c)| self.edit_cursor + i + c.len_utf8())
            .unwrap_or(self.edit_buffer.len());
    }
}
```

**Analysis:** The `char_indices().next()` returns `(0, char)` for the first character at offset 0 from the slice start. Adding `i` (which is 0) then `c.len_utf8()` gives correct result. **This is actually correct.** The existing plan's claim of a bug using `.nth(1)` appears to reference old code that has since been fixed.

### 4.2 Token Budget Calculation

**Location:** `christina/src/io/llm/orchestrator.rs` + `generate.rs`

The `TokenBudget` struct reserves space for prompts and messages, then calculates remaining budget for diff content. This is sound, but:

- No validation that `reserved_for_prompt + reserved_for_messages < max_input`
- If misconfigured, `remaining_for_diff()` could return 0 or negative (saturating to 1)

**Required:** Add invariant check in `TokenBudget::new()`.

### 4.3 Lockfile Pattern Matching

**Location:** `christina/src/io/git/chunking.rs:155-159`

```rust
fn should_limit_file(path: &FilePath, ignore_patterns: &[String]) -> bool {
    ignore_patterns
        .iter()
        .any(|pattern| path.as_str().ends_with(pattern))
}
```

**Issue:** This is naive substring matching, not glob matching. Pattern `*.lock` will match file `*.lock` literally, not `Cargo.lock`. The default `ignore_files` in `Config::default()` includes `"Cargo.lock"` explicitly, masking this bug.

**Required:** Use proper glob matching or document that patterns are suffixes only.

### 4.4 JSON Extraction from LLM Response

**Location:** `christina/src/io/llm/orchestrator.rs` (method `extract_json`)

The orchestrator attempts to extract JSON from potentially malformed LLM responses. This is inherently fragile but necessary. The current implementation uses brace matching with escape handling—reasonable for the use case.

---

## 5. Missing Production Guarantees

### 5.1 Determinism and Reproducibility

| Aspect | Status |
|--------|--------|
| **Same diff → same commit message?** | ❌ No. LLM temperature > 0 by default. |
| **Audit trail for generated messages** | ❌ No logging of prompt/response pairs. |
| **Version stamp in config** | ❌ No config schema version. Future upgrades may break. |

**Recommendation:** Add optional `--deterministic` flag that sets temperature=0 and seeds RNG.

### 5.2 Idempotency

| Operation | Idempotent? |
|-----------|-------------|
| `christina config set key value` | ✅ Yes |
| `christina profile create name` | ❌ No—errors if exists. Acceptable. |
| Generation with same staged diff | ❌ No—LLM non-deterministic. Acceptable. |

### 5.3 Observability

| Feature | Status |
|---------|--------|
| Structured logging | ❌ `tracing` crate present but no subscriber configured in `main.rs` |
| Metrics/counters | ❌ None |
| Debug dump of LLM prompts | ❌ `debug_enabled()` function exists but no way to enable at runtime |

**Required:** Add `--verbose`/`-v` handling to configure tracing subscriber.

### 5.4 Failure Semantics

| Scenario | Behavior | Gap |
|----------|----------|-----|
| Config file parse error | Fail with error message | ✅ OK |
| API key resolution fails | Error at generation time | Could fail-fast at config load |
| Partial chunk failures | Continue with warning | ✅ Excellent—`max_partial_failure_rate` configurable |
| Keyring unavailable | Runtime error | ✅ Error message includes fix suggestion |

---

## 6. Specification Ambiguities and Mandatory Clarifications

### 6.1 Commit Message Length

The system uses 72 characters as default max length (git convention). However:
- `ValidationMode::Soft` truncates silently
- `ValidationMode::Hard` (if implemented) would error
- No user feedback when truncation occurs in TUI (only in `GenerationResult.truncated` flag)

**Clarification needed:** Should truncation be visible to user in TUI? Currently it's in `warning_summary()` but may not be prominently displayed.

### 6.2 Multi-Provider Consistency

Different providers (OpenAI, Azure, Groq) may produce differently-styled messages. No normalization layer exists.

**Clarification needed:** Is this acceptable? Should there be post-processing to enforce consistent style?

### 6.3 Empty Diff Handling

If staged changes exist but produce empty processable content (e.g., only binary files), the system errors with "No processable diff content found."

**Clarification needed:** Should this be an error or a warning with skip?

---

## 7. Performance and Scalability Risks

### 7.1 Identified Risks

| Risk | Severity | Detail |
|------|----------|--------|
| **Large diff processing** | LOW | 10MB limit enforced. Chunking is O(n) with efficient buffer pooling. ✅ Well-designed. |
| **Concurrent request limit** | LOW | Capped at 20. Rate limiter prevents burst. ✅ |
| **HashMap for profiles** | LOW | Typical user has <10 profiles. Not a concern. |
| **Tokenizer initialization** | MEDIUM | `TokenizerService::new()` loads tiktoken model on each generation. Should cache. |
| **Regex compilation** | LOW | `regex` crate auto-caches compiled patterns. ✅ |

### 7.2 Memory Considerations

| Concern | Analysis |
|---------|----------|
| `Arc<str>` for diff chunks | ✅ Efficient sharing, no copy on clone |
| `CompactString` for paths | ✅ Inline storage for short strings |
| Buffer pool in chunking | ✅ Reduces allocations |
| `mimalloc` allocator | ✅ Good choice for many small allocations |

**No critical performance issues identified.**

---

## 8. Developer Experience and Operability

### 8.1 CLI Ergonomics

| Aspect | Status | Gap |
|--------|--------|-----|
| Help text | ✅ clap-derived | — |
| Error messages | ⚠️ Mixed | Some errors expose internal details |
| Subcommand discoverability | ✅ `config`, `profile` well-organized | — |
| Shell completions | ❌ Not generated | Add `clap_complete` |
| `--dry-run` mode | ❌ Missing | Would be useful for CI |

### 8.2 Configuration Design

| Aspect | Status | Gap |
|--------|--------|-----|
| Layered loading | ✅ Env > Local > Global > Default | — |
| Documentation of keys | ⚠️ Partial | No generated docs or `--help-config` |
| Validation on load | ⚠️ Partial | Clamping happens but silent |
| Migration path | ❌ None | No schema versioning |

### 8.3 Error Quality

| Error Type | Quality |
|------------|---------|
| API auth failures | ✅ "Run `christina config setup` to reconfigure" |
| Rate limits | ✅ "Please wait a moment and try again" |
| Network errors | ⚠️ Exposes raw error message |
| Config parse errors | ⚠️ May be cryptic (TOML parse errors) |

### 8.4 Test Surface

| Crate | Test Status | Gap |
|-------|-------------|-----|
| `christina-core` | ✅ Good coverage | Some edge cases in `Secret` resolution |
| `christina` | ⚠️ Partial | Many TUI modules untested (per existing plan) |

---

## 9. Edge Cases and Failure Modes

### 9.1 Invalid Inputs

| Input | Handling | Gap |
|-------|----------|-----|
| Empty staged diff | ✅ Error: "No staged changes to process" | — |
| Binary-only diff | ✅ Filtered out, may result in empty | Could warn user |
| Non-UTF8 file names | ⚠️ `to_string_lossy()` used | May corrupt paths |
| Extremely long file paths | ⚠️ No limit | Could exceed terminal width |

### 9.2 Partial Execution

| Scenario | Behavior | Gap |
|----------|----------|-----|
| Generation cancelled mid-flight | Task aborted, state reset | In-flight API calls not cancelled |
| Config save fails after profile add | Profile in memory but not persisted | Could leave inconsistent state |
| Commit creation fails after generation | Error shown, message preserved | ✅ Can retry |

### 9.3 Crash Recovery

| Scenario | Recovery | Gap |
|----------|----------|-----|
| Crash during TUI | Terminal restored via `TerminalHandle::cleanup` | ✅ |
| Panic in generation | Caught by `catch_unwind`, terminal cleaned | ✅ |
| Crash during config save | Config file may be corrupt | Use atomic write (rename) |

**Required:** Use atomic file writes for config persistence.

### 9.4 Corrupted State

| State | Detection | Recovery |
|-------|-----------|----------|
| Invalid TOML config | Parse error at load | User must fix manually |
| Profile with missing API key | Runtime error at generation | Error message guides fix |
| Keyring entry deleted | Runtime error | Error message guides fix |

---

## 10. Codebase Structure and Aesthetics

### 10.1 File Layout Issues

| Issue | Files Affected | Recommendation |
|-------|----------------|----------------|
| **Oversized files** | `settings.rs` (1300 LOC), `orchestrator.rs` (1800 LOC) | Split by concern |
| **Inconsistent mod structure** | `tui/profiles/` vs `tui/config/` | Standardize: each screen gets `mod.rs`, `state.rs`, `view.rs` |
| **Test helpers split** | `christina-core/src/test_helpers.rs` + inline in test modules | Consolidate or document pattern |

### 10.2 Naming Issues

| Current | Issue | Suggested |
|---------|-------|-----------|
| `TuiSessionData` | Unclear what "session" means | `TuiAppData` or `TuiState` |
| `DataState` | Generic | `SharedDataState` or `CoreDataState` |
| `base` field in structs | Unclear inheritance intent | `shared` or `common` |
| `GeneratingState` vs `GenerationState` | Confusingly similar | `GeneratingUiState` and `GenerationTaskState` |

### 10.3 Module Organization

**Current structure:**
```
christina/src/
├── app/           # App state, handlers
├── bootstrap/     # Terminal setup
├── cli/           # CLI commit flow
├── config/        # Config loading, CLI handlers
├── event_loop/    # TUI event loop
├── generate.rs    # Generation orchestration entry point
├── io/            # Git/LLM I/O
│   ├── git/       # Git operations
│   └── llm/       # LLM providers, orchestration
└── tui/           # TUI screens, widgets
```

**Issues:**
- `generate.rs` at root level; should be in `io/llm/` or `app/`
- `config/cli.rs` and `config/profile_cli.rs` are CLI handlers, not config logic
- `tui/` mixes screens, widgets, forms, and Elm architecture

**Recommended:**
```
christina/src/
├── app/
│   ├── mod.rs
│   ├── state.rs
│   └── handlers.rs
├── cli/
│   ├── mod.rs
│   ├── commit.rs
│   ├── config.rs      # moved from config/cli.rs
│   └── profile.rs     # moved from config/profile_cli.rs
├── config/
│   ├── mod.rs
│   ├── loader.rs
│   ├── persistence.rs
│   └── settings.rs    # just the Config struct
├── git/               # renamed from io/git
├── llm/               # renamed from io/llm, include generate.rs
├── tui/
│   ├── mod.rs
│   ├── screens/
│   ├── widgets/
│   └── forms/
└── main.rs
```

### 10.4 Dead Code

| Item | Location | Action |
|------|----------|--------|
| `#[allow(dead_code)]` on `GitRepository` trait | `adapter.rs:7` | Remove allow, trait is used |
| `#[allow(dead_code)]` on `MockGitRepository` | `adapter.rs:67` | Legitimate for test-only code |

---

## 11. Critical Path to Production

### Phase 1: Blockers (Must Fix)

| Task | Priority | Effort |
|------|----------|--------|
| Add runtime validation to `FilePath::new()` | P0 | 30min |
| Add config schema version | P0 | 1hr |
| Configure tracing subscriber for `--verbose` | P0 | 1hr |
| Atomic config file writes | P0 | 1hr |
| Validate `TokenBudget` invariants | P1 | 30min |

### Phase 2: High Priority

| Task | Priority | Effort |
|------|----------|--------|
| Add shell completions via `clap_complete` | P1 | 1hr |
| Warn on plaintext API keys in config | P1 | 1hr |
| Cache `TokenizerService` instead of recreating | P1 | 30min |
| Add `--dry-run` flag | P1 | 2hr |
| Split `settings.rs` | P2 | 3hr |

### Phase 3: Test Coverage

Per existing plan in `.sisyphus/plans/comprehensive-test-coverage.md`, 17 modules need tests. Estimated 15-20 hours total.

---

## 12. Summary of Required Actions

### Must Have (Blocking Release)

1. **Runtime path validation** in `FilePath::new()`
2. **Config schema version** for future compatibility
3. **Tracing subscriber** for observability
4. **Atomic file writes** for config persistence
5. **TokenBudget invariant checks**

### Should Have (Release Quality)

6. **Shell completions**
7. **Plaintext API key warnings**
8. **Tokenizer caching**
9. **`--dry-run` mode**
10. **Module split for `settings.rs`**

### Nice to Have (Polish)

11. **Breaking change indicator** in commit messages
12. **Scope character validation**
13. **Cancellation tokens** for API requests
14. **Naming consistency cleanup**
15. **Module reorganization**

---

## Appendix: File-by-File Status

| File | LOC | Tests | Status |
|------|-----|-------|--------|
| `christina-core/src/error.rs` | 418 | ✅ | Clean |
| `christina-core/src/profile.rs` | 203 | ✅ | Clean |
| `christina-core/src/state.rs` | 572 | ✅ | Excellent coverage |
| `christina-core/src/config/secret.rs` | 235 | ✅ | Clean |
| `christina-core/src/types/file_path.rs` | 139 | ✅ | **Needs runtime validation** |
| `christina-core/src/types/token_count.rs` | 138 | ✅ | Clean |
| `christina-core/src/llm/request.rs` | 157 | ✅ | Clean |
| `christina-core/src/llm/provider_spec.rs` | 139 | ✅ | Clean |
| `christina/src/generate.rs` | 657 | ✅ | Clean |
| `christina/src/config/settings.rs` | 1315 | ✅ | **Needs split** |
| `christina/src/io/llm/orchestrator.rs` | 1795 | ✅ | Clean, tests inline |
| `christina/src/io/llm/provider.rs` | 244 | ⚠️ | Needs more tests |
| `christina/src/io/git/adapter.rs` | 776 | ✅ | Clean |
| `christina/src/io/git/chunking.rs` | 841 | ✅ | **Pattern matching fragile** |
| `christina/src/tui/form/state.rs` | 170 | ❌ | Needs tests |
| `christina/src/tui/profiles/profile_editable.rs` | 174 | ❌ | Needs tests |
| `christina/src/event_loop/handlers.rs` | 508 | ✅ | Clean |

---

*End of Report*

NOTE: MAKE SURE ALL TESTS PASSED, EVEN IF ITS NOT RELATED TO CURRENT CONTEXT.
