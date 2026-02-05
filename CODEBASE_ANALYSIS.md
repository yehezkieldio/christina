## Critical Issues

### C-001: Quality Gate Failures in Clippy

**Finding**: Clippy fails with errors due to unused crate dependencies in benchmarks and lib.rs, plus unused imports in test code.
**Status**: Resolved
**Resolution**: Added placeholder uses for unused dependencies with appropriate conditional compilation. Fixed unused imports in error.rs. Added #![allow] attributes for test code and benchmarks to permit unwrap/expect/panic. All clippy checks now pass with zero warnings.

**Assumptions**: Benchmark files do not re-export library dependencies; lib.rs exposes only `io` module.

---

### C-002: TokenCount Allows Zero via new_saturating

**Finding**: TokenCount::new_saturating(0) returns TokenCount(NonZeroU32::MIN) which equals 1. The type claims to represent a non-zero count, but the saturation behavior silently transforms zero input into 1.
**Status**: Open
**Rationale**: Silent conversion hides bugs. Callers passing 0 may not realize they receive 1. This violates the "correct by construction" principle stated in AGENTS.md.

**Assumptions**: Callers expect new_saturating(0) to represent "no tokens" but actually get 1 token.

**Approach**:
1. Rename new_saturating to new_at_least_one with documentation stating the clamping behavior
2. Add a new_or_none method returning Option for cases where zero is meaningful
3. Audit all call sites of new_saturating(0) to verify intent—most appear in tests where 0 indicates empty, which should likely remain empty

---

### C-003: debug_assert for API Key Validation in config_to_profile

**Finding**: In generate.rs line 36-39, API key presence is validated with debug_assert, which is stripped in release builds.
**Status**: Resolved
**Resolution**: Replaced debug_assert with regular assert! that works in release builds. Updated test expectations to match new panic message. The invariant violation is now properly caught in all build configurations.

**Assumptions**: Callers should validate before calling, but the defensive check should remain in release.

---

## Structural Issues

### S-001: Duplicated Retry Logic Between Secret Resolution and Orchestrator

**Finding**: Secret::resolve() in secret.rs (lines 60-79) has inline retry logic with thread::sleep(500ms). The orchestrator uses RetryPolicy. Two different retry mechanisms.
**Status**: Open
**Rationale**: Inconsistent retry semantics. Secret retry is blocking (thread::sleep), while orchestrator retry is async. The secret retry has hardcoded 500ms delay with no exponential backoff.

**Assumptions**: Keyring transient failures should use the same retry policy as LLM requests.

**Approach**:
1. Extract retry logic from secret.rs into a shared utility
2. Convert secret resolution to async if it must retry, or use tokio::task::spawn_blocking
3. Apply RetryPolicy with jitter for keyring failures

---

### S-002: Profiles Type Uses Generic S Parameter with Default String

**Finding**: Profiles<S = String> and ProviderProfile<S = String> use generics for the secret type, but this creates complexity in serialization and the type erasure makes it hard to track what S actually is at runtime.
**Status**: Open
**Rationale**: The generic exists to support both SecretRef (disk) and SecretString (runtime), but creates friction. Types like ProviderProfile<SecretString> don't impl Serialize correctly because SecretString deliberately omits PartialEq comparison.

**Assumptions**: The pattern works but adds cognitive load and potential for misuse.

**Approach**:
1. Consider using an enum wrapper type instead of generics: enum ProfileSecret { Ref(SecretRef), Resolved(SecretString) }
2. Alternatively, keep the pattern but add exhaustive documentation in profile.rs explaining when to use which variant
3. Add compile-time assertions that Profiles<SecretRef> is Serializable and Profiles<SecretString> is not

---

### S-003: RepoSnapshot Stores Absolute PathBuf for repo_root

**Finding**: RepoSnapshot in snapshot.rs uses PathBuf for repo_root, which may be absolute.
**Status**: Resolved
**Resolution**: Created RepoRoot newtype that enforces absolute path invariants, mirroring FilePath's design for relative paths. Updated RepoSnapshot to use RepoRoot. Added comprehensive documentation and tests. Exported types from library.

**Assumptions**: repo_root must be absolute for git operations, while FilePath is relative within the repo.

---

## Edge Cases and Missing Validation

### E-001: AzureEndpoint Accepts Non-Standard Paths

**Finding**: AzureEndpoint::try_from in azure_endpoint.rs only validates that path contains "/openai/deployments/", but accepts any suffix after deployment_id.
**Status**: Open
**Rationale**: Non-standard Azure URLs (e.g., with typos like "/openai/deploymets/") will silently fail at the wrong validation point.

**Assumptions**: Azure URLs follow a predictable pattern.

**Approach**:
1. Add validation that the path follows the expected pattern: /openai/deployments/{id}/chat/completions
2. Emit a warning if the path deviates from expected structure

---

### E-002: FilePath::new Panics on Absolute Paths

**Finding**: FilePath::new uses assert! to reject absolute paths. This panics at construction.
**Status**: Open
**Rationale**: Per AGENTS.md, panics are for invariant violations. However, user-provided paths from diffs could include absolute paths, causing runtime panics.

**Assumptions**: Git diff output always uses relative paths.

**Approach**:
1. Verify that git diff output never produces absolute paths (it does not by spec)
2. Add integration test confirming parsing never produces absolute FilePath inputs
3. Consider using try_new at parsing boundaries and reserving new for internal construction

---

### E-003: Theme::scope is String, Can Be Empty or Invalid

**Finding**: In prompt.rs, Theme has pub scope: String with no validation. Empty scope is semantically "no scope" but stored as "".
**Status**: Open
**Rationale**: Scope rules in INTENT_EXTRACTION_PROMPT indicate scope can be null, but Theme stores empty string instead of Option<String>.

**Assumptions**: LLM may return null scope which becomes empty string.

**Approach**:
1. Change Theme.scope to Option<String> or a Scope newtype
2. Update PromptBuilder::build_synthesis_prompt to handle None scope correctly

---

### E-004: CommitMessage Regex Compiled Per Validation

**Finding**: In commit_message.rs line 97, Regex::new is called every time validate() is called.
**Status**: Resolved
**Resolution**: Replaced inline regex compilation with LazyLock static that compiles once on first use. Added #[allow(clippy::expect_used)] since the compile-time constant pattern should never fail.

**Assumptions**: Validation is called infrequently (once per generation).

---

## Brittleness and Hardening

### B-001: Binary Detection Relies on Content Scanning

**Finding**: DiffProcessor::is_binary_content scans for NUL bytes in the first 8KB, but large text files with NUL bytes beyond 8KB will be treated as text.
**Status**: Open
**Rationale**: The comment on line 54-56 acknowledges this limitation but offers no mitigation.

**Assumptions**: Text files rarely have NUL bytes. Binary files usually have them early.

**Approach**:
1. Document this behavior explicitly in user-facing docs
2. Add a config option for stricter binary detection if needed
3. Consider sampling the entire file at intervals (already done for >1MB files, extend to all)

---

### B-002: should_limit_file Uses ends_with on Patterns

**Finding**: In chunking.rs line 163, ignore patterns are matched via `path_str.ends_with(pattern)`. This means pattern "lock" matches "package-lock" but also "unlock.txt".
**Status**: Open
**Rationale**: Pattern matching is imprecise. Users expect glob-style matching.

**Assumptions**: Current patterns are full filenames like "Cargo.lock".

**Approach**:
1. Switch to glob pattern matching (the glob crate is available)
2. Or document that patterns are suffix matches, not glob patterns
3. Validate patterns at config load time

---

### B-003: Tokenizer Initialization Failure is Cached Forever

**Finding**: In tokenizer.rs, get_tokenizer uses OnceLock to cache the result, including errors. If tiktoken fails once, all subsequent calls fail.
**Status**: Open
**Rationale**: Transient initialization failures (e.g., temp file issues) become permanent.

**Assumptions**: Tiktoken initialization is deterministic and will always fail if it fails once.

**Approach**:
1. Verify tiktoken_rs::o200k_base is pure and deterministic
2. If transient failures are possible, use a retry on subsequent calls
3. Add explicit error message suggesting restart if initialization fails

---

### B-004: RequestLimiter Token Bucket Uses f64 for Capacity

**Finding**: TokenBucket in concurrency.rs uses f64 for tokens and capacity.
**Status**: Open
**Rationale**: Floating point accumulation over long periods can lead to precision issues, though unlikely to matter in practice.

**Assumptions**: Session duration is short (minutes, not days).

**Approach**:
1. Document that the limiter is designed for short sessions
2. Alternatively, use integer arithmetic with milli-token precision

---

### B-005: Fallback Line Truncation Can Produce Different Token Counts

**Finding**: truncate_to_token_limit in chunking.rs uses a fallback that counts tokens per line with newline appended. This can produce slightly different counts than the main path due to BPE boundary effects.
**Status**: Open
**Rationale**: The main path uses token decode; fallback uses line-by-line count. These may not agree.

**Assumptions**: The difference is small and acceptable.

**Approach**:
1. Add a test verifying fallback produces equivalent or fewer tokens than limit
2. Document the potential variance in comments

---

## Incomplete or Missing Features

### I-001: Event Enum Has Only Two Variants

**Finding**: events.rs defines Event with GenerationProgress and TokenCountUpdate only.
**Status**: Open
**Rationale**: The event system appears designed for extensibility but currently underutilized.

**Assumptions**: Future features (TUI, real-time status) will need more events.

**Approach**:
1. Add events for: ChunkProcessed, RetryAttempt, CommitCreated
2. Or remove the event system if progress channel is sufficient

---

### I-002: DiffConfig in settings.rs Referenced But Not Fully Used

**Finding**: Config contains pub diff: DiffConfig, and tests verify diff_tool and diff_show_preview, but there's no evide**Status**: Opennce these are consumed during diff processing.

**Rationale**: Configuration exists but may not be wired up.

**Assumptions**: DiffConfig was planned but not fully implemented.

**Approach**:
1. Trace DiffConfig usage through the codebase
2. Either wire it up to DiffProcessor or remove if unused
3. Add integration tests verifying diff options affect behavior

---

### I-003: GenerationId is a Simple u64 Wrapper

**Finding**: GenerationId in ids.rs is just a newtype over u64 with new() method.
**Status**: Open
**Rationale**: The ID is created via atomic counter in provider.rs but never used for correlation or logging.

**Assumptions**: IDs were intended for request tracking.

**Approach**:
1. Add tracing spans that include GenerationId
2. Use GenerationId in error messages for debugging
3. Or simplify if tracking is not needed

---

### I-004: test_helpers.rs Exists But Has Minimal Content

**Finding**: christina-core/src/test_helpers.rs is conditionally compiled but its contents are not visible in the analysis.
**Status**: Open
**Rationale**: Test helpers should provide utilities for creating test fixtures.

**Assumptions**: The module may be empty or minimal.

**Approach**:
1. Review test_helpers.rs content
2. Add builders for common test types: TestProfile, TestDiff, MockTokenizer
3. Ensure feature flag works correctly

---

## Performance Opportunities

### P-001: split_by_files Allocates New Strings for Each File

**Finding**: In parsing.rs, split_by_files creates Vec<FileDiff> where each FileDiff.content is a String clone of the slice.
**Status**: Open
**Rationale**: For large diffs, this copies megabytes of data.

**Assumptions**: Diff content must outlive the original diff string.

**Approach**:
1. Consider using Arc<str> or Cow<str> for FileDiff.content
2. Profile memory usage with large diffs to quantify impact

---

### P-002: TokenCache Uses ahash but Hashes Full Content

**Finding**: TokenizerService hashes the full text content for cache lookup.
**Status**: Open
**Rationale**: For large texts, hashing is O(n) even if cached.

**Assumptions**: Cache hits save more time than hash computation costs.

**Approach**:
1. Add instrumentation to measure cache hit rate
2. Consider content-length-based cache bypass for very large texts
3. Current skip for <50 bytes is good; consider upper bound too

---

### P-003: PromptBuilder Clones Strings Multiple Times

**Finding**: PromptBuilder methods like build_synthesis_prompt do string replacement and concatenation.
**Status**: Open
**Rationale**: Small prompts make this negligible, but could be optimized.

**Assumptions**: Prompt building is not on the hot path.

**Approach**:
1. Measure if prompt building is a bottleneck
2. If so, use pre-allocated String with capacity hints

---

## Security Hardening

### SEC-001: SecretString Does Not Implement Zeroize

**Finding**: SecretString in secret.rs holds sensitive data in a plain String that is not zeroized on drop.
**Status**: Open
**Rationale**: Secrets may remain in memory after SecretString is dropped.

**Assumptions**: Rust's allocator may reuse or leak memory containing secrets.

**Approach**:
1. Add secrecy crate or implement Zeroize trait
2. Wrap inner String in a zeroizing wrapper
3. Audit all places secrets are cloned to ensure they're also zeroized

---

### SEC-002: API Key Logged in Debug Output for ApiKey Type

**Finding**: ApiKey in provider.rs implements custom Debug that redacts, but the inner String can still be exposed via Clone.
**Status**: Open
**Rationale**: Debug is safe, but cloned ApiKey could be logged elsewhere.

**Assumptions**: No logging of ApiKey happens outside Debug.

**Approach**:
1. Make ApiKey(String) private and only expose as_str() with clear documentation
2. Add #[non_exhaustive] to prevent pattern matching
3. Consider secrecy::Secret wrapper

---

## Testing Gaps

### T-002: Concurrent Tokenizer Access Not Tested

**Finding**: TokenizerService is used via Arc across threads, but no concurrent tests verify thread safety.
**Status**: Open
**Rationale**: Moka cache and tiktoken are thread-safe, but worth verifying.

**Assumptions**: Dependencies are correctly thread-safe.

**Approach**:
1. Add a concurrent test spawning multiple tokio tasks counting tokens
2. Verify consistent results and no panics

---

### T-003: Error Path Coverage in Provider::generate

**Finding**: Provider::generate matches on variants but only Mock variants are tested.
**Status**: Open
**Rationale**: Real provider errors require network mocking.

**Assumptions**: Integration tests are out of scope.

**Approach**:
1. Add MockSequence variant for testing error sequences (already exists)
2. Add tests for specific error types: RateLimited, Timeout, ServerError
3. Verify retry behavior in orchestrator tests

---

## Documentation Gaps

### D-002: TOML Config Format Not Documented

**Finding**: Config loading supports TOML but no example config file exists.
**Status**: Open
**Rationale**: Users don't know available options.

**Assumptions**: CLI help is sufficient.

**Approach**:
1. Add example config.toml with all fields documented
2. Generate config docs from struct annotations
3. Add config path command output example in README

### D-003: Generate JSON Schema for Config

**Finding**: No JSON schema exists for validating config files.
**Status**: Open
**Rationale**: Users may create invalid configs without schema validation. TOML LSPs could use JSON schema for better editor support.

**Approach**:
1. Use schemars crate to derive JSON schema from Config struct
2. Output schema to a file during build or release process
3. Add a bin script to generate schema on demand