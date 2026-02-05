Address and fix and implement and work on this all, don't leave anything, do stuff or parallaze aggressively using subagents or background agents, so everything is systematically covered and adressed:

## Critical Issues

### 1. Race Condition in Tokenizer Cache

Location: christina/src/io/llm/tokenizer.rs lines 62-88

The token cache implementation drops the lock during expensive computation, then re-acquires it. This creates a window where multiple threads can compute the same value simultaneously, wasting resources.

```
// Cache miss, compute token count
// Drop lock temporarily for expensive computation
drop(cache);
let count = self.bpe.encode_ordinary(text).len();
...
// Re-acquire lock and store in cache
let mut cache = self.token_cache.lock();
cache.put(text.to_string(), token_count);
```

Recommendation: Use a concurrent cache like `dashmap` or implement double-checked locking with a "computing" sentinel value to prevent thundering herd on cache misses.

---

### 2. Unbounded String Cloning in Cache Key

Location: christina/src/io/llm/tokenizer.rs line 85

The cache uses `text.to_string()` as a key, which clones potentially large strings (up to megabytes of diff content). This causes memory pressure and allocation overhead.

Recommendation: Consider content-based hashing with FxHash (rustc one, not since the fxhash is not maintained) or xxhash, storing only the hash as the key. Trade-off is collision risk, but with a good hash function this is negligible.

---

### 3. Missing Timeout on LLM Requests

Location: christina/src/io/llm/openai.rs, azure.rs, groq.rs

The LLM client builder does not set explicit timeouts. The retry policy defines timeout constants (LLM_TIMEOUT_SECONDS, etc.) but these are not applied to the actual HTTP client.

Recommendation: Configure the llm crate builder with explicit connection and request timeouts to prevent indefinite hangs on network issues.

---

## Edge Cases

### 4. Empty API Key Handling

Location: christina/src/generate.rs lines 91-94

The code checks for empty API keys but does this after already sending a progress event. If the key is empty, the user sees "Retrieving API key..." followed by an error.

```
let api_key = match &config.api_key {
    Some(key) if !key.is_empty() => key.clone(),
    _ => anyhow::bail!("API key not found in configuration"),
};
```

Recommendation: Validate configuration (including API key presence) before starting any progress events to provide cleaner UX.

---

### 5. Unicode Path Handling in Ignore Patterns

Location: christina/src/io/git/chunking.rs lines 155-159

The `should_limit_file` function uses `ends_with` on file paths, which may not correctly match Unicode-normalized paths or paths with platform-specific separators.

Recommendation: Normalize paths before comparison and consider using glob-style matching instead of suffix matching for ignore patterns.

---

### 6. Binary Detection Misses NUL Bytes Beyond 8KB

Location: christina/src/io/git/diff_processor.rs lines 63-67

For files smaller than MAX_BINARY_SCAN_SIZE (1MB), binary detection only scans the first 8192 bytes. A file with NUL bytes at position 9000 would be treated as text.

```
let scan_limit = content_len.min(8192);
if content.bytes().take(scan_limit).any(|b| b == 0) {
    return true;
}
```

Recommendation: Document this behavior explicitly or extend the scan range. The current behavior is intentional for performance but may surprise users.

---

### 7. Commit History on Shallow Clones

Location: christina/src/generate.rs lines 264-301

When running in a shallow clone (common in CI), `get_commit_history_impl` may fail or return fewer commits than expected. The error is logged but silently swallowed.

Recommendation: Detect shallow clones and warn the user that commit history context may be limited, rather than silently falling back to no history.

---

## Unfinished/Incomplete

### 8. Dead Code Markers Suggest Incomplete Features

Location: christina/src/events.rs

Multiple fields have `#[allow(dead_code)]` annotations, suggesting the event system is partially implemented:

```
#[derive(Debug)]
pub enum Event {
    GenerationProgress {
        stage: String,
        #[allow(dead_code)]
        generation_id: u64,
    },
    TokenCountUpdate {
        #[allow(dead_code)]
        token_count: TokenCount,
        #[allow(dead_code)]
        generation_id: u64,
    },
}
```

Recommendation: If the dead code is TUI related, remove them all. If not, implement the missing functionality or remove the fields. TUI is currently removed so these appear vestigial.

---

### 9. Unused GitRepository Trait Methods

Location: christina/src/io/git/adapter.rs lines 7-17

The `GitRepository` trait defines methods like `validate_for_commit` that are implemented but marked `#[allow(dead_code)]`. This suggests planned features not yet integrated.

Recommendation: Remove or integrate the trait abstraction. If test-only, gate behind `#[cfg(test)]`.

---

### 10. TUI Integration Document References Unimplemented Features

Location: TUI_INTEGRATION.md

The document describes a TUI integration plan that appears incomplete based on the current crate structure and the dead code markers in events.rs.

Recommendation: Update documentation to reflect actual state, TUI is temporarily removed from the codebase, will be integrated later, but not now.

---

### 12. Magic Numbers in Orchestrator

Location: christina/src/io/llm/orchestrator.rs

Several magic numbers lack constants or clear derivation:

- `0.15` for history budget calculation (line 217)
- `150.0` tokens per commit estimation (line 217)
- `MAX_SUMMARIES_PER_INTENT_BATCH = 20` (line 56)

Recommendation: Extract to named constants with documented rationale.

---

### 13. JSON Extraction Regex-Free Parser

Location: christina/src/io/llm/orchestrator.rs `extract_json` method (not shown in full read)

The JSON extraction from LLM responses uses manual brace matching. This is brittle against:
- Escaped braces in strings
- Unicode characters
- Nested structures with string content containing braces

Recommendation: Use a proper streaming JSON parser or the `serde_json::Deserializer::from_str` with custom error recovery.

---

### 14. Config Reload Silently Creates Default Profile

Location: christina/src/config/settings.rs lines 179-194

When no profiles exist, a default profile is created and immediately persisted. If file write fails, the error is converted to a hard failure. This could block users who have read-only config directories.

Recommendation: Handle write failures gracefully.

---

## Improvements

### 15. SecretString Equality Always Returns False

Location: christina-core/src/config/secret.rs lines 154-159

```
impl PartialEq for SecretString {
    fn eq(&self, _other: &Self) -> bool {
        // Secrets are never equal (security)
        false
    }
}
```

This breaks common expectations of PartialEq and could cause subtle bugs in code that relies on equality checks (e.g., caching, deduplication).

Recommendation: Either remove the PartialEq impl entirely (forcing explicit comparison) or implement constant-time comparison for security-sensitive contexts.

---

### 16. Global Tokenizer Initialization Error Handling

Location: christina/src/io/llm/tokenizer.rs lines 20-35

If tokenizer initialization fails once, subsequent calls will continue trying to initialize. There is no cached error state.

Recommendation: Cache the initialization error in a `OnceLock<Result<...>>` pattern to avoid repeated initialization attempts.

---

### 17. Diff Chunking Does Not Respect File Boundaries Fully

Location: christina/src/io/git/chunking.rs

When splitting by hunks or lines, file context (the diff header) is duplicated in each chunk. This wastes tokens and could confuse the LLM about file boundaries.

Recommendation: Consider a header-once strategy where file metadata is extracted once and chunks reference it, or ensure the LLM prompt explicitly handles fragmented diffs.

---

### 19. Provider From Profile Clones Excessively

Location: christina/src/io/llm/provider.rs lines 70-113

Multiple `.clone()` calls on strings and URLs during provider construction could be avoided with borrows or references.

Recommendation: Consider using `Cow<str>` or lifetime parameters to reduce allocations.

---

## Hardening Opportunities

### 20. API Key Logging Risk

Location: christina/src/io/llm/provider.rs

While `ApiKey` implements redacted Debug, tracing may still capture the key if passed to certain macros or if Debug is called on containing structures.

Recommendation: Audit all tracing::debug! and tracing::info! calls to ensure API keys cannot leak. Consider a `SecretGuard` wrapper that panics if Debug is called in non-test builds.

---

### 21. Prompt Injection in User Context

Location: christina-core/src/prompt.rs lines 268-273

User-provided context is directly interpolated into prompts:

```
pub const USER_CONTEXT_TEMPLATE: &str = r#"
ADDITIONAL CONTEXT PROVIDED BY THE USER:
<context>{context}</context>
```

While the system prompt warns about untrusted data, a malicious user could craft context that overrides instructions.

Recommendation: Add explicit delimiters or escape user context. Consider structured data passing if the LLM supports it.

---

### 22. No Rate Limiting Across Provider Instances

Location: christina/src/io/llm/concurrency.rs

Rate limiting is per-orchestrator instance. If multiple orchestrators exist (unlikely but possible), they would each have independent rate limits, potentially exceeding provider quotas.

Recommendation: Consider a global rate limiter if multiple orchestrators become possible.

---

### 23. Git Author/Committer Fallback May Fail

Location: christina/src/io/git/adapter.rs create_commit function

If git config lacks user.name or user.email, the commit will fail with a confusing git2 error.

Recommendation: Detect missing author configuration early and provide a clear error message directing users to configure git.

---

### 24. No Retry on Keyring Access Failure

Location: christina-core/src/config/secret.rs

Keyring access is a single attempt. On some systems, the keyring daemon may need to be woken up or the user prompted for authentication.

Recommendation: Consider a single retry with a brief delay for keyring access failures, or provide clearer guidance on keyring troubleshooting.

---

### 25. Temperature Not Clamped at Provider Level

Location: christina/src/io/llm/provider.rs line 71

Temperature is clamped in `from_profile`, but if a Provider is constructed directly with invalid temperature, no validation occurs.

Recommendation: Move validation into the Provider enum constructors or use a validated Temperature newtype.
