# AIOrchestrator Robustness Fixes

## TL;DR

> **Quick Summary**: Three targeted fixes to improve robustness of the AIOrchestrator: (1) allow single chunk failures in small batches, (2) add simplified JSON extraction fallback, (3) preserve more context in deletion diffs for move detection.
>
> **Deliverables**:
> - Modified `christina-llm/src/orchestrator.rs` (lines 473, 893-942)
> - Modified `christina-git/src/diff_processor.rs` (lines 120, 123)
> - Updated unit tests in `christina-git/src/parsing.rs`
>
> **Estimated Effort**: Short (~30 minutes)
> **Parallel Execution**: YES - 3 independent tasks
> **Critical Path**: None (all tasks parallel)

---

## Context

### Original Request
User requested three specific fixes to improve AIOrchestrator robustness:

1. **Small Batch Fragility**: Change partial failure threshold logic to allow at least 1 chunk failure regardless of percentage
2. **JSON Parsing Robustness**: Add simplified JSON extraction as a fallback to the brace-counting parser
3. **Deletion Truncation vs. Moves**: Increase truncation limits to preserve context for move detection

### Interview Summary
**Key Decisions**:
- All three fixes are independent and can run in parallel
- Fix 2 implemented as fallback strategy to maintain backward compatibility with existing tests
- Fix 1 only modifies hard abort threshold, not user prompt threshold
- Fix 3 uses constants 50/100 as specified

**Research Findings**:
- Workspace has 4 crates: christina, christina-core, christina-git, christina-llm
- `christina-llm/src/orchestrator.rs` has unit tests in `#[cfg(test)]` module
- `christina-git/src/parsing.rs` has unit tests for `truncate_deletion_diff`
- Code quality gates: `just check` and `just clippy` must pass with zero warnings

### Metis Review
**Identified Gaps** (addressed):
- Fix 2's simplified approach would break existing test expecting balanced extraction
- **Resolution**: Implement as fallback (simplified first, fall back to balanced)
- Fix 1's division-by-zero already guarded by line 456 check
- Fix 3's test `truncate_deletion_diff_respects_line_limit` uses hardcoded value 3

---

## Work Objectives

### Core Objective
Improve AIOrchestrator robustness through three targeted fixes that handle edge cases in partial failure thresholds, JSON parsing, and diff truncation.

### Concrete Deliverables
1. **Fix 1**: Line 473 in `christina-llm/src/orchestrator.rs` - Add `failed_count > 1 &&` condition
2. **Fix 2**: Lines 893-942 in `christina-llm/src/orchestrator.rs` - Implement simplified JSON extraction with fallback
3. **Fix 3**: Lines 120, 123 in `christina-git/src/diff_processor.rs` - Update truncation constants
4. **Test Updates**: Update `truncate_deletion_diff_respects_line_limit` test in `christina-git/src/parsing.rs`

### Definition of Done
- [ ] All three code changes implemented
- [ ] All existing tests pass
- [ ] `just check` passes with zero warnings
- [ ] `just clippy` passes with zero warnings
- [ ] New test added for simplified JSON extraction fallback path

### Must Have
- Backward compatibility for JSON extraction (all existing tests pass)
- Zero compiler warnings
- Zero clippy warnings

### Must NOT Have (Guardrails)
- No changes to public APIs
- No changes to user prompt threshold (line 491)
- No changes to parsing logic beyond constant updates
- No new dependencies

---

## Verification Strategy

### Test Decision
- **Infrastructure exists**: YES (built-in `cargo test`)
- **User wants tests**: YES (Tests-after)
- **Framework**: Built-in Rust test framework

### Test Coverage

**Fix 1** (Partial Failure Threshold):
- Existing tests cover failure rate logic
- No new tests needed - behavior change is straightforward

**Fix 2** (JSON Parsing):
- MUST add test for fallback path
- MUST verify all existing JSON extraction tests still pass
- Test: Force simplified approach to fail, verify balanced kicks in

**Fix 3** (Deletion Truncation):
- MUST update `truncate_deletion_diff_respects_line_limit` test
- Change `max_deletion_lines` from 3 to 50 in test
- Verify truncation notice still appears

### Automated Verification

**For all fixes**:
```bash
# Verify compilation
cargo check --all

# Verify no warnings
just check

# Verify clippy cleanliness
just clippy

# Run all tests
cargo test --all
```

**Expected outputs**:
- `cargo check`: No errors
- `just check`: Zero warnings
- `just clippy`: Zero warnings
- `cargo test`: All tests pass (including updated test)

---

## Execution Strategy

### Parallel Execution Waves

```
Wave 1 (All Parallel - No Dependencies):
├── Task 1: Fix partial failure threshold
│   └── File: christina-llm/src/orchestrator.rs:473
├── Task 2: Implement JSON fallback strategy
│   └── File: christina-llm/src/orchestrator.rs:893-942
└── Task 3: Update deletion truncation constants
    └── File: christina-git/src/diff_processor.rs:116-124

Wave 2 (After Wave 1):
└── Task 4: Update tests and verify
    ├── Update parsing.rs test
    ├── Add JSON fallback test
    └── Run full test suite

Critical Path: None (Wave 1 tasks are independent)
Parallel Speedup: ~60% faster than sequential
```

### Dependency Matrix

| Task | Depends On | Blocks | Can Parallelize With |
|------|------------|--------|---------------------|
| 1 (Failure Threshold) | None | 4 | 2, 3 |
| 2 (JSON Fallback) | None | 4 | 1, 3 |
| 3 (Deletion Truncation) | None | 4 | 1, 2 |
| 4 (Test Updates) | 1, 2, 3 | None | None |

---

## TODOs

### Task 1: Fix Small Batch Fragility (Partial Failure Threshold)

**What to do**:
- Modify line 473 in `christina-llm/src/orchestrator.rs`
- Change condition from `if failure_rate > max_failure_rate` to `if failed_count > 1 && failure_rate > max_failure_rate`
- This allows at least 1 chunk failure regardless of percentage threshold

**Must NOT do**:
- Do NOT modify the user prompt threshold at line 491
- Do NOT change the error message format
- Do NOT modify the `max_failure_rate()` function

**Recommended Agent Profile**:
- **Category**: `quick`
- **Reason**: Single line change, straightforward logic
- **Skills**: None needed

**Parallelization**:
- **Can Run In Parallel**: YES
- **Parallel Group**: Wave 1
- **Blocks**: Task 4 (Test verification)
- **Blocked By**: None

**References**:
- `christina-llm/src/orchestrator.rs:473` - Target line to modify
- `christina-llm/src/orchestrator.rs:456` - Guard check ensuring `total_chunks > 0`
- `christina-llm/src/orchestrator.rs:1052-1058` - `max_failure_rate()` function

**Acceptance Criteria**:
- [ ] Line 473 condition includes `failed_count > 1 &&`
- [ ] `cargo check` passes
- [ ] All existing tests pass
- [ ] `just clippy` shows zero warnings

**Commit**: YES
- Message: `fix(orchestrator): allow single chunk failure in small batches`
- Files: `christina-llm/src/orchestrator.rs`
- Pre-commit: `cargo test --package christina-llm`

---

### Task 2: Implement JSON Fallback Strategy

**What to do**:
- Modify `extract_balanced_json` function in `christina-llm/src/orchestrator.rs` (lines 893-942)
- Implement two-phase extraction:
  1. **Phase 1 (Simplified)**: Find first `{` and last `}`, extract substring, validate with `serde_json::from_str`
  2. **Phase 2 (Fallback)**: If Phase 1 fails, use existing brace-counting logic
- Rename function to `extract_json_with_fallback` or keep name for backward compatibility

**Implementation approach**:
```rust
fn extract_balanced_json(response: &str) -> Option<String> {
    // Phase 1: Try simplified approach
    if let Some(first) = response.find('{') {
        if let Some(last) = response.rfind('}') {
            if last > first {
                let candidate = &response[first..=last];
                if serde_json::from_str::<serde_json::Value>(candidate).is_ok() {
                    return Some(candidate.to_string());
                }
            }
        }
    }
    
    // Phase 2: Fall back to balanced brace counting
    // ... existing implementation ...
}
```

**Must NOT do**:
- Do NOT remove the existing brace-counting logic
- Do NOT change the function signature
- Do NOT break existing tests
- Do NOT add new dependencies

**Recommended Agent Profile**:
- **Category**: `quick`
- **Reason**: Logic modification within existing function, requires careful testing
- **Skills**: None needed

**Parallelization**:
- **Can Run In Parallel**: YES
- **Parallel Group**: Wave 1
- **Blocks**: Task 4 (Test verification)
- **Blocked By**: None

**References**:
- `christina-llm/src/orchestrator.rs:893-942` - Current `extract_balanced_json` implementation
- `christina-llm/src/orchestrator.rs:1238-1310` - Existing tests for JSON extraction
- Test at line ~1654: "Should extract balanced JSON, not just first { to last }"

**Acceptance Criteria**:
- [ ] Simplified extraction implemented as first attempt
- [ ] Fallback to brace-counting implemented
- [ ] All existing JSON extraction tests pass
- [ ] New test added: Force simplified to fail, verify balanced succeeds
- [ ] `cargo check` passes
- [ ] `just clippy` shows zero warnings

**Test to add**:
```rust
#[test]
fn extract_json_fallback_to_balanced() {
    let provider = Arc::new(Provider::default());
    let orchestrator = AIOrchestrator::new(provider);
    
    // This response has nested JSON that simplified approach would get wrong
    // but balanced approach handles correctly
    let response = r#"Some text {"outer": {"inner": "value"}} more text"#;
    
    let json = orchestrator.extract_json(response);
    // Should extract balanced JSON, not first { to last }
    assert!(json.contains("\"outer\""));
    assert!(!json.contains(" more text"));
}
```

**Commit**: YES
- Message: `feat(orchestrator): add simplified JSON extraction with fallback`
- Files: `christina-llm/src/orchestrator.rs`
- Pre-commit: `cargo test --package christina-llm extract_json`

---

### Task 3: Update Deletion Truncation Constants

**What to do**:
- Modify `christina-git/src/diff_processor.rs` lines 120 and 123
- Line 120: Change `truncate_deletion_diff(diff, 3)` to `truncate_deletion_diff(diff, 50)`
- Line 123: Change `truncate_deletion_diff(diff, 10)` to `truncate_deletion_diff(diff, 100)`

**Must NOT do**:
- Do NOT change the `truncate_deletion_diff` function signature
- Do NOT modify truncation logic in parsing.rs
- Do NOT change behavior for non-deletion diffs

**Recommended Agent Profile**:
- **Category**: `quick`
- **Reason**: Simple constant updates
- **Skills**: None needed

**Parallelization**:
- **Can Run In Parallel**: YES
- **Parallel Group**: Wave 1
- **Blocks**: Task 4 (Test verification)
- **Blocked By**: None

**References**:
- `christina-git/src/diff_processor.rs:116-124` - Target lines
- `christina-git/src/parsing.rs:226-287` - `truncate_deletion_diff` function
- `christina-git/src/parsing.rs:489-527` - Test `truncate_deletion_diff_respects_line_limit`

**Acceptance Criteria**:
- [ ] Line 120 updated to use 50 instead of 3
- [ ] Line 123 updated to use 100 instead of 10
- [ ] `cargo check` passes
- [ ] `just clippy` shows zero warnings

**Commit**: YES
- Message: `fix(diff-processor): increase deletion truncation limits for move detection`
- Files: `christina-git/src/diff_processor.rs`
- Pre-commit: `cargo check --package christina-git`

---

### Task 4: Update Tests and Final Verification

**What to do**:
1. Update `truncate_deletion_diff_respects_line_limit` test in `christina-git/src/parsing.rs`
   - Change `max_deletion_lines` from 3 to 50
   - Update assertions to check first 50 lines instead of 3
2. Add test for JSON fallback strategy in `christina-llm/src/orchestrator.rs`
3. Run full test suite
4. Verify `just check` and `just clippy` pass

**Test Updates**:

**Parsing test update** (line ~508):
```rust
#[test]
fn truncate_deletion_diff_respects_line_limit() {
    // ... existing setup ...
    let truncated = truncate_deletion_diff(deletion_diff, 50); // Changed from 3
    
    // Should keep first 50 deletion lines (changed from 3)
    assert!(truncated.contains("-line 1"));
    assert!(truncated.contains("-line 50"));
    assert!(!truncated.contains("-line 51")); // Changed from line 10
}
```

**Recommended Agent Profile**:
- **Category**: `quick`
- **Reason**: Test updates and verification
- **Skills**: None needed

**Parallelization**:
- **Can Run In Parallel**: NO
- **Parallel Group**: Wave 2 (final)
- **Blocks**: None
- **Blocked By**: Tasks 1, 2, 3

**Acceptance Criteria**:
- [ ] Parsing test updated with new constants
- [ ] New JSON fallback test added
- [ ] All tests pass: `cargo test --all`
- [ ] `just check` passes with zero warnings
- [ ] `just clippy` passes with zero warnings

**Commit**: YES (can be squashed with respective changes or separate)
- Message: `test: update tests for truncation and JSON fallback`
- Files: `christina-git/src/parsing.rs`, `christina-llm/src/orchestrator.rs`
- Pre-commit: `cargo test --all && just check && just clippy`

---

## Commit Strategy

| After Task | Message | Files | Verification |
|------------|---------|-------|--------------|
| 1 | `fix(orchestrator): allow single chunk failure in small batches` | `christina-llm/src/orchestrator.rs` | `cargo test --package christina-llm` |
| 2 | `feat(orchestrator): add simplified JSON extraction with fallback` | `christina-llm/src/orchestrator.rs` | `cargo test --package christina-llm extract_json` |
| 3 | `fix(diff-processor): increase deletion truncation limits` | `christina-git/src/diff_processor.rs` | `cargo check --package christina-git` |
| 4 | `test: update tests for truncation and JSON fallback` | `christina-git/src/parsing.rs`, `christina-llm/src/orchestrator.rs` | `cargo test --all` |

---

## Success Criteria

### Verification Commands
```bash
# Compilation
cargo check --all
# Expected: No errors

# Warnings check
just check
# Expected: Zero warnings

# Clippy check
just clippy
# Expected: Zero warnings

# Test suite
cargo test --all
# Expected: All tests pass
```

### Final Checklist
- [ ] Fix 1: Line 473 has `failed_count > 1 &&` condition
- [ ] Fix 2: JSON extraction has simplified + fallback logic
- [ ] Fix 3: Lines 120 and 123 use 50 and 100 respectively
- [ ] All existing tests pass
- [ ] New JSON fallback test added
- [ ] Parsing test updated for new truncation limits
- [ ] Zero compiler warnings
- [ ] Zero clippy warnings

---

## Notes

### Fix 2 Design Rationale
The fallback strategy was chosen because:
1. The existing test explicitly expects balanced extraction (not first-to-last)
2. First-to-last approach fails on nested JSON objects
3. Fallback maintains backward compatibility while adding requested functionality
4. No performance concern - simplified approach runs first and is O(n)

### Token Cost Impact (Fix 3)
Increasing truncation limits from 3/10 to 50/100 lines may increase token usage by ~10-16x for deletion-heavy workflows. This is accepted as a trade-off for better move detection context.

### Fix 1 Scope Boundary
Only the hard abort threshold (line 473) is modified. The user prompt threshold (line 491) remains unchanged, meaning users will still be prompted for confirmation on single failures above 5%.
