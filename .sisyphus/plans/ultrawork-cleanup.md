# Ultrawork Execution Plan: Eliminate Stubs/TODOs/Dead Code

## TL;DR

Complete the christina TUI application by removing all dead code allowances, replacing panic branches with proper error handling, and ensuring the end-to-end commit generation flow works correctly. The codebase is largely complete but has brittle error handling and unused code that must be cleaned up to pass `just check/clippy` with zero warnings.

**Estimated Effort**: Medium (1-2 focused sessions)  
**Parallel Execution**: YES - 3 waves  
**Critical Path**: TUI panic fixes → Dead code removal → unwrap cleanup → clippy compliance

---

## Context

### Current State
The christina application is a TUI-based conventional commit message generator with:
- Complete git integration (staging, diff processing, commit creation)
- Full LLM orchestration pipeline (OpenAI, Azure, Groq providers)
- Working TUI with Elm architecture pattern
- Comprehensive test coverage

### Issues Identified

#### 1. TUI Screen Panic Branches (Critical)
Multiple TUI screens panic on unexpected messages instead of handling them gracefully:
- `staging.rs:325` - panics on unexpected StageFiles message variant
- `staging.rs:338` - panics on unexpected Navigate message
- `error.rs:245,258,271,298` - panics on unexpected navigation messages
- `dashboard.rs:700` - panics on unexpected UnstageFile message

#### 2. Dead Code Allowances (High)
Non-test code marked with `#[allow(dead_code)]`:
- `christina/src/io/llm/orchestrator.rs:155` - `AIOrchestrator::new()` constructor
- `christina/src/io/llm/tokenizer.rs:16,24,193,204,214,225` - TokenBudget presets, get_tokenizer()
- `christina/src/io/git/adapter.rs:13,76,85,96` - status(), open(), get_branch_name(), convert_status()
- `christina/src/tui/elm.rs:5,8` - ToastLevel::Success, ToastLevel::Error variants
- `christina/src/app/edit_history.rs:105` - EditHistory::load() method

#### 3. Unwrap/Expect in Production Code (High)
Non-test unwrap/expect calls that could panic:
- `christina/src/tui/diff_executor.rs:211` - `tool.render_diff(input, 80).unwrap()`
- `christina/src/io/llm/tokenizer.rs:35,40,63` - OnceLock unwraps in get_tokenizer()
- `christina/src/io/llm/orchestrator.rs:1760,1764,1767,1812` - Test unwraps (acceptable)

#### 4. Unused Elm Architecture in christina-core (Medium)
The christina-core crate contains a complete Elm-style Model/Msg/update layer that is unused by the TUI runtime. This is architectural debt causing confusion.

---

## Work Objectives

### Core Objective
Eliminate all stubs, TODOs, dead code allowances, and panic branches to create a robust, production-ready TUI application that passes `just check` and `just clippy` with zero warnings.

### Concrete Deliverables
1. All TUI screen panic branches replaced with graceful error handling
2. All `#[allow(dead_code)]` attributes removed (either implement usage or delete code)
3. All production-code unwrap/expect calls eliminated or justified with comments
4. All `just check` and `just clippy` warnings resolved
5. End-to-end commit generation flow verified working

### Definition of Done
- [ ] `just check` passes with zero warnings
- [ ] `just clippy` passes with zero warnings
- [ ] No `#[allow(dead_code)]` in non-test code
- [ ] No panic branches in TUI message handling
- [ ] Staging → Generation → Review → Commit flow works end-to-end

### Must Have
- TUI screens handle all message variants without panicking
- Dead code either used or removed
- Production code uses proper error handling (Result, Option, or expect with justification)

### Must NOT Have (Guardrails)
- No new `#[allow(dead_code)]` attributes
- No `unwrap()` or `expect()` without explicit justification comments
- No panic branches for "impossible" conditions (handle gracefully instead)
- No changes to test code (tests should continue to work)

---

## Execution Strategy

### Wave 1: TUI Panic Fixes (Critical Path)
**Parallel Tasks**: 4 tasks can run simultaneously

#### Task 1.1: Fix staging.rs panic branches
**File**: `christina/src/tui/screens/staging.rs`
**Lines**: 325, 338

**Current Code**:
```rust
other => panic!("Expected StageFiles message, got {:?}", other),
other => panic!("Expected Navigate to Dashboard, got {:?}", other),
```

**Fix**: Replace panic with graceful handling - log warning and return empty vec

**Acceptance Criteria**:
- [ ] Line 325: Replace panic with `tracing::warn!()` + `vec![]`
- [ ] Line 338: Replace panic with `tracing::warn!()` + `vec![]`
- [ ] `just clippy` shows no warnings for this file

---

#### Task 1.2: Fix error.rs panic branches
**File**: `christina/src/tui/screens/error.rs`
**Lines**: 245, 258, 271, 298

**Current Code**: Multiple `panic!()` calls for unexpected message variants

**Fix**: Replace with graceful navigation fallback

**Acceptance Criteria**:
- [ ] All 4 panic branches replaced with `tracing::warn!()` + sensible fallback
- [ ] Error screen handles all navigation messages gracefully
- [ ] `just clippy` shows no warnings for this file

---

#### Task 1.3: Fix dashboard.rs panic branch
**File**: `christina/src/tui/screens/dashboard.rs`
**Line**: 700

**Current Code**:
```rust
other => panic!("Expected UnstageFile message, got {:?}", other),
```

**Fix**: Replace with graceful handling

**Acceptance Criteria**:
- [ ] Line 700: Replace panic with `tracing::warn!()` + `vec![]`
- [ ] `just clippy` shows no warnings for this file

---

### Wave 2: Dead Code Cleanup (High Priority)
**Parallel Tasks**: 5 tasks can run simultaneously

#### Task 2.1: Remove dead code from git/adapter.rs
**File**: `christina/src/io/git/adapter.rs`
**Lines**: 13, 76, 85, 96

**Analysis**:
- `status()` (line 13): Not used - remove function entirely
- `open()` (line 76): Not used - remove function entirely
- `get_branch_name()` (line 85): Used internally - remove `#[allow(dead_code)]`
- `convert_status()` (line 96): Used internally - remove `#[allow(dead_code)]`

**Acceptance Criteria**:
- [ ] Remove `status()` function (lines 8-73)
- [ ] Remove `open()` function (lines 76-82)
- [ ] Remove `#[allow(dead_code)]` from `get_branch_name()`
- [ ] Remove `#[allow(dead_code)]` from `convert_status()`
- [ ] `just check` passes

---

#### Task 2.2: Clean up tokenizer.rs dead code
**File**: `christina/src/io/llm/tokenizer.rs`
**Lines**: 16, 24, 193, 204, 214, 225

**Analysis**:
- `TOKENIZER` static (line 16): Only used in tests - wrap in `#[cfg(test)]`
- `get_tokenizer()` (line 24): Only used in tests - wrap in `#[cfg(test)]`
- `TokenBudget::small()` (line 193): Marked dead_code - either use or remove
- `TokenBudget::medium()` (line 204): Used as Default - remove attribute
- `TokenBudget::large()` (line 214): Marked dead_code - either use or remove
- `TokenBudget::massive()` (line 225): Marked dead_code - either use or remove

**Decision**: Remove unused presets (small, large, massive), keep medium as default

**Acceptance Criteria**:
- [ ] Wrap `TOKENIZER` static in `#[cfg(test)]`
- [ ] Wrap `get_tokenizer()` in `#[cfg(test)]`
- [ ] Remove `TokenBudget::small()` method
- [ ] Remove `TokenBudget::large()` method
- [ ] Remove `TokenBudget::massive()` method
- [ ] Remove `#[allow(dead_code)]` from `TokenBudget::medium()`
- [ ] Update tests that use removed presets to use `medium()`
- [ ] `just check` passes

---

#### Task 2.3: Clean up orchestrator.rs dead code
**File**: `christina/src/io/llm/orchestrator.rs`
**Line**: 155

**Analysis**:
- `AIOrchestrator::new()` (line 155-158): Simple constructor, not used

**Decision**: Remove unused constructor, keep `with_config()` as primary constructor

**Acceptance Criteria**:
- [ ] Remove `AIOrchestrator::new()` method
- [ ] Remove `#[allow(dead_code)]` attribute
- [ ] Update any tests to use `with_config()` instead
- [ ] `just check` passes

---

#### Task 2.4: Clean up elm.rs dead code
**File**: `christina/src/tui/elm.rs`
**Lines**: 5, 8

**Analysis**:
- `ToastLevel::Success` (line 5): Not used - remove variant or implement usage
- `ToastLevel::Error` (line 8): Not used - remove variant or implement usage

**Decision**: Remove unused variants to simplify ToastLevel enum

**Acceptance Criteria**:
- [ ] Remove `ToastLevel::Success` variant
- [ ] Remove `ToastLevel::Error` variant
- [ ] Update any code referencing these variants
- [ ] `just check` passes

---

#### Task 2.5: Clean up edit_history.rs dead code
**File**: `christina/src/app/edit_history.rs`
**Line**: 105

**Analysis**:
- `EditHistory::load()` (line 105): Not used - remove or implement usage

**Decision**: Remove unused `load()` method

**Acceptance Criteria**:
- [ ] Remove `EditHistory::load()` method
- [ ] Remove `#[allow(dead_code)]` attribute
- [ ] `just check` passes

---

### Wave 3: Unwrap/Expect Cleanup (High Priority)
**Parallel Tasks**: 2 tasks can run simultaneously

#### Task 3.1: Fix diff_executor.rs unwrap
**File**: `christina/src/tui/diff_executor.rs`
**Line**: 211

**Current Code**:
```rust
let result = tool.render_diff(input, 80).unwrap();
```

**Fix**: Propagate error or use graceful fallback

**Acceptance Criteria**:
- [ ] Replace `.unwrap()` with `?` or match handling
- [ ] Function signature updated to return `Result` if needed
- [ ] Callers updated to handle potential error
- [ ] `just clippy` shows no warnings for this file

---

#### Task 3.2: Fix tokenizer.rs unwraps
**File**: `christina/src/io/llm/tokenizer.rs`
**Lines**: 35, 40, 63

**Analysis**:
- Lines 35, 40: `TOKENIZER.get().unwrap()` - These are safe due to OnceLock guarantees but should use `expect()` with justification
- Line 63: `NonZeroUsize::new(TOKEN_CACHE_CAPACITY).unwrap()` - Safe due to constant being non-zero, should use `expect()`

**Fix**: Add explicit expect messages or use safe alternatives

**Acceptance Criteria**:
- [ ] Line 35: Replace with `.expect("Tokenizer initialized above")`
- [ ] Line 40: Replace with `.expect("Tokenizer initialized by other thread")`
- [ ] Line 63: Replace with `.expect("TOKEN_CACHE_CAPACITY is non-zero constant")`
- [ ] `just clippy` shows no warnings for this file

---

### Wave 4: Clippy Compliance (Final)
**Sequential Tasks**: Must run after Waves 1-3

#### Task 4.1: Run just check and fix remaining issues
**Command**: `just check`

**Acceptance Criteria**:
- [ ] `just check` passes with zero warnings
- [ ] All compilation errors resolved
- [ ] All unused import warnings resolved

---

#### Task 4.2: Run just clippy and fix remaining issues
**Command**: `just clippy`

**Acceptance Criteria**:
- [ ] `just clippy` passes with zero warnings
- [ ] All clippy lints resolved
- [ ] No `#[allow(...)]` attributes added to suppress warnings (fix root cause instead)

---

## Verification Strategy

### Test Commands
```bash
# Check compilation
just check

# Run clippy
just clippy

# Run tests to ensure no regressions
just test
```

### Manual Verification
1. Run the TUI in a git repository with staged changes
2. Navigate through all screens (Staging → Dashboard → Generating → Review → Editing)
3. Verify commit generation works end-to-end
4. Verify error handling works (e.g., invalid API key, no staged changes)

---

## Key Risks

### Risk 1: Breaking Test Code
**Mitigation**: Only modify non-test code. Tests are marked with `#[cfg(test)]` - do not touch these sections.

### Risk 2: Removing Actually-Used Code
**Mitigation**: Before removing any function, verify it's not used via:
- `grep -r "function_name" christina/src/ --include="*.rs"`
- Check if it's a public API that external code might use

### Risk 3: TUI Message Handling Regressions
**Mitigation**: After fixing panic branches, manually test all TUI flows to ensure messages are handled correctly.

### Risk 4: TokenBudget Preset Removal Breaks Config
**Mitigation**: Check if presets are referenced in configuration files or documentation before removing.

---

## Files to Modify (Summary)

| File | Lines | Action |
|------|-------|--------|
| `christina/src/tui/screens/staging.rs` | 325, 338 | Replace panic with graceful handling |
| `christina/src/tui/screens/error.rs` | 245, 258, 271, 298 | Replace panic with graceful handling |
| `christina/src/tui/screens/dashboard.rs` | 700 | Replace panic with graceful handling |
| `christina/src/io/git/adapter.rs` | 8-82 | Remove unused functions, clean attributes |
| `christina/src/io/llm/tokenizer.rs` | 16, 24, 193, 204, 214, 225 | Clean dead code, fix unwraps |
| `christina/src/io/llm/orchestrator.rs` | 155-158 | Remove unused constructor |
| `christina/src/tui/elm.rs` | 5, 8 | Remove unused enum variants |
| `christina/src/app/edit_history.rs` | 105 | Remove unused method |
| `christina/src/tui/diff_executor.rs` | 211 | Fix unwrap |

---

## Success Criteria

### Final Checklist
- [ ] All TUI panic branches eliminated
- [ ] All `#[allow(dead_code)]` removed from non-test code
- [ ] All production unwraps have justification or are eliminated
- [ ] `just check` passes with zero warnings
- [ ] `just clippy` passes with zero warnings
- [ ] All tests pass (`just test`)
- [ ] End-to-end commit generation flow verified working

### Evidence to Capture
- Screenshot or terminal output of `just check` passing
- Screenshot or terminal output of `just clippy` passing
- Screenshot or terminal output of `just test` passing
- Manual test: TUI navigation through all screens

---

## Appendix: Current Code Locations

### TUI Panic Sites
```
christina/src/tui/screens/staging.rs:325
christina/src/tui/screens/staging.rs:338
christina/src/tui/screens/error.rs:245
christina/src/tui/screens/error.rs:258
christina/src/tui/screens/error.rs:271
christina/src/tui/screens/error.rs:298
christina/src/tui/screens/dashboard.rs:700
```

### Dead Code Allowances
```
christina/src/io/llm/orchestrator.rs:155
christina/src/io/llm/tokenizer.rs:16,24,193,204,214,225
christina/src/io/git/adapter.rs:13,76,85,96
christina/src/tui/elm.rs:5,8
christina/src/app/edit_history.rs:105
```

### Unwrap/Expect Sites (Production)
```
christina/src/tui/diff_executor.rs:211
christina/src/io/llm/tokenizer.rs:35,40,63
```
