#![allow(unused_crate_dependencies, clippy::unwrap_used, clippy::expect_used)]

use christina_core::{types::TokenCount, Tokenizer};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

// Placeholder uses for dev-dependencies not used in this benchmark
use anyhow as _;
use compact_str as _;
use git2 as _;
use keyring as _;
use regex as _;
use serde as _;
use serde_json as _;
use tempfile as _;
use thiserror as _;
use tracing as _;
use url as _;

/// Mock tokenizer for benchmarking trait methods.
struct MockTokenizer;

impl Tokenizer for MockTokenizer {
    fn count_tokens(&self, text: &str) -> TokenCount {
        if text.is_empty() {
            return TokenCount::new_saturating(0);
        }
        let count = text.split_whitespace().count();
        TokenCount::new_saturating(count as u32)
    }

    fn encoding_name(&self) -> &str {
        "mock-whitespace"
    }

    fn encode(&self, text: &str) -> Vec<u32> {
        text.chars().map(|c| c as u32).collect()
    }

    fn decode(&self, tokens: &[u32]) -> Option<String> {
        tokens
            .iter()
            .filter_map(|&token| char::from_u32(token))
            .collect::<String>()
            .into()
    }
}

fn generate_text(word_count: usize) -> String {
    let words = ["hello", "world", "test", "benchmark", "performance", "rust"];
    (0..word_count)
        .map(|i| words[i % words.len()])
        .collect::<Vec<_>>()
        .join(" ")
}

fn generate_long_text(lines: usize) -> String {
    let line = "The quick brown fox jumps over the lazy dog. Lorem ipsum dolor sit amet, consectetur adipiscing elit.";
    (0..lines).map(|_| line).collect::<Vec<_>>().join("\n")
}

fn bench_count_tokens(c: &mut Criterion) {
    let tokenizer = MockTokenizer;
    let mut group = c.benchmark_group("count_tokens");

    for size in [10, 100, 1000, 10_000].iter() {
        let text = generate_text(*size);
        group.throughput(Throughput::Bytes(text.len() as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), &text, |b, text| {
            b.iter(|| tokenizer.count_tokens(black_box(text)));
        });
    }

    group.finish();
}

fn bench_encode(c: &mut Criterion) {
    let tokenizer = MockTokenizer;
    let mut group = c.benchmark_group("encode");

    for size in [10, 100, 1000, 10_000].iter() {
        let text = generate_text(*size);
        group.throughput(Throughput::Bytes(text.len() as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), &text, |b, text| {
            b.iter(|| tokenizer.encode(black_box(text)));
        });
    }

    group.finish();
}

fn bench_decode(c: &mut Criterion) {
    let tokenizer = MockTokenizer;
    let mut group = c.benchmark_group("decode");

    for size in [10, 100, 1000, 10_000].iter() {
        let text = generate_text(*size);
        let tokens = tokenizer.encode(&text);
        group.throughput(Throughput::Elements(tokens.len() as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), &tokens, |b, tokens| {
            b.iter(|| tokenizer.decode(black_box(tokens)));
        });
    }

    group.finish();
}

fn bench_slice_to_token_limit(c: &mut Criterion) {
    let tokenizer = MockTokenizer;
    let mut group = c.benchmark_group("slice_to_token_limit");

    for lines in [10, 100, 1000].iter() {
        let text = generate_long_text(*lines);
        let limit = TokenCount::new_saturating((*lines as u32) / 2); // 50% limit
        group.throughput(Throughput::Bytes(text.len() as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}_lines", lines)),
            &text,
            |b, text| {
                b.iter(|| tokenizer.slice_to_token_limit(black_box(text), black_box(limit)));
            },
        );
    }

    group.finish();
}

fn bench_slice_to_token_limit_fast_path(c: &mut Criterion) {
    let tokenizer = MockTokenizer;
    let mut group = c.benchmark_group("slice_to_token_limit_fast_path");

    for lines in [10, 100, 1000].iter() {
        let text = generate_long_text(*lines);
        let limit = TokenCount::new_saturating(*lines as u32 * 20); // Large limit (fast path)
        group.throughput(Throughput::Bytes(text.len() as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}_lines", lines)),
            &text,
            |b, text| {
                b.iter(|| tokenizer.slice_to_token_limit(black_box(text), black_box(limit)));
            },
        );
    }

    group.finish();
}

fn bench_slice_to_token_limit_binary_search(c: &mut Criterion) {
    let tokenizer = MockTokenizer;
    let mut group = c.benchmark_group("slice_to_token_limit_binary_search");

    for lines in [10, 100, 1000].iter() {
        let text = generate_long_text(*lines);
        let limit = TokenCount::new_saturating((*lines as u32) / 10); // 10% limit (slow path)
        group.throughput(Throughput::Bytes(text.len() as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}_lines", lines)),
            &text,
            |b, text| {
                b.iter(|| tokenizer.slice_to_token_limit(black_box(text), black_box(limit)));
            },
        );
    }

    group.finish();
}

fn bench_encode_decode_roundtrip(c: &mut Criterion) {
    let tokenizer = MockTokenizer;
    let mut group = c.benchmark_group("encode_decode_roundtrip");

    for size in [10, 100, 1000].iter() {
        let text = generate_text(*size);
        group.throughput(Throughput::Bytes(text.len() as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), &text, |b, text| {
            b.iter(|| {
                let encoded = tokenizer.encode(black_box(text));
                tokenizer.decode(black_box(&encoded))
            });
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_count_tokens,
    bench_encode,
    bench_decode,
    bench_slice_to_token_limit,
    bench_slice_to_token_limit_fast_path,
    bench_slice_to_token_limit_binary_search,
    bench_encode_decode_roundtrip,
);
criterion_main!(benches);
