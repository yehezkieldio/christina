# Performance Optimization

Christina is optimized for low latency and high throughput. This document details the specific techniques used to ensure performance remains a design-time property.

## Hot-Path Optimizations

### 1. Buffer Pooling in Chunking
Diff chunking involves frequent string manipulation. To avoid the overhead of constant allocations and deallocations, we use a thread-local buffer pool.
- **Implementation**: `BUFFER_POOL` in `christina-core/src/processing/chunking.rs`.
- **Benefit**: Reuses memory for chunking large diffs, significantly reducing pressure on the allocator during the analysis phase.

### 2. LRU Token Caching
Tokenization is CPU-intensive. Since many technical prompts or diff fragments (e.g., file headers) repeat, we cache token counts.
- **Implementation**: `moka` cache in `christina-core/src/processing/tokenizer.rs`.
- **Strategy**: 
  - Skip cache for trivial strings (<50 bytes).
  - Bypass cache for massive strings (>100KB) where hashing costs exceed tokenization costs.
- **Benefit**: Reduces redundant BPE (Byte Pair Encoding) computations by over 80% in typical multi-chunk runs.

### 3. Fast-Path Binary Search
When slicing text to fit a token limit, we use a boundary-aware binary search.
- **Optimization**: We first attempt a "fast path" that maps token indices directly to byte offsets if the tokenizer supports it.
- **Safety**: The search always aligns to valid UTF-8 character boundaries to prevent panics or corrupted data.

## Orchestration Concurrency

Christina leverages `tokio` for non-blocking IO and parallel execution.

### Parallel Map Phase
Chunk summaries are generated in parallel.
- **Default Concurrency**: 4 simultaneous requests.
- **Benefit**: Transforms an O(n) latency problem into O(1) + overhead, provided the LLM provider's rate limits allow.

### Throttling & Backoff
We use a Token Bucket rate limiter combined with a semaphore to prevent thundering herds.
- **Retry Strategy**: Exponential backoff with **Full Jitter**. This ensures that if a provider rate-limits us, retries are spread out randomly to avoid immediate re-failure.

## Allocation Strategy

- **`mimalloc`**: Christina uses the `mimalloc` allocator for better performance in multi-threaded scenarios compared to the default system allocator.
- **`CompactString`**: Small strings (file paths, model names) are stored inline using the `compact_str` crate, avoiding heap allocations for the common case.

## Cross-References

- [Benchmarks](BENCHMARKS.md): Empirical measurements of these optimizations.
- [Generation Pipeline](GENERATION_PIPELINE.md): Data flow context for these optimizations.
