# Benchmarks

This document describes the performance benchmarks for Christina, covering tokenization operations and diff chunking algorithms.

## Overview

Christina uses [Criterion](https://github.com/bheisler/criterion.rs) for benchmarking performance-critical code paths:

- **Tokenization operations** (encoding, decoding, token counting, slicing)
- **Diff chunking algorithm** (recursive splitting, hunk-based splitting, line splitting)
- **Diff processing** (binary detection, size limits, mixed content)

## Running Benchmarks

### Run all benchmarks

```bash
cargo bench
```

### Run specific benchmark suite

```bash
# Tokenizer trait benchmarks (christina-core)
cargo bench --bench tokenizer_bench

# Real tokenizer service benchmarks (christina)
cargo bench --bench tokenizer_service_bench

# Diff chunking benchmarks
cargo bench --bench chunking_bench

# Diff processor benchmarks
cargo bench --bench diff_processor_bench
```

### Test benchmarks (without measuring)

Verify benchmarks compile and run without full measurement cycles:

```bash
cargo bench -- --test
```

### Run specific benchmark groups

```bash
# Only count_tokens benchmarks
cargo bench count_tokens

# Only split_recursive benchmarks
cargo bench split_recursive

# Only binary detection benchmarks
cargo bench binary_detection
```

### Profile-guided benchmarking

For more detailed profiling, run with longer warm-up and measurement times:

```bash
CRITERION_WARM_UP_TIME=10s CRITERION_MEASUREMENT_TIME=30s cargo bench
```

## Benchmark Suites

### 1. Tokenizer Benchmarks (`christina-core`)

**File:** `christina-core/benches/tokenizer_bench.rs`

Benchmarks the core tokenizer trait implementation using a mock tokenizer:

- `count_tokens` - Token counting with various text sizes (10-10k words)
- `encode` - Text encoding to token IDs
- `decode` - Token IDs decoding back to text
- `slice_to_token_limit` - UTF-8-aware slicing with token limits
  - Fast path (already under limit)
  - Binary search path (tight limits)
- `encode_decode_roundtrip` - Full encoding/decoding cycle

### 2. Tokenizer Service Benchmarks (`christina`)

**File:** `christina/benches/tokenizer_service_bench.rs`

Benchmarks the real tokenizer service using `tiktoken-rs` (o200k_base):

- `count_tokens/short_text` - Very short strings (10-50 words)
- `count_tokens/medium_text` - Medium texts (100-5k words)
- `count_tokens/large_text` - Large texts (10k-100k words)
- `count_tokens/with_cache` - Cache hit performance
- `count_tokens/cache_miss` - Cache miss overhead
- `encode` - BPE encoding performance
- `decode` - BPE decoding performance
- `slice_to_token_limit` - Real-world slicing with various limits
- `token_budget/remaining` - Token budget calculations
- `tokenization/unicode` - Unicode handling (emoji, CJK, mixed scripts)

### 3. Diff Chunking Benchmarks (`christina`)

**File:** `christina/benches/chunking_bench.rs`

Benchmarks the diff chunking algorithm:

- `split_recursive/small_files` - 5-50 files, multiple hunks each
- `split_recursive/large_files` - Fewer files with many hunks (10-200 hunks/file)
- `split_recursive/with_lockfiles` - Lockfile truncation behavior
- `split_by_hunks` - Hunk-level splitting (5-100 hunks)
- `split_by_lines` - Line-level splitting (100-5k lines)
- `split_by_lines/oversized_line` - Single very long lines (100-5k words)
- `truncate_to_token_limit` - Token-aware truncation
  - Normal path
  - Fast path (already under limit)
- `split_by_lines/unicode` - Unicode handling (emoji, CJK)
- `split_recursive/token_limits` - Various token budget scenarios (100-10k tokens)

### 4. Diff Processor Benchmarks (`christina`)

**File:** `christina/benches/diff_processor_bench.rs`

Benchmarks diff processing and binary detection:

- `binary_detection/text` - Pure text diffs (10-5k lines)
- `binary_detection/nul_bytes` - NUL byte detection
- `binary_detection/extensions` - Extension-based detection (1-50 files)
- `binary_detection/late_nul_byte` - NUL bytes beyond 8KB scan window
- `binary_detection/large_file_sampling` - Sampled detection for 2MB+ files
- `process/small_diff` - Small diffs (1-20 files)
- `process/large_diff` - Large diffs (1k-50k lines)
- `process/token_limits` - Processing with various token budgets
- `process_safe/mixed_content` - Mixed text/binary content
- `process/deletion_only` - Deletion-only diffs
- `process/with_ignore_patterns` - Lockfile handling
- `process/unicode` - Unicode in diffs

## Understanding Results

Criterion produces HTML reports in `target/criterion/`:

```
target/criterion/
├── count_tokens/
│   ├── 10/
│   ├── 100/
│   └── report/
│       └── index.html
└── split_recursive/
    └── ...
```

Open `target/criterion/<benchmark_name>/report/index.html` in a browser to view:

- Throughput measurements (operations/sec or bytes/sec)
- Time distributions (mean, median, std dev)
- Regression analysis (vs. previous runs)
- Detailed plots

### Key Metrics

- **Time**: Lower is better
- **Throughput**: Higher is better
- **Regression**: Changes > 5% are flagged

### Interpreting Warnings

Criterion may warn about high variance or outliers. Common causes:

- **High variance**: System load, thermal throttling
- **Outliers**: GC pauses, system interrupts, cache effects

For production benchmarking:
1. Close other applications
2. Disable CPU frequency scaling
3. Run multiple times and compare medians

## Continuous Integration

For CI benchmarking, use test mode to verify benchmarks compile and run:

```bash
cargo bench -- --test
```

This runs a single quick iteration without full measurement cycles.

## Profiling Integration

To profile with `perf`:

```bash
cargo bench --bench chunking_bench -- --profile-time=10
perf record -g target/release/deps/chunking_bench-* --bench
perf report
```

To profile with `flamegraph`:

```bash
cargo install flamegraph
cargo flamegraph --bench chunking_bench
```

## Adding New Benchmarks

1. Create a new file in the appropriate `benches/` directory
2. Add the benchmark configuration to `Cargo.toml`:

```toml
[[bench]]
name = "my_new_bench"
harness = false
```

3. Use the Criterion API:

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn my_benchmark(c: &mut Criterion) {
    let data = setup_test_data();
    c.bench_function("my_operation", |b| {
        b.iter(|| my_function(black_box(&data)));
    });
}

criterion_group!(benches, my_benchmark);
criterion_main!(benches);
```

## Performance Baselines

Current performance targets (approximate, hardware-dependent):

- **Token counting (cached)**: ~1-5 µs for 1k word text
- **Token counting (uncached)**: ~50-200 µs for 1k word text
- **Diff chunking**: ~100-500 µs for 10 files with 20 hunks each
- **Binary detection**: <1 ms for files up to 1MB
- **Overall processing**: <10 ms for typical 100-file diff

These are guideline targets. Actual performance depends on:
- Hardware (CPU, memory)
- Input characteristics (file sizes, hunk count, text complexity)
- Cache state
- System load

## Troubleshooting

### Benchmarks fail to compile

Ensure `criterion` is in dev-dependencies:

```toml
[dev-dependencies]
criterion = { workspace = true }
```

### Benchmarks run forever

Some benchmarks process large inputs. Wait for completion or reduce input sizes in test mode.

### Results vary wildly

Run on an idle system with consistent CPU frequency. For production benchmarks, use a dedicated benchmark machine.

### Low throughput numbers

Criterion reports throughput in elements/sec or bytes/sec. Ensure you're:
1. Setting throughput correctly: `group.throughput(Throughput::Bytes(data.len() as u64))`
2. Measuring realistic workloads

## References

- [Criterion User Guide](https://bheisler.github.io/criterion.rs/book/)
- [Rust Performance Book](https://nnethercote.github.io/perf-book/)
- [Benchmarking Best Practices](https://easyperf.net/blog/)
