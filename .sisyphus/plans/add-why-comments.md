# Add High-Quality WHY Comments to Rust Codebase

## TL;DR

> **Quick Summary**: Add intentional, WHY-focused comments to the Christina codebase that explain design decisions, invariants, constraints, and edge cases. Focus on code that cannot speak for itself.
>
> **Deliverables**:
> - Documented error handling invariants (`christina-core/src/error.rs`)
> - Documented type system constraints (`christina-core/src/types/`)
> - Documented algorithmic decisions (`chunking.rs`, `orchestrator.rs`)
> - Documented concurrency design (`concurrency.rs`, `retry.rs`)
> - Documented performance optimizations (`buffer_pool.rs`)
> - Documented state machine transitions (`state.rs`)
>
> **Estimated Effort**: Medium (6-8 hours)
> **Parallel Execution**: YES - 5 waves
> **Critical Path**: Core Types → Error Handling → Algorithms → Concurrency → Integration

---

## Context

### Original Request
Add high-quality, intentional comments to the Rust codebase that explain WHY the code exists and behaves the way it does—not WHAT it does. Comment only where the code cannot speak for itself: docs comments, non-obvious decisions, invariants, constraints, edge cases, sharp edges, and cross-cutting rationale, with a bias toward clarity over density.

### Codebase Overview
- **Language**: Rust (edition 2024)
- **Workspace**: Two crates - `christina` (binary) and `christina-core` (library)
- **Purpose**: TUI tool for generating conventional commit messages using LLMs
- **Quality Gates**: `just check` and `just clippy` with ZERO warnings
- **Lint Policy**: All warnings treated as errors, unsafe_code denied

### Existing Documentation Patterns
The codebase has minimal documentation with some existing WHY comments:
- Panic safety rationale (main.rs lines 74-75)
- Saturation arithmetic explanation (state.rs lines 48-51)
- Buffer pool limits (buffer_pool.rs line 73-74)
- Token bucket algorithm choice (concurrency.rs lines 14-18)
- Full jitter rationale (concurrency.rs lines 86-91)

### Comment Quality Criteria
Per AGENTS.md and request:
1. **Explain WHY, not WHAT**: Assume reader knows Rust
2. **Non-obvious decisions**: Algorithm choices, magic numbers, tradeoffs
3. **Invariants**: Type system invariants, safety requirements
4. **Constraints**: Performance constraints, API constraints
5. **Edge cases**: Why specific edge case handling exists
6. **Cross-cutting concerns**: How pieces interact, architectural rationale

---

## Work Objectives

### Core Objective
Add high-quality WHY comments to critical areas of the codebase, focusing on design decisions, invariants, constraints, and edge cases that are not self-evident from the code.

### Concrete Deliverables
- Documented error types with transient/non-transient rationale
- Documented type invariants (FilePath, TokenCount, GenerationId)
- Documented chunking algorithm decisions
- Documented MapReduce orchestration strategy
- Documented rate limiting and retry policies
- Documented state machine transitions
- Documented buffer pooling strategy

### Definition of Done
- [ ] All critical files have module-level documentation (`//!`)
- [ ] All public APIs have doc comments (`///`) explaining purpose and invariants
- [ ] All magic numbers have explanatory comments
- [ ] All complex algorithms have rationale comments
- [ ] All edge case handling has context comments
- [ ] `just check` passes with zero warnings
- [ ] `just clippy` passes with zero warnings

### Must Have
- Comments explaining WHY, not WHAT
- Focus on non-obvious decisions
- Invariant documentation for types
- Algorithm rationale
- Performance decision explanations

### Must NOT Have (Guardrails)
- NO "what" comments that merely restate code
- NO comments on simple getters/setters
- NO comments on trait implementations (From, Display, etc.)
- NO comments on test code
- NO comments on CLI parsing (clap is declarative)
- NO doc comments on private functions unless complex

---

## Verification Strategy

### Test Decision
- **Infrastructure exists**: YES (cargo test, nextest)
- **User wants tests**: NO - This is documentation work
- **Framework**: N/A

### Automated Verification
Each task includes verification via:
- `just check` - compilation check
- `just clippy` - lint check
- `cargo doc --no-deps` - doc generation check

### Manual Verification
- Review comments for WHY focus
- Ensure no WHAT-style comments added
- Verify comments add value beyond code readability

---

## Task Dependency Graph

| Task | Depends On | Reason |
|------|------------|--------|
| Task 1: Core Types | None | Foundational types used throughout |
| Task 2: Error Handling | Task 1 | Error types reference core types |
| Task 3: Git I/O - Buffer Pool | Task 1 | Uses FilePath from types |
| Task 4: Git I/O - Chunking | Task 1, Task 3 | Uses types and buffer pool |
| Task 5: LLM I/O - Concurrency | None | Independent module |
| Task 6: LLM I/O - Retry | Task 5 | Retry uses concurrency primitives |
| Task 7: LLM I/O - Orchestrator | Task 2, Task 4, Task 5, Task 6 | Uses all lower-level modules |
| Task 8: State Management | Task 1 | Uses GenerationId concept |
| Task 9: Application Layer | Task 8 | Uses state management |

---

## Parallel Execution Graph

```
Wave 1 (Start immediately):
├── Task 1: Core Types Documentation
│   └── christina-core/src/types/file_path.rs
│   └── christina-core/src/types/token_count.rs
│   └── christina-core/src/types/model_name.rs
│   └── christina-core/src/types/commit_message.rs
│   └── christina-core/src/types/provider_kind.rs
│   └── christina-core/src/types/mod.rs
└── Task 5: LLM Concurrency Documentation
    └── christina/src/io/llm/concurrency.rs

Wave 2 (After Wave 1):
├── Task 2: Error Handling Documentation
│   └── christina-core/src/error.rs
└── Task 3: Git Buffer Pool Documentation
    └── christina/src/io/git/buffer_pool.rs

Wave 3 (After Wave 2):
├── Task 4: Git Chunking Documentation
│   └── christina/src/io/git/chunking.rs
└── Task 6: LLM Retry Documentation
    └── christina/src/io/llm/retry.rs

Wave 4 (After Wave 3):
├── Task 7: LLM Orchestrator Documentation
│   └── christina/src/io/llm/orchestrator.rs
└── Task 8: State Management Documentation
    └── christina-core/src/state.rs
    └── christina/src/app/state.rs

Wave 5 (After Wave 4):
└── Task 9: Application Layer Documentation
    └── christina/src/main.rs
    └── christina/src/app/mod.rs
    └── christina/src/app/context.rs
    └── christina/src/app/init.rs

Critical Path: Task 1 → Task 2 → Task 4 → Task 7
Estimated Parallel Speedup: ~50% faster than sequential
```

---

## Tasks

### Task 1: Core Types Documentation

**Description**: Add WHY comments to core type definitions explaining invariants, design decisions, and performance choices.

**Files**:
- `christina-core/src/types/file_path.rs` - FilePath newtype with CompactString
- `christina-core/src/types/token_count.rs` - TokenCount newtype
- `christina-core/src/types/model_name.rs` - ModelName type
- `christina-core/src/types/commit_message.rs` - CommitMessage validation
- `christina-core/src/types/provider_kind.rs` - ProviderKind enum
- `christina-core/src/types/mod.rs` - Module exports

**Key Comments to Add**:
- FilePath: WHY CompactString vs String/Arc<str>, WHY relative paths only
- TokenCount: WHY saturating arithmetic, token counting invariants
- CommitMessage: Validation invariants, Conventional Commits rationale

**Must NOT do**:
- Don't comment simple getter methods
- Don't comment From/Display implementations
- Don't add module docs if already present

**Delegation Recommendation**:
- **Category**: `unspecified-high` - Requires understanding Rust type system and invariants
- **Skills**: `rust-programmer` (if available), otherwise general programming
- **Reason**: High effort task requiring deep understanding of type design

**Skills Evaluation**:
- INCLUDED: Domain knowledge of Rust newtypes and invariants
- OMITTED: frontend-ui-ux (not UI work), git-master (no git ops)

**Parallelization**:
- **Can Run In Parallel**: YES
- **Parallel Group**: Wave 1
- **Blocks**: Task 2, Task 3, Task 4, Task 8
- **Blocked By**: None

**Acceptance Criteria**:
- [ ] FilePath has comments explaining CompactString choice and relative path invariant
- [ ] TokenCount has comments explaining saturating arithmetic
- [ ] CommitMessage has comments explaining validation strategy
- [ ] All types have module-level documentation
- [ ] `just check` passes
- [ ] `just clippy` passes

**Commit**: YES
- Message: `docs(core): add WHY comments to core types`
- Files: `christina-core/src/types/*.rs`
- Pre-commit: `just check && just clippy`

---

### Task 2: Error Handling Documentation

**Description**: Document error type design, transient error classification, and recovery strategies.

**Files**:
- `christina-core/src/error.rs` - All error types and IsTransient trait

**Key Comments to Add**:
- IsTransient trait: WHY this abstraction exists
- GitError: WHY only Locked is transient
- CompletionError: WHY certain errors are transient vs permanent
- Error categorization rationale
- from_api_error parsing strategy

**Must NOT do**:
- Don't comment individual error variant definitions
- Don't add docs to test code

**Delegation Recommendation**:
- **Category**: `unspecified-high` - Complex error handling patterns
- **Skills**: `rust-programmer`
- **Reason**: Requires understanding error handling philosophy

**Parallelization**:
- **Can Run In Parallel**: NO (depends on Task 1)
- **Parallel Group**: Wave 2
- **Blocks**: Task 7
- **Blocked By**: Task 1

**Acceptance Criteria**:
- [ ] IsTransient trait has rationale comment
- [ ] GitError::is_transient explains WHY only Locked is transient
- [ ] CompletionError::is_transient explains classification
- [ ] from_api_error explains parsing strategy
- [ ] `just check` passes
- [ ] `just clippy` passes

**Commit**: YES
- Message: `docs(core): document error handling invariants`
- Files: `christina-core/src/error.rs`
- Pre-commit: `just check && just clippy`

---

### Task 3: Git Buffer Pool Documentation

**Description**: Document buffer pooling strategy, capacity choices, and thread-local design.

**Files**:
- `christina/src/io/git/buffer_pool.rs`

**Key Comments to Add**:
- WHY 4KB pre-allocation (line 15)
- WHY 16 buffer limit (line 74)
- WHY thread-local vs global pool
- ChunkBuffer design rationale

**Must NOT do**:
- Don't comment simple clear/push operations
- Don't comment test assertions

**Delegation Recommendation**:
- **Category**: `unspecified-low` - Smaller, focused module
- **Skills**: `rust-programmer`
- **Reason**: Moderate effort, focused scope

**Parallelization**:
- **Can Run In Parallel**: NO (depends on Task 1)
- **Parallel Group**: Wave 2
- **Blocks**: Task 4
- **Blocked By**: Task 1

**Acceptance Criteria**:
- [ ] 4KB capacity has explanatory comment
- [ ] 16 buffer limit has explanatory comment
- [ ] Thread-local design rationale documented
- [ ] `just check` passes
- [ ] `just clippy` passes

**Commit**: YES
- Message: `docs(git): document buffer pooling strategy`
- Files: `christina/src/io/git/buffer_pool.rs`
- Pre-commit: `just check && just clippy`

---

### Task 4: Git Chunking Documentation

**Description**: Document the greedy packing algorithm, token limit decisions, and splitting strategies.

**Files**:
- `christina/src/io/git/chunking.rs`

**Key Comments to Add**:
- WHY lockfile token limit = 100 (line 11)
- WHY 80% retention threshold (line 166)
- WHY binary search for oversized lines (lines 408-440)
- Greedy packing algorithm rationale
- Splitting strategy hierarchy (files → hunks → lines)

**Must NOT do**:
- Don't comment obvious loop iterations
- Don't comment simple string operations

**Delegation Recommendation**:
- **Category**: `unspecified-high` - Complex algorithm documentation
- **Skills**: `rust-programmer`
- **Reason**: Requires understanding algorithmic tradeoffs

**Parallelization**:
- **Can Run In Parallel**: NO (depends on Task 1, Task 3)
- **Parallel Group**: Wave 3
- **Blocks**: Task 7
- **Blocked By**: Task 1, Task 3

**Acceptance Criteria**:
- [ ] LOCKFILE_TOKEN_LIMIT has rationale
- [ ] 80% threshold explained
- [ ] Binary search strategy explained
- [ ] Algorithm hierarchy documented
- [ ] `just check` passes
- [ ] `just clippy` passes

**Commit**: YES
- Message: `docs(git): document chunking algorithm decisions`
- Files: `christina/src/io/git/chunking.rs`
- Pre-commit: `just check && just clippy`

---

### Task 5: LLM Concurrency Documentation

**Description**: Document rate limiting design, token bucket algorithm, and jitter strategy.

**Files**:
- `christina/src/io/llm/concurrency.rs`

**Key Comments to Add**:
- WHY token bucket + semaphore combination (lines 14-18)
- WHY capacity = requests_per_second * 2 (line 106)
- WHY full jitter vs additive/multiplicative (lines 86-91)
- WHY min_delay separate from token bucket (lines 38-41)
- Loop-sleep pattern vs condition variable

**Must NOT do**:
- Don't comment obvious mutex operations
- Don't comment simple duration calculations

**Delegation Recommendation**:
- **Category**: `unspecified-high` - Complex concurrency patterns
- **Skills**: `rust-programmer`
- **Reason**: Requires understanding rate limiting and concurrency

**Parallelization**:
- **Can Run In Parallel**: YES
- **Parallel Group**: Wave 1
- **Blocks**: Task 6, Task 7
- **Blocked By**: None

**Acceptance Criteria**:
- [ ] Token bucket + semaphore rationale documented
- [ ] Capacity multiplier explained
- [ ] Full jitter choice explained
- [ ] min_delay design explained
- [ ] `just check` passes
- [ ] `just clippy` passes

**Commit**: YES
- Message: `docs(llm): document rate limiting design`
- Files: `christina/src/io/llm/concurrency.rs`
- Pre-commit: `just check && just clippy`

---

### Task 6: LLM Retry Documentation

**Description**: Document retry policy, exponential backoff, and jitter implementation.

**Files**:
- `christina/src/io/llm/retry.rs`

**Key Comments to Add**:
- WHY seed-based randomization vs pure random (lines 102-116)
- Exponential backoff rationale
- Full jitter vs other strategies
- Retry policy defaults rationale

**Must NOT do**:
- Don't comment simple math operations
- Don't comment obvious loop logic

**Delegation Recommendation**:
- **Category**: `unspecified-low` - Smaller module
- **Skills**: `rust-programmer`
- **Reason**: Moderate effort, focused scope

**Parallelization**:
- **Can Run In Parallel**: NO (depends on Task 5)
- **Parallel Group**: Wave 3
- **Blocks**: Task 7
- **Blocked By**: Task 5

**Acceptance Criteria**:
- [ ] Seed-based randomization explained
- [ ] Exponential backoff rationale documented
- [ ] Jitter strategy explained
- [ ] Default values rationale
- [ ] `just check` passes
- [ ] `just clippy` passes

**Commit**: YES
- Message: `docs(llm): document retry strategy`
- Files: `christina/src/io/llm/retry.rs`
- Pre-commit: `just check && just clippy`

---

### Task 7: LLM Orchestrator Documentation

**Description**: Document MapReduce pipeline, intent extraction, and failure handling strategies.

**Files**:
- `christina/src/io/llm/orchestrator.rs`

**Key Comments to Add**:
- WHY hierarchical intent extraction at 20 summaries (line 25)
- WHY 15% history budget (line 177)
- WHY 10% max failure rate (line 952)
- WHY timeout escalation pattern (lines 21-23, 962-968)
- WHY buffer_unordered not buffer (line 379)
- WHY dynamic concurrency calculation (lines 934-941)
- WHY salvage vs fail hard (lines 393-401, 420-440)
- WHY contradiction detection (lines 695-721)

**Must NOT do**:
- Don't comment obvious struct fields
- Don't comment simple async/await patterns

**Delegation Recommendation**:
- **Category**: `ultrabrain` - Most complex module, requires deep understanding
- **Skills**: `rust-programmer`
- **Reason**: Complex MapReduce algorithm, many design decisions

**Parallelization**:
- **Can Run In Parallel**: NO (depends on Task 2, Task 4, Task 5, Task 6)
- **Parallel Group**: Wave 4
- **Blocks**: None (final integration)
- **Blocked By**: Task 2, Task 4, Task 5, Task 6

**Acceptance Criteria**:
- [ ] MAX_SUMMARIES_PER_INTENT_BATCH explained
- [ ] History budget calculation explained
- [ ] Max failure rate rationale documented
- [ ] Timeout escalation explained
- [ ] Concurrency strategy explained
- [ ] Salvage strategy explained
- [ ] Contradiction detection explained
- [ ] `just check` passes
- [ ] `just clippy` passes

**Commit**: YES
- Message: `docs(llm): document orchestration pipeline`
- Files: `christina/src/io/llm/orchestrator.rs`
- Pre-commit: `just check && just clippy`

---

### Task 8: State Management Documentation

**Description**: Document state machine transitions, generation ID handling, and abort-on-drop pattern.

**Files**:
- `christina-core/src/state.rs` - Core state machine
- `christina/src/app/state.rs` - TUI state management

**Key Comments to Add**:
- WHY saturating add for generation ID (lines 48-51 in state.rs)
- WHY same-state transitions forbidden (lines 64-71)
- WHY specific transition graph
- AbortOnDrop rationale (lines 5-12 in app/state.rs)
- GenerationState design

**Must NOT do**:
- Don't comment obvious enum variants
- Don't comment simple Display implementations

**Delegation Recommendation**:
- **Category**: `unspecified-low` - Moderate complexity
- **Skills**: `rust-programmer`
- **Reason**: State machine logic, moderate effort

**Parallelization**:
- **Can Run In Parallel**: NO (depends on Task 1)
- **Parallel Group**: Wave 4
- **Blocks**: Task 9
- **Blocked By**: Task 1

**Acceptance Criteria**:
- [ ] Saturating add rationale documented
- [ ] Same-state transition rule explained
- [ ] Transition graph rationale documented
- [ ] AbortOnDrop pattern explained
- [ ] `just check` passes
- [ ] `just clippy` passes

**Commit**: YES
- Message: `docs(state): document state machine design`
- Files: `christina-core/src/state.rs`, `christina/src/app/state.rs`
- Pre-commit: `just check && just clippy`

---

### Task 9: Application Layer Documentation

**Description**: Document main entry point, panic recovery, and application initialization.

**Files**:
- `christina/src/main.rs` - Entry point and panic recovery
- `christina/src/app/mod.rs` - App module
- `christina/src/app/context.rs` - App context
- `christina/src/app/init.rs` - Initialization

**Key Comments to Add**:
- WHY AssertUnwindSafe for panic recovery (lines 74-75 in main.rs)
- WHY MiMalloc with Cap (lines 17-19)
- WHY dhat-heap feature gating
- Application initialization flow

**Must NOT do**:
- Don't comment obvious CLI parsing
- Don't comment simple module declarations

**Delegation Recommendation**:
- **Category**: `unspecified-low` - Entry point documentation
- **Skills**: `rust-programmer`
- **Reason**: Straightforward, focused on safety comments

**Parallelization**:
- **Can Run In Parallel**: NO (depends on Task 8)
- **Parallel Group**: Wave 5
- **Blocks**: None
- **Blocked By**: Task 8

**Acceptance Criteria**:
- [ ] AssertUnwindSafe rationale expanded
- [ ] Global allocator choice explained
- [ ] dhat-heap feature explained
- [ ] Initialization flow documented
- [ ] `just check` passes
- [ ] `just clippy` passes

**Commit**: YES
- Message: `docs(app): document application entry and safety`
- Files: `christina/src/main.rs`, `christina/src/app/*.rs`
- Pre-commit: `just check && just clippy`

---

## Commit Strategy

| After Task | Message | Files | Verification |
|------------|---------|-------|--------------|
| Task 1 | `docs(core): add WHY comments to core types` | `christina-core/src/types/*.rs` | `just check && just clippy` |
| Task 2 | `docs(core): document error handling invariants` | `christina-core/src/error.rs` | `just check && just clippy` |
| Task 3 | `docs(git): document buffer pooling strategy` | `christina/src/io/git/buffer_pool.rs` | `just check && just clippy` |
| Task 4 | `docs(git): document chunking algorithm decisions` | `christina/src/io/git/chunking.rs` | `just check && just clippy` |
| Task 5 | `docs(llm): document rate limiting design` | `christina/src/io/llm/concurrency.rs` | `just check && just clippy` |
| Task 6 | `docs(llm): document retry strategy` | `christina/src/io/llm/retry.rs` | `just check && just clippy` |
| Task 7 | `docs(llm): document orchestration pipeline` | `christina/src/io/llm/orchestrator.rs` | `just check && just clippy` |
| Task 8 | `docs(state): document state machine design` | `christina-core/src/state.rs`, `christina/src/app/state.rs` | `just check && just clippy` |
| Task 9 | `docs(app): document application entry and safety` | `christina/src/main.rs`, `christina/src/app/*.rs` | `just check && just clippy` |

---

## Success Criteria

### Verification Commands
```bash
# All checks must pass
just check
just clippy

# Documentation should build without warnings
cargo doc --no-deps --workspace

# All tests should still pass
just test
```

### Final Checklist
- [ ] All "Must Have" comments present
- [ ] All "Must NOT Have" exclusions respected
- [ ] All quality gates pass
- [ ] No WHAT-style comments added
- [ ] All magic numbers explained
- [ ] All complex algorithms have rationale
- [ ] All type invariants documented
- [ ] Module-level docs added where missing

---

## Notes for Implementers

### Comment Style Guide
1. **Use `//!` for module-level docs** - Explain the module's purpose and design
2. **Use `///` for public items** - Explain invariants, edge cases, usage
3. **Use `//` for inline WHY comments** - Explain specific lines/sections
4. **Focus on intent, not mechanics** - "We use X because Y" not "This does X"

### Example Good Comments
```rust
// WHY: CompactString stores small strings inline (≤16 bytes), avoiding heap
// allocation for common file paths. FilePath is relative to ensure portability
// across different checkout directories.
pub struct FilePath(CompactString);

// WHY: Only Locked is transient because Git's index.lock is released when the
// other process completes. Other errors (Auth, NotFound) require user intervention.
fn is_transient(&self) -> bool {
    matches!(self, GitError::Locked)
}

// WHY: 20 summaries per batch balances context window limits with intent
// extraction quality. Larger batches dilute themes; smaller batches fragment
// related changes across multiple LLM calls.
const MAX_SUMMARIES_PER_INTENT_BATCH: usize = 20;
```

### Files to Skip Entirely
- Simple trait implementations (From, Display, Clone, etc.)
- Test modules (already self-explanatory via test names)
- CLI argument structs (clap handles this)
- Straightforward constructors
- Simple getters/setters
- TUI widget rendering code (visual, not algorithmic)
