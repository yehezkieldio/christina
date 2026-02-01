# Comprehensive Parallel Execution Plan: Complete Christina Rust Codebase

## TL;DR

> **Core Objective**: Implement missing AI orchestration, retry/concurrency, diff chunking, and GPG signing to achieve behavioral parity with old_backup reference.
>
> **Deliverables**:
> - Full AIOrchestrator with Map-Reduce pipeline
> - RequestLimiter with semaphore + token bucket
> - RetryPolicy with exponential backoff + jitter
> - DiffProcessor with token-aware chunking
> - GPG signing support in git adapter
> - Comprehensive test coverage for all new modules
>
> **Estimated Effort**: Large (6-8 waves, ~20 tasks)
> **Parallel Execution**: YES - 3-4 tasks per wave
> **Critical Path**: Task 1 → Task 2 → Task 3 → Task 4 → Task 5 → Task 18

---

## Context

### Original Request
Create a comprehensive, parallel execution plan to finish and harden an existing Rust codebase by closing gaps, implementing stubs, and ensuring behavioral completeness.

### Current State Assessment

**Workspace Structure**:
- `christina/` - Binary crate (61 files) - TUI, event loop, git/LLM adapters
- `christina-core/` - Library crate (37 files) - Types, config, state machine
- Edition 2024, strict lints (zero warnings enforced)

**What's Complete (Production Quality)**:
1. ✅ Error handling: Exhaustive error types with thiserror
2. ✅ State machine: AppState FSM with generation_id tracking
3. ✅ Type system: Strong types (CommitMessage, FilePath, TokenCount, etc.)
4. ✅ Config: ConfigFile, ResolvedConfig, Profiles CRUD
5. ✅ Git types: GitFile, GitFileStatus, DiffChunk, FileDiff
6. ✅ LLM types: ChatMessage, Role, LlmRequest, LlmResponse
7. ✅ Prompt system: 5 const prompts, Theme, PromptBuilder
8. ✅ Tokenizer trait: count_tokens, encode, decode, slice_to_token_limit
9. ✅ TUI screens: All 6 screens fully implemented with tests
10. ✅ Event loop: Complete with producers, handlers
11. ✅ Basic git adapter: status, stage, unstage, commit, diff extraction
12. ✅ Basic LLM providers: OpenAI, Azure using `llm` crate

### Critical Gaps Identified

#### 🚨 HIGH PRIORITY

1. **christina/src/generate.rs is a STUB**
   - Current: Simple direct LLM call with basic prompt
   - Missing: Map-Reduce pipeline, concurrent chunking, retry policies, partial failure handling
   - Reference: old_backup/christina-llm/src/orchestrator.rs

2. **Retry/Concurrency Infrastructure Missing**
   - No RequestLimiter (semaphore + token bucket)
   - No RetryPolicy (exponential backoff + jitter)
   - No IsTransient trait for error classification
   - Reference: old_backup/christina-llm/src/concurrency.rs, retry.rs

3. **Diff Chunking Algorithms Missing**
   - No DiffProcessor module
   - No token-aware splitting
   - No file → hunks → lines → binary search slice algorithm
   - Reference: old_backup/christina-git/src/diff_processor.rs, chunking.rs, parsing.rs

4. **GPG Signing Not Implemented**
   - Git adapter lacks GPG signing support
   - No git config gpg.program detection
   - No commit buffer signing
   - Reference: old_backup/christina-git/src/repository.rs (create_commit with GPG)

5. **LLM Provider Resilience Missing**
   - No retry logic in execute_openai_request/execute_azure_request
   - No rate limiting
   - No streaming support
   - Groq provider not wired to ProviderKind

### Additional Gaps

6. **Integration Points**:
   - Event loop needs to integrate with new AIOrchestrator
   - Progress events need proper stage reporting
   - Token count updates during chunking

7. **Test Coverage Gaps**:
   - No tests for retry/concurrency logic
   - No tests for diff chunking
   - No tests for GPG signing
   - Integration tests for full generation flow

8. **Performance Considerations**:
   - No bounded parallelism for chunk processing
   - No buffer pooling for diff processing
   - No cancellation support for long-running operations

---

## Work Objectives

### Core Objective
Implement the missing AI orchestration layer with Map-Reduce pipeline, retry/concurrency management, diff chunking, and GPG signing to achieve behavioral parity with the old_backup reference implementation.

### Concrete Deliverables
- [ ] AIOrchestrator module with Map-Reduce pipeline
- [ ] RequestLimiter with semaphore + token bucket
- [ ] RetryPolicy with exponential backoff and full jitter
- [ ] DiffProcessor with token-aware chunking
- [ ] GPG signing support in git adapter
- [ ] Enhanced LLM providers with retry and rate limiting
- [ ] Comprehensive test coverage (>80% for new modules)
- [ ] Integration tests for full generation flow

### Definition of Done
- [ ] All TODOs removed from generate.rs
- [ ] `cargo check` passes with zero warnings
- [ ] `cargo clippy` passes with zero warnings
- [ ] All tests pass (`cargo test`)
- [ ] No behavioral gaps vs old_backup reference
- [ ] Documentation comments for all public APIs

### Must Have
- Map-Reduce pipeline for large diffs
- Retry with exponential backoff and jitter
- Rate limiting to avoid provider throttling
- Token-aware diff chunking
- GPG signing support
- Partial failure handling
- Comprehensive test coverage

### Must NOT Have (Guardrails)
- NO changes to existing architecture patterns
- NO refactoring of working code
- NO changes to Elm architecture
- NO breaking changes to existing APIs unless necessary
- NO new dependencies without justification
- NO premature abstraction

---

## Verification Strategy

### Test Decision
- **Infrastructure exists**: YES (built-in test framework)
- **User wants tests**: TDD for new modules, tests-after for integration
- **Framework**: Built-in `cargo test`

### Test Strategy by Module

**New Infrastructure (TDD)**:
1. Write tests first defining expected behavior
2. Implement to make tests pass
3. Refactor while keeping tests green

**Integration (Tests-after)**:
1. Implement feature
2. Write integration tests
3. Verify end-to-end behavior

### Acceptance Criteria Format

Each task includes:
- Unit tests: `cargo test <module>` → PASS
- Integration tests: `cargo test integration` → PASS
- Quality gates: `just check && just clippy` → zero warnings

---

## Execution Strategy

### Parallel Execution Waves

```
Wave 1 (Foundation - Independent):
├── Task 1: Create RetryPolicy module
├── Task 2: Create RequestLimiter module
└── Task 3: Create diff parsing utilities

Wave 2 (Core Algorithms - Depends on Wave 1):
├── Task 4: Implement DiffProcessor
├── Task 5: Implement diff chunking algorithm
└── Task 6: Add IsTransient trait and error classification

Wave 3 (LLM Resilience - Depends on Wave 1):
├── Task 7: Enhance OpenAI provider with retry
├── Task 8: Enhance Azure provider with retry
└── Task 9: Wire Groq provider to ProviderKind

Wave 4 (AI Orchestration - Depends on Waves 1-3):
├── Task 10: Implement AIOrchestrator core
├── Task 11: Implement Map phase (chunk summarization)
└── Task 12: Implement Reduce phase (synthesis)

Wave 5 (Git Enhancement - Independent of Waves 1-4):
├── Task 13: Implement GPG signing support
└── Task 14: Enhance git adapter with unborn branch handling

Wave 6 (Integration - Depends on Waves 1-5):
├── Task 15: Integrate AIOrchestrator with generate.rs
├── Task 16: Integrate DiffProcessor with event loop
└── Task 17: Add progress event reporting

Wave 7 (Testing & Polish - Depends on Waves 1-6):
├── Task 18: Write integration tests
├── Task 19: Write documentation
└── Task 20: Final quality gate verification
```

### Dependency Matrix

| Task | Depends On | Blocks | Can Parallelize With |
|------|------------|--------|---------------------|
| 1 | None | 6, 7, 8 | 2, 3 |
| 2 | None | 4, 10 | 1, 3 |
| 3 | None | 4, 5 | 1, 2 |
| 4 | 2, 3 | 10 | 5, 6 |
| 5 | 3 | 10 | 4, 6 |
| 6 | 1 | 7, 8, 10 | 4, 5 |
| 7 | 1, 6 | 10 | 8, 9 |
| 8 | 1, 6 | 10 | 7, 9 |
| 9 | None | 10 | 7, 8 |
| 10 | 1-9 | 11, 12 | None |
| 11 | 10 | 15 | 12 |
| 12 | 10 | 15 | 11 |
| 13 | None | 15 | 1-12 |
| 14 | None | 15 | 1-12 |
| 15 | 10-14 | 16, 17 | None |
| 16 | 4, 15 | 18 | 17 |
| 17 | 10, 15 | 18 | 16 |
| 18 | 15-17 | 19, 20 | None |
| 19 | 18 | 20 | None |
| 20 | 18, 19 | None | None |

### Critical Path
Task 1 → Task 2 → Task 4 → Task 10 → Task 15 → Task 18 → Task 20

---

## TODOs

### Task 1: Implement RetryPolicy Module

**What to do**:
- Create `christina/src/retry.rs` module
- Implement `RetryPolicy` struct with exponential backoff
- Implement `retry_with_backoff` function
- Add `IsTransient` trait for error classification
- Support configurable: max_retries, base_delay, max_delay, jitter

**Must NOT do**:
- Don't use external retry crates (backon, etc.) - implement manually
- Don't block on retries - use async/await

**Recommended Agent Profile**:
- **Category**: `ultrabrain` (algorithm implementation)
- **Skills**: `rust`, `async`, `error-handling`

**Parallelization**:
- **Can Run In Parallel**: YES
- **Parallel Group**: Wave 1
- **Blocks**: Task 6, 7, 8
- **Blocked By**: None

**References**:
- old_backup/christina-llm/src/retry.rs - Full implementation reference
- Pattern: Exponential backoff with full jitter
- Formula: delay = min(base_delay * 2^attempt, max_delay) + random_jitter

**Acceptance Criteria**:
- [ ] `RetryPolicy::new()` creates policy with defaults
- [ ] `retry_with_backoff` retries on transient errors
- [ ] `retry_with_backoff` fails fast on non-transient errors
- [ ] Jitter is applied to prevent thundering herd
- [ ] All delays use tokio::time::sleep (non-blocking)
- [ ] Unit tests: 100% coverage of retry logic
- [ ] `cargo test retry` → PASS

**Commit**: YES
- Message: `feat(retry): implement RetryPolicy with exponential backoff`
- Files: `christina/src/retry.rs`, `christina/src/lib.rs`

---

### Task 2: Implement RequestLimiter Module

**What to do**:
- Create `christina/src/concurrency.rs` module
- Implement `RequestLimiter` combining Semaphore + TokenBucket
- Support concurrent request limiting
- Support rate limiting (requests per second)
- Add deterministic jitter for retry distribution

**Must NOT do**:
- Don't use external rate limit crates
- Don't block threads - use async acquire

**Recommended Agent Profile**:
- **Category**: `ultrabrain` (concurrency primitives)
- **Skills**: `rust`, `async`, `tokio`, `concurrency`

**Parallelization**:
- **Can Run In Parallel**: YES
- **Parallel Group**: Wave 1
- **Blocks**: Task 4, 10
- **Blocked By**: None

**References**:
- old_backup/christina-llm/src/concurrency.rs - Full implementation
- Pattern: Semaphore for concurrency + TokenBucket for rate
- TokenBucket: refill rate, burst capacity

**Acceptance Criteria**:
- [ ] `RequestLimiter::new(max_concurrent, max_rate)` creates limiter
- [ ] `acquire().await` respects concurrency limit
- [ ] `acquire().await` respects rate limit
- [ ] Permits are released on drop (RAII)
- [ ] Deterministic jitter seed for testing
- [ ] Unit tests: concurrency bounds, rate limiting accuracy
- [ ] `cargo test concurrency` → PASS

**Commit**: YES
- Message: `feat(concurrency): implement RequestLimiter with semaphore + token bucket`
- Files: `christina/src/concurrency.rs`, `christina/src/lib.rs`

---

### Task 3: Create Diff Parsing Utilities

**What to do**:
- Create `christina/src/io/git/parsing.rs` module
- Implement `split_by_files` function
- Implement `extract_file_paths` function
- Implement `truncate_deletion_diff` function
- Add helper for safe string truncation

**Must NOT do**:
- Don't use regex for parsing (performance)
- Don't allocate unnecessarily

**Recommended Agent Profile**:
- **Category**: `quick` (string processing)
- **Skills**: `rust`, `string-parsing`

**Parallelization**:
- **Can Run In Parallel**: YES
- **Parallel Group**: Wave 1
- **Blocks**: Task 4, 5
- **Blocked By**: None

**References**:
- old_backup/christina-git/src/parsing.rs - Full implementation
- Pattern: Find "diff --git " headers, extract paths
- Handle: a/, b/, c/, i/ prefixes, quoted paths, no-prefix

**Acceptance Criteria**:
- [ ] `split_by_files` correctly splits multi-file diffs
- [ ] `extract_file_paths` handles all git path formats
- [ ] `truncate_deletion_diff` preserves metadata
- [ ] All functions handle edge cases (empty, malformed)
- [ ] Unit tests: 90%+ coverage
- [ ] `cargo test parsing` → PASS

**Commit**: YES
- Message: `feat(git): implement diff parsing utilities`
- Files: `christina/src/io/git/parsing.rs`, `christina/src/io/git/mod.rs`

---

### Task 4: Implement DiffProcessor

**What to do**:
- Create `christina/src/io/git/diff_processor.rs` module
- Implement `DiffProcessor` struct
- Implement `process_safe` method
- Add binary detection logic
- Add deletion-only diff truncation
- Add token budget checking

**Must NOT do**:
- Don't process diffs > MAX_DIFF_SIZE (10MB)
- Don't send binary content to LLM

**Recommended Agent Profile**:
- **Category**: `ultrabrain` (algorithm orchestration)
- **Skills**: `rust`, `performance`, `git`

**Parallelization**:
- **Can Run In Parallel**: NO (depends on Task 2, 3)
- **Parallel Group**: Wave 2
- **Blocks**: Task 10, 16
- **Blocked By**: Task 2, 3

**References**:
- old_backup/christina-git/src/diff_processor.rs - Full implementation
- Pattern: Binary detection → Token check → Chunk or return single
- Binary detection: fast markers + NUL byte sampling + extension heuristics

**Acceptance Criteria**:
- [ ] `DiffProcessor::new(tokenizer, token_limit)` creates processor
- [ ] `process_safe(diff)` returns chunks or single chunk
- [ ] Binary files detected and marked
- [ ] Deletion-only diffs truncated
- [ ] Token budget respected
- [ ] Unit tests: binary detection, truncation, token limits
- [ ] `cargo test diff_processor` → PASS

**Commit**: YES
- Message: `feat(git): implement DiffProcessor with binary detection`
- Files: `christina/src/io/git/diff_processor.rs`

---

### Task 5: Implement Diff Chunking Algorithm

**What to do**:
- Create `christina/src/io/git/chunking.rs` module
- Implement `split_recursive` function
- Implement `split_by_hunks` function
- Implement `split_by_lines` function
- Implement `split_oversized_line` function (binary search)
- Add buffer pool for allocation optimization

**Must NOT do**:
- Don't split mid-UTF8 sequence
- Don't lose context between chunks

**Recommended Agent Profile**:
- **Category**: `ultrabrain` (complex algorithm)
- **Skills**: `rust`, `algorithms`, `performance`

**Parallelization**:
- **Can Run In Parallel**: NO (depends on Task 3)
- **Parallel Group**: Wave 2
- **Blocks**: Task 10
- **Blocked By**: Task 3

**References**:
- old_backup/christina-git/src/chunking.rs - Full implementation
- Pattern: Greedy first-fit packing
- Algorithm: files → hunks → lines → binary search slice

**Acceptance Criteria**:
- [ ] `split_recursive` packs files greedily by token limit
- [ ] Lockfiles get hard token limit
- [ ] `split_by_hunks` preserves hunk boundaries
- [ ] `split_by_lines` preserves line boundaries
- [ ] `split_oversized_line` finds UTF8-safe split points
- [ ] Buffer pool reduces allocations
- [ ] Unit tests: all split functions, edge cases
- [ ] `cargo test chunking` → PASS

**Commit**: YES
- Message: `feat(git): implement token-aware diff chunking`
- Files: `christina/src/io/git/chunking.rs`

---

### Task 6: Add IsTransient Trait and Error Classification

**What to do**:
- Add `IsTransient` trait to `christina-core/src/error.rs`
- Implement for `CompletionError`
- Implement for `GitError`
- Add error classification logic

**Must NOT do**:
- Don't classify all errors as transient
- Don't retry auth errors

**Recommended Agent Profile**:
- **Category**: `quick` (trait implementation)
- **Skills**: `rust`, `error-handling`

**Parallelization**:
- **Can Run In Parallel**: NO (depends on Task 1)
- **Parallel Group**: Wave 2
- **Blocks**: Task 7, 8, 10
- **Blocked By**: Task 1

**References**:
- old_backup/christina-llm/src/retry.rs - IsTransient trait
- Pattern: Network/timeout = transient, Auth/invalid = fatal

**Acceptance Criteria**:
- [ ] `IsTransient` trait defined with `is_transient(&self) -> bool`
- [ ] Implemented for `CompletionError`
- [ ] Implemented for `GitError`
- [ ] Network errors classified as transient
- [ ] Auth errors classified as fatal
- [ ] Unit tests: classification accuracy
- [ ] `cargo test error` → PASS

**Commit**: YES
- Message: `feat(error): add IsTransient trait for retry classification`
- Files: `christina-core/src/error.rs`

---

### Task 7: Enhance OpenAI Provider with Retry

**What to do**:
- Modify `christina/src/io/llm/openai.rs`
- Integrate `RetryPolicy` from Task 1
- Integrate `RequestLimiter` from Task 2
- Add `IsTransient` error classification
- Add request timeout handling

**Must NOT do**:
- Don't change function signature (keep backward compatible)
- Don't remove existing functionality

**Recommended Agent Profile**:
- **Category**: `unspecified-high` (integration)
- **Skills**: `rust`, `async`, `api-client`

**Parallelization**:
- **Can Run In Parallel**: NO (depends on Task 1, 6)
- **Parallel Group**: Wave 3
- **Blocks**: Task 10
- **Blocked By**: Task 1, 6

**References**:
- old_backup/christina-llm/src/providers/openai.rs - Retry integration
- Pattern: Wrap LLM call in retry_with_backoff

**Acceptance Criteria**:
- [ ] OpenAI requests use RetryPolicy
- [ ] OpenAI requests use RequestLimiter
- [ ] Transient errors trigger retry
- [ ] Fatal errors fail fast
- [ ] Timeout per attempt (not total)
- [ ] Unit tests: retry behavior, timeout handling
- [ ] `cargo test openai` → PASS

**Commit**: YES
- Message: `feat(llm): add retry and rate limiting to OpenAI provider`
- Files: `christina/src/io/llm/openai.rs`

---

### Task 8: Enhance Azure Provider with Retry

**What to do**:
- Modify `christina/src/io/llm/azure.rs`
- Integrate `RetryPolicy` from Task 1
- Integrate `RequestLimiter` from Task 2
- Add `IsTransient` error classification
- Add request timeout handling

**Must NOT do**:
- Don't duplicate logic from OpenAI provider
- Don't change function signature

**Recommended Agent Profile**:
- **Category**: `unspecified-high` (integration)
- **Skills**: `rust`, `async`, `api-client`

**Parallelization**:
- **Can Run In Parallel**: NO (depends on Task 1, 6)
- **Parallel Group**: Wave 3
- **Blocks**: Task 10
- **Blocked By**: Task 1, 6

**References**:
- old_backup/christina-llm/src/providers/azure.rs - Retry integration

**Acceptance Criteria**:
- [ ] Azure requests use RetryPolicy
- [ ] Azure requests use RequestLimiter
- [ ] Transient errors trigger retry
- [ ] Fatal errors fail fast
- [ ] Unit tests: retry behavior
- [ ] `cargo test azure` → PASS

**Commit**: YES
- Message: `feat(llm): add retry and rate limiting to Azure provider`
- Files: `christina/src/io/llm/azure.rs`

---

### Task 9: Wire Groq Provider to ProviderKind

**What to do**:
- Add `Groq` variant to `ProviderKind` enum in christina-core
- Update `christina/src/io/llm/groq.rs` to remove `#[expect(dead_code)]`
- Add Groq handling in generate.rs match statement
- Update config parsing for Groq

**Must NOT do**:
- Don't add retry yet (covered in separate task if needed)
- Don't break existing provider selection

**Recommended Agent Profile**:
- **Category**: `quick` (wiring)
- **Skills**: `rust`, `enums`

**Parallelization**:
- **Can Run In Parallel**: YES
- **Parallel Group**: Wave 3
- **Blocks**: None (optional enhancement)
- **Blocked By**: None

**Acceptance Criteria**:
- [ ] `ProviderKind::Groq` exists
- [ ] Groq provider is callable from generate.rs
- [ ] Config supports Groq provider selection
- [ ] Unit tests: Groq provider selection
- [ ] `cargo test groq` → PASS

**Commit**: YES
- Message: `feat(llm): wire Groq provider to ProviderKind enum`
- Files: `christina-core/src/types/provider_kind.rs`, `christina/src/io/llm/groq.rs`, `christina/src/generate.rs`

---

### Task 10: Implement AIOrchestrator Core

**What to do**:
- Create `christina/src/orchestrator.rs` module
- Implement `AIOrchestrator` struct
- Implement `generate_commit_message` method
- Add direct generation path (single chunk)
- Add Map-Reduce path (multiple chunks)
- Add progress reporting via mpsc

**Must NOT do**:
- Don't implement Map/Reduce phases yet (separate tasks)
- Don't change existing generate.rs signature

**Recommended Agent Profile**:
- **Category**: `ultrabrain` (complex orchestration)
- **Skills**: `rust`, `async`, `architecture`

**Parallelization**:
- **Can Run In Parallel**: NO (depends on Tasks 1-9)
- **Parallel Group**: Wave 4
- **Blocks**: Task 11, 12, 15
- **Blocked By**: Tasks 1-9

**References**:
- old_backup/christina-llm/src/orchestrator.rs - Full implementation
- Pattern: Check chunk count → direct or Map-Reduce
- Progress: Send events via mpsc::Sender<Event>

**Acceptance Criteria**:
- [ ] `AIOrchestrator::new(config, limiter)` creates orchestrator
- [ ] `generate_commit_message(diff, tx)` returns `GenerationResult`
- [ ] Single chunk uses direct generation
- [ ] Multiple chunks uses Map-Reduce (placeholder)
- [ ] Progress events sent at each stage
- [ ] Unit tests: orchestrator creation, direct path
- [ ] `cargo test orchestrator` → PASS

**Commit**: YES
- Message: `feat(orchestrator): implement AIOrchestrator core structure`
- Files: `christina/src/orchestrator.rs`, `christina/src/lib.rs`

---

### Task 11: Implement Map Phase (Chunk Summarization)

**What to do**:
- Implement `map_phase` method in AIOrchestrator
- Process chunks concurrently using `buffer_unordered`
- Generate summaries for each chunk
- Track partial failures
- Apply failure rate threshold

**Must NOT do**:
- Don't process chunks sequentially (use parallelism)
- Don't ignore failures (track and threshold)

**Recommended Agent Profile**:
- **Category**: `ultrabrain` (concurrent processing)
- **Skills**: `rust`, `async`, `futures`, `streams`

**Parallelization**:
- **Can Run In Parallel**: NO (depends on Task 10)
- **Parallel Group**: Wave 4
- **Blocks**: Task 15
- **Blocked By**: Task 10

**References**:
- old_backup/christina-llm/src/orchestrator.rs - Map phase
- Pattern: `stream::iter(chunks).map(|c| summarize(c)).buffer_unordered(n)`
- Failure tracking: Count successes/failures, threshold check

**Acceptance Criteria**:
- [ ] `map_phase` processes chunks concurrently
- [ ] Bounded parallelism via `buffer_unordered`
- [ ] Summaries generated for each chunk
- [ ] Partial failures tracked
- [ ] Failure rate threshold enforced
- [ ] Systemic errors abort immediately
- [ ] Unit tests: concurrent processing, failure handling
- [ ] `cargo test map_phase` → PASS

**Commit**: YES
- Message: `feat(orchestrator): implement Map phase for chunk summarization`
- Files: `christina/src/orchestrator.rs`

---

### Task 12: Implement Reduce Phase (Synthesis)

**What to do**:
- Implement `reduce_phase` method in AIOrchestrator
- Aggregate chunk summaries into themes
- Implement hierarchical intent extraction for many summaries
- Synthesize final commit message
- Add validation and salvage logic

**Must NOT do**:
- Don't lose themes during aggregation
- Don't fail on malformed LLM output (salvage)

**Recommended Agent Profile**:
- **Category**: `ultrabrain` (complex logic)
- **Skills**: `rust`, `llm-prompting`, `parsing`

**Parallelization**:
- **Can Run In Parallel**: NO (depends on Task 10)
- **Parallel Group**: Wave 4
- **Blocks**: Task 15
- **Blocked By**: Task 10

**References**:
- old_backup/christina-llm/src/orchestrator.rs - Reduce phase
- Pattern: Summaries → Themes → Synthesis
- Hierarchical: Batch summaries if > threshold

**Acceptance Criteria**:
- [ ] `reduce_phase` aggregates summaries
- [ ] Hierarchical extraction for many summaries
- [ ] Final commit message synthesized
- [ ] Validation checks format
- [ ] Salvage extracts valid commit from malformed output
- [ ] Unit tests: aggregation, synthesis, salvage
- [ ] `cargo test reduce_phase` → PASS

**Commit**: YES
- Message: `feat(orchestrator): implement Reduce phase for message synthesis`
- Files: `christina/src/orchestrator.rs`

---

### Task 13: Implement GPG Signing Support

**What to do**:
- Modify `christina/src/io/git/adapter.rs`
- Add `create_signed_commit` function
- Detect gpg.program from git config
- Create commit buffer and sign with external gpg
- Handle GPG errors gracefully

**Must NOT do**:
- Don't use gpgme library (use external gpg CLI)
- Don't fail silently on GPG errors

**Recommended Agent Profile**:
- **Category**: `unspecified-high` (system integration)
- **Skills**: `rust`, `git`, `gpg`, `process-management`

**Parallelization**:
- **Can Run In Parallel**: YES (independent)
- **Parallel Group**: Wave 5
- **Blocks**: Task 15
- **Blocked By**: None

**References**:
- old_backup/christina-git/src/repository.rs - GPG signing
- Pattern: commit_create_buffer → gpg sign → commit_signed
- Fallback: Unsigned commit if GPG fails

**Acceptance Criteria**:
- [ ] `create_signed_commit` function exists
- [ ] Reads gpg.program from git config
- [ ] Creates commit buffer via git2
- [ ] Signs buffer with external gpg CLI
- [ ] Creates signed commit via git2
- [ ] Falls back to unsigned on GPG error
- [ ] Unit tests: signing detection, fallback
- [ ] `cargo test gpg` → PASS

**Commit**: YES
- Message: `feat(git): implement GPG signing support`
- Files: `christina/src/io/git/adapter.rs`

---

### Task 14: Enhance Git Adapter with Unborn Branch Handling

**What to do**:
- Review and enhance unborn branch handling in adapter.rs
- Ensure proper HEAD initialization
- Add tests for unborn branch scenarios

**Must NOT do**:
- Don't break existing branch handling
- Don't assume main branch name

**Recommended Agent Profile**:
- **Category**: `quick` (enhancement)
- **Skills**: `rust`, `git`

**Parallelization**:
- **Can Run In Parallel**: YES
- **Parallel Group**: Wave 5
- **Blocks**: Task 15
- **Blocked By**: None

**Acceptance Criteria**:
- [ ] Unborn branch detection works
- [ ] First commit creates proper HEAD
- [ ] No panic on empty repo
- [ ] Unit tests: unborn branch scenarios
- [ ] `cargo test unborn` → PASS

**Commit**: YES
- Message: `fix(git): enhance unborn branch handling`
- Files: `christina/src/io/git/adapter.rs`

---

### Task 15: Integrate AIOrchestrator with generate.rs

**What to do**:
- Replace stub implementation in `christina/src/generate.rs`
- Integrate DiffProcessor for chunking
- Integrate AIOrchestrator for generation
- Remove TODO comments
- Update progress reporting

**Must NOT do**:
- Don't change public API signature
- Don't remove existing error handling

**Recommended Agent Profile**:
- **Category**: `ultrabrain` (integration)
- **Skills**: `rust`, `architecture`, `refactoring`

**Parallelization**:
- **Can Run In Parallel**: NO (depends on Waves 1-5)
- **Parallel Group**: Wave 6
- **Blocks**: Task 16, 17, 18
- **Blocked By**: Tasks 1-14

**Acceptance Criteria**:
- [ ] generate.rs uses DiffProcessor for chunking
- [ ] generate.rs uses AIOrchestrator for generation
- [ ] All TODOs removed
- [ ] Progress events properly reported
- [ ] Small diffs use direct generation
- [ ] Large diffs use Map-Reduce
- [ ] `cargo test generate` → PASS
- [ ] `cargo check` → zero warnings

**Commit**: YES
- Message: `feat(generate): integrate AIOrchestrator and DiffProcessor`
- Files: `christina/src/generate.rs`

---

### Task 16: Integrate DiffProcessor with Event Loop

**What to do**:
- Update event loop to use DiffProcessor
- Add TokenCountUpdate events during chunking
- Update progress stages to reflect chunking

**Must NOT do**:
- Don't block event loop during chunking
- Don't lose existing event handling

**Recommended Agent Profile**:
- **Category**: `unspecified-high` (integration)
- **Skills**: `rust`, `async`, `event-driven`

**Parallelization**:
- **Can Run In Parallel**: NO (depends on Task 4, 15)
- **Parallel Group**: Wave 6
- **Blocks**: Task 18
- **Blocked By**: Task 4, 15

**Acceptance Criteria**:
- [ ] Event loop uses DiffProcessor
- [ ] TokenCountUpdate events sent
- [ ] Progress shows chunking stage
- [ ] Non-blocking chunking (spawn_blocking if needed)
- [ ] `cargo test event_loop` → PASS

**Commit**: YES
- Message: `feat(event_loop): integrate DiffProcessor with token count reporting`
- Files: `christina/src/event_loop/mod.rs`

---

### Task 17: Add Comprehensive Progress Event Reporting

**What to do**:
- Enhance progress reporting in AIOrchestrator
- Add stages: chunking, mapping, intent extraction, reducing
- Update GeneratingState to show detailed progress

**Must NOT do**:
- Don't spam events (rate limit updates)
- Don't show internal details

**Recommended Agent Profile**:
- **Category**: `visual-engineering` (UX)
- **Skills**: `rust`, `ui`, `ux`

**Parallelization**:
- **Can Run In Parallel**: NO (depends on Task 10, 15)
- **Parallel Group**: Wave 6
- **Blocks**: Task 18
- **Blocked By**: Task 10, 15

**Acceptance Criteria**:
- [ ] Progress shows: "Analyzing diff...", "Chunking...", "Summarizing...", "Synthesizing..."
- [ ] Progress includes chunk count (e.g., "Summarizing chunk 3/10...")
- [ ] Spinner updates smoothly
- [ ] `cargo test progress` → PASS

**Commit**: YES
- Message: `feat(ui): add detailed progress reporting for generation stages`
- Files: `christina/src/orchestrator.rs`, `christina/src/tui/screens/generating.rs`

---

### Task 18: Write Integration Tests

**What to do**:
- Create `christina/tests/integration_test.rs`
- Test full generation flow with mock LLM
- Test retry behavior
- Test partial failure handling
- Test GPG signing (mock)

**Must NOT do**:
- Don't depend on external services
- Don't use real API keys

**Recommended Agent Profile**:
- **Category**: `ultrabrain` (testing)
- **Skills**: `rust`, `testing`, `mocking`

**Parallelization**:
- **Can Run In Parallel**: NO (depends on Waves 1-6)
- **Parallel Group**: Wave 7
- **Blocks**: Task 19, 20
- **Blocked By**: Tasks 15-17

**Acceptance Criteria**:
- [ ] Integration test for direct generation
- [ ] Integration test for Map-Reduce generation
- [ ] Integration test for retry behavior
- [ ] Integration test for partial failure
- [ ] All tests pass without external services
- [ ] `cargo test integration` → PASS

**Commit**: YES
- Message: `test(integration): add comprehensive integration tests`
- Files: `christina/tests/integration_test.rs`

---

### Task 19: Write Documentation

**What to do**:
- Add module-level documentation for all new modules
- Add doc comments for all public APIs
- Update README with architecture overview
- Document configuration options

**Must NOT do**:
- Don't document private functions
- Don't duplicate code comments

**Recommended Agent Profile**:
- **Category**: `writing` (documentation)
- **Skills**: `technical-writing`, `rust`

**Parallelization**:
- **Can Run In Parallel**: NO (depends on Task 18)
- **Parallel Group**: Wave 7
- **Blocks**: Task 20
- **Blocked By**: Task 18

**Acceptance Criteria**:
- [ ] All new modules have module docs
- [ ] All public APIs have doc comments
- [ ] README updated with architecture
- [ ] `cargo doc` generates without warnings
- [ ] Documentation tests pass

**Commit**: YES
- Message: `docs: add comprehensive documentation for new modules`
- Files: All new .rs files, README.md

---

### Task 20: Final Quality Gate Verification

**What to do**:
- Run full test suite
- Run clippy with all features
- Check for any remaining TODOs
- Verify zero warnings
- Create final summary

**Must NOT do**:
- Don't skip any quality gates
- Don't ignore warnings

**Recommended Agent Profile**:
- **Category**: `quick` (verification)
- **Skills**: `rust`, `ci-cd`

**Parallelization**:
- **Can Run In Parallel**: NO (depends on Task 18, 19)
- **Parallel Group**: Wave 7
- **Blocks**: None (final task)
- **Blocked By**: Task 18, 19

**Acceptance Criteria**:
- [ ] `cargo test` → ALL PASS
- [ ] `cargo clippy` → ZERO WARNINGS
- [ ] `cargo check` → ZERO WARNINGS
- [ ] No TODO/FIXME comments remaining
- [ ] All files formatted (`cargo fmt`)
- [ ] Documentation complete
- [ ] Final report generated

**Commit**: YES
- Message: `chore: final quality gate verification and cleanup`
- Files: Any remaining cleanup

---

## Commit Strategy

| After Task | Message | Files | Verification |
|------------|---------|-------|--------------|
| 1 | `feat(retry): implement RetryPolicy with exponential backoff` | retry.rs | `cargo test retry` |
| 2 | `feat(concurrency): implement RequestLimiter` | concurrency.rs | `cargo test concurrency` |
| 3 | `feat(git): implement diff parsing utilities` | parsing.rs | `cargo test parsing` |
| 4 | `feat(git): implement DiffProcessor` | diff_processor.rs | `cargo test diff_processor` |
| 5 | `feat(git): implement token-aware diff chunking` | chunking.rs | `cargo test chunking` |
| 6 | `feat(error): add IsTransient trait` | error.rs | `cargo test error` |
| 7 | `feat(llm): add retry to OpenAI provider` | openai.rs | `cargo test openai` |
| 8 | `feat(llm): add retry to Azure provider` | azure.rs | `cargo test azure` |
| 9 | `feat(llm): wire Groq provider` | provider_kind.rs, groq.rs | `cargo test groq` |
| 10 | `feat(orchestrator): implement AIOrchestrator core` | orchestrator.rs | `cargo test orchestrator` |
| 11 | `feat(orchestrator): implement Map phase` | orchestrator.rs | `cargo test map_phase` |
| 12 | `feat(orchestrator): implement Reduce phase` | orchestrator.rs | `cargo test reduce_phase` |
| 13 | `feat(git): implement GPG signing support` | adapter.rs | `cargo test gpg` |
| 14 | `fix(git): enhance unborn branch handling` | adapter.rs | `cargo test unborn` |
| 15 | `feat(generate): integrate AIOrchestrator` | generate.rs | `cargo test generate` |
| 16 | `feat(event_loop): integrate DiffProcessor` | event_loop/mod.rs | `cargo test event_loop` |
| 17 | `feat(ui): add detailed progress reporting` | orchestrator.rs, generating.rs | `cargo test progress` |
| 18 | `test(integration): add integration tests` | tests/integration_test.rs | `cargo test integration` |
| 19 | `docs: add comprehensive documentation` | All new files | `cargo doc` |
| 20 | `chore: final quality gate verification` | Cleanup | Full test suite |

---

## Risk Assessment

### High Risk

| Risk | Impact | Mitigation |
|------|--------|------------|
| Map-Reduce complexity | High | Break into Map and Reduce tasks, extensive testing |
| GPG signing edge cases | Medium | Test with/without GPG, fallback to unsigned |
| Async cancellation | Medium | Use AbortOnDrop pattern, test cancellation |
| Token counting accuracy | High | Validate against tiktoken, test edge cases |

### Medium Risk

| Risk | Impact | Mitigation |
|------|--------|------------|
| LLM provider rate limits | Medium | Implement RequestLimiter, test throttling |
| Diff chunking performance | Medium | Use buffer pool, benchmark large diffs |
| Partial failure handling | Medium | Clear thresholds, user confirmation |

### Low Risk

| Risk | Impact | Mitigation |
|------|--------|------------|
| Groq provider integration | Low | Simple enum variant addition |
| Documentation completeness | Low | Review checklist, peer review |

---

## Success Criteria

### Verification Commands

```bash
# Full test suite
cargo test

# Quality gates
just check
just clippy

# Documentation
cargo doc --no-deps

# Integration tests only
cargo test integration
```

### Final Checklist

- [ ] All 20 tasks complete
- [ ] All TODOs removed from generate.rs
- [ ] All stubs implemented
- [ ] AIOrchestrator fully implements Map-Reduce
- [ ] Git operations support GPG signing
- [ ] LLM providers have retry/rate limiting
- [ ] Diff chunking fully functional
- [ ] TUI screens fully wired
- [ ] Event loop complete
- [ ] All tests pass
- [ ] Cargo check + clippy clean (ZERO warnings)
- [ ] Documentation complete
- [ ] No behavioral gaps vs old_backup

---

## Appendix: Reference Files

### old_backup Behavioral Reference

Key files for behavioral reference (extract patterns, not code):

- `old_backup/christina-llm/src/orchestrator.rs` - Map-Reduce pipeline
- `old_backup/christina-llm/src/retry.rs` - RetryPolicy implementation
- `old_backup/christina-llm/src/concurrency.rs` - RequestLimiter
- `old_backup/christina-git/src/diff_processor.rs` - Diff processing
- `old_backup/christina-git/src/chunking.rs` - Diff chunking algorithm
- `old_backup/christina-git/src/parsing.rs` - Diff parsing
- `old_backup/christina-git/src/repository.rs` - GPG signing

### Current Implementation Reference

- `christina/src/generate.rs` - Stub to replace
- `christina/src/io/git/adapter.rs` - Git operations
- `christina/src/io/llm/openai.rs` - OpenAI provider
- `christina/src/io/llm/azure.rs` - Azure provider
- `christina/src/event_loop/mod.rs` - Event loop
- `christina-core/src/error.rs` - Error types
- `christina-core/src/git/diff.rs` - Diff types
- `christina-core/src/git/file.rs` - Git file types

---

## Next Steps

1. Review this plan for completeness
2. Decide on high accuracy mode (Momus review)
3. Run `/start-work` to begin execution
4. Monitor progress through waves
5. Verify each task's acceptance criteria
