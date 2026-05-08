# Christina Benchmarks

Performance is a first-class requirement. We use `criterion` to measure and track the performance of critical data transformation paths.

## Primary Benchmark Suites

### 1. Tokenization (`christina-core`)
Measures the overhead of local BPE tokenization and context window slicing.
- **BPE Cache**: Effectiveness of the LRU cache for repeated token counts.
- **Slicing**: Speed of UTF-8 boundary-aware text slicing under tight token limits.

### 2. Diff Processing (`christina`)
Measures the throughput of our recursive chunking and binary detection algorithms.
- **Recursive Chunking**: Hierarchical splitting of large (10MB+) diffs.
- **Binary Detection**: Fast scanning for NUL bytes and extension heuristics.

## Execution

### Full Suite
```bash
cargo bench
```

### Targeted Execution
```bash
cargo bench --bench tokenizer_bench
cargo bench --bench chunking_bench
```

## Performance Baselines

| Operation | Baseline Target | Hardware |
|:--- |:--- |:--- |
| Token Count (Cached) | ~2µs / 1k words | Modern x86/ARM |
| Token Count (Uncached) | ~100µs / 1k words | Modern x86/ARM |
| 100-file Diff Processing | <15ms | Modern x86/ARM |

## Regression Policy

Changes to the core pipeline must not regress the `split_recursive` or `count_tokens` benchmarks by more than **5%** without explicit justification (e.g., a required accuracy improvement).
