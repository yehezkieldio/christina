#![allow(unused_crate_dependencies, clippy::unwrap_used, clippy::expect_used)]

use christina::io::llm::tokenizer::{get_tokenizer, TokenBudget};
use christina_core::types::TokenCount;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

// Placeholder uses for dev-dependencies not used in this benchmark
use ahash as _;
use anyhow as _;
use cap as _;
use clap as _;
use clap_complete as _;
use config as _;
use console as _;
use dhat as _;
use dialoguer as _;
use directories as _;
use fs2 as _;
use futures as _;
use git2 as _;
use indicatif as _;
use keyring as _;
use llm as _;
use mimalloc as _;
use moka as _;
use proptest as _;
use serde as _;
use serde_json as _;
use tiktoken_rs as _;
use tokio as _;
use toml as _;
use tracing as _;
use tracing_appender as _;
use tracing_subscriber as _;
use url as _;

/// Generate text with specified word count.
fn generate_text(word_count: usize) -> String {
    let words = vec![
        "hello", "world", "test", "benchmark", "performance", "rust", "tokenization",
        "encoding", "decoding", "optimization", "algorithm", "implementation",
    ];
    (0..word_count)
        .map(|i| words[i % words.len()])
        .collect::<Vec<_>>()
        .join(" ")
}

/// Generate realistic code snippet.
fn generate_code(lines: usize) -> String {
    (0..lines)
        .map(|i| {
            format!(
                "fn function_{}(param: &str) -> Result<String, Error> {{\n    Ok(param.to_string())\n}}\n",
                i
            )
        })
        .collect()
}

/// Generate long single line (minified JS style).
fn generate_long_line(tokens: usize) -> String {
    format!("const data = [{}];", (0..tokens).map(|i| format!("'{}'", i)).collect::<Vec<_>>().join(", "))
}

fn bench_count_tokens_short_text(c: &mut Criterion) {
    let tokenizer = get_tokenizer().expect("Failed to initialize tokenizer");
    let mut group = c.benchmark_group("count_tokens/short_text");

    for size in [10, 20, 30, 40, 50].iter() {
        let text = generate_text(*size);
        group.throughput(Throughput::Bytes(text.len() as u64));

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}_words", size)),
            &text,
            |b, text| {
                b.iter(|| tokenizer.count_tokens(black_box(text)));
            },
        );
    }

    group.finish();
}

fn bench_count_tokens_medium_text(c: &mut Criterion) {
    let tokenizer = get_tokenizer().expect("Failed to initialize tokenizer");
    let mut group = c.benchmark_group("count_tokens/medium_text");

    for size in [100, 500, 1000, 5000].iter() {
        let text = generate_text(*size);
        group.throughput(Throughput::Bytes(text.len() as u64));

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}_words", size)),
            &text,
            |b, text| {
                b.iter(|| tokenizer.count_tokens(black_box(text)));
            },
        );
    }

    group.finish();
}

fn bench_count_tokens_large_text(c: &mut Criterion) {
    let tokenizer = get_tokenizer().expect("Failed to initialize tokenizer");
    let mut group = c.benchmark_group("count_tokens/large_text");

    for size in [10_000, 50_000, 100_000].iter() {
        let text = generate_text(*size);
        group.throughput(Throughput::Bytes(text.len() as u64));

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}_words", size)),
            &text,
            |b, text| {
                b.iter(|| tokenizer.count_tokens(black_box(text)));
            },
        );
    }

    group.finish();
}

fn bench_count_tokens_with_cache(c: &mut Criterion) {
    let tokenizer = get_tokenizer().expect("Failed to initialize tokenizer");
    let mut group = c.benchmark_group("count_tokens/with_cache");

    let texts: Vec<String> = (0..100).map(|i| generate_text(100 + i * 10)).collect();

    // Prime the cache
    for text in &texts {
        tokenizer.count_tokens(text);
    }

    group.bench_function("100_cached_texts", |b| {
        b.iter(|| {
            for text in &texts {
                tokenizer.count_tokens(black_box(text));
            }
        });
    });

    group.finish();
}

fn bench_count_tokens_cache_miss(c: &mut Criterion) {
    let tokenizer = get_tokenizer().expect("Failed to initialize tokenizer");
    let mut group = c.benchmark_group("count_tokens/cache_miss");

    // Generate unique texts for each iteration
    group.bench_function("100_unique_texts", |b| {
        let mut counter = 0;
        b.iter(|| {
            for i in 0..100 {
                let text = format!("{} {}", generate_text(100), counter + i);
                tokenizer.count_tokens(black_box(&text));
            }
            counter += 100;
        });
    });

    group.finish();
}

fn bench_encode(c: &mut Criterion) {
    let tokenizer = get_tokenizer().expect("Failed to initialize tokenizer");
    let mut group = c.benchmark_group("encode");

    for size in [100, 1000, 10_000, 50_000].iter() {
        let text = generate_text(*size);
        group.throughput(Throughput::Bytes(text.len() as u64));

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}_words", size)),
            &text,
            |b, text| {
                b.iter(|| tokenizer.encode(black_box(text)));
            },
        );
    }

    group.finish();
}

fn bench_decode(c: &mut Criterion) {
    let tokenizer = get_tokenizer().expect("Failed to initialize tokenizer");
    let mut group = c.benchmark_group("decode");

    for size in [100, 1000, 10_000, 50_000].iter() {
        let text = generate_text(*size);
        let tokens = tokenizer.encode(&text);
        group.throughput(Throughput::Elements(tokens.len() as u64));

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}_tokens", tokens.len())),
            &tokens,
            |b, tokens| {
                b.iter(|| tokenizer.decode(black_box(tokens)));
            },
        );
    }

    group.finish();
}

fn bench_encode_decode_roundtrip(c: &mut Criterion) {
    let tokenizer = get_tokenizer().expect("Failed to initialize tokenizer");
    let mut group = c.benchmark_group("encode_decode_roundtrip");

    for size in [100, 1000, 10_000].iter() {
        let text = generate_text(*size);
        group.throughput(Throughput::Bytes(text.len() as u64));

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}_words", size)),
            &text,
            |b, text| {
                b.iter(|| {
                    let encoded = tokenizer.encode(black_box(text));
                    tokenizer.decode(black_box(&encoded))
                });
            },
        );
    }

    group.finish();
}

fn bench_slice_to_token_limit_small(c: &mut Criterion) {
    let tokenizer = get_tokenizer().expect("Failed to initialize tokenizer");
    let mut group = c.benchmark_group("slice_to_token_limit/small");

    for size in [100, 500, 1000].iter() {
        let text = generate_code(*size);
        let limit = TokenCount::new_at_least_one((*size as u32) / 2);
        group.throughput(Throughput::Bytes(text.len() as u64));

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}_lines_50pct", size)),
            &text,
            |b, text| {
                b.iter(|| tokenizer.slice_to_token_limit(black_box(text), black_box(limit)));
            },
        );
    }

    group.finish();
}

fn bench_slice_to_token_limit_large(c: &mut Criterion) {
    let tokenizer = get_tokenizer().expect("Failed to initialize tokenizer");
    let mut group = c.benchmark_group("slice_to_token_limit/large");

    for size in [5000, 10_000, 50_000].iter() {
        let text = generate_code(*size);
        let limit = TokenCount::new_at_least_one((*size as u32) / 2);
        group.throughput(Throughput::Bytes(text.len() as u64));

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}_lines_50pct", size)),
            &text,
            |b, text| {
                b.iter(|| tokenizer.slice_to_token_limit(black_box(text), black_box(limit)));
            },
        );
    }

    group.finish();
}

fn bench_slice_to_token_limit_fast_path(c: &mut Criterion) {
    let tokenizer = get_tokenizer().expect("Failed to initialize tokenizer");
    let mut group = c.benchmark_group("slice_to_token_limit/fast_path");

    for size in [100, 1000, 10_000].iter() {
        let text = generate_code(*size);
        let limit = TokenCount::new_at_least_one((*size as u32) * 10); // Already under limit
        group.throughput(Throughput::Bytes(text.len() as u64));

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}_lines_already_under", size)),
            &text,
            |b, text| {
                b.iter(|| tokenizer.slice_to_token_limit(black_box(text), black_box(limit)));
            },
        );
    }

    group.finish();
}

fn bench_slice_to_token_limit_tight(c: &mut Criterion) {
    let tokenizer = get_tokenizer().expect("Failed to initialize tokenizer");
    let mut group = c.benchmark_group("slice_to_token_limit/tight");

    for size in [100, 1000, 10_000].iter() {
        let text = generate_code(*size);
        let limit = TokenCount::new_at_least_one((*size as u32) / 10); // 10% limit
        group.throughput(Throughput::Bytes(text.len() as u64));

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}_lines_10pct", size)),
            &text,
            |b, text| {
                b.iter(|| tokenizer.slice_to_token_limit(black_box(text), black_box(limit)));
            },
        );
    }

    group.finish();
}

fn bench_slice_long_line(c: &mut Criterion) {
    let tokenizer = get_tokenizer().expect("Failed to initialize tokenizer");
    let mut group = c.benchmark_group("slice_to_token_limit/long_line");

    for size in [1000, 5000, 10_000].iter() {
        let text = generate_long_line(*size);
        let limit = TokenCount::new_at_least_one((*size as u32) / 2);
        group.throughput(Throughput::Bytes(text.len() as u64));

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}_tokens", size)),
            &text,
            |b, text| {
                b.iter(|| tokenizer.slice_to_token_limit(black_box(text), black_box(limit)));
            },
        );
    }

    group.finish();
}

fn bench_token_budget_remaining(c: &mut Criterion) {
    let mut group = c.benchmark_group("token_budget/remaining");

    group.bench_function("medium", |b| {
        let budget = TokenBudget::medium();
        b.iter(|| black_box(budget.remaining_for_diff()));
    });

    group.bench_function("custom_large", |b| {
        let budget = TokenBudget::new(
            TokenCount::new_at_least_one(256_000),
            TokenCount::new_at_least_one(4_096),
            TokenCount::new_at_least_one(1_000),
            TokenCount::new_at_least_one(500),
        );
        b.iter(|| black_box(budget.remaining_for_diff()));
    });

    group.finish();
}

fn bench_unicode_tokenization(c: &mut Criterion) {
    let tokenizer = get_tokenizer().expect("Failed to initialize tokenizer");
    let mut group = c.benchmark_group("tokenization/unicode");

    // Emoji heavy text
    let emoji_text = (0..1000)
        .map(|i| format!("Message {} 🚀 👋 🌍 ✨", i))
        .collect::<Vec<_>>()
        .join(" ");

    group.throughput(Throughput::Bytes(emoji_text.len() as u64));
    group.bench_function("emoji_heavy", |b| {
        b.iter(|| tokenizer.count_tokens(black_box(&emoji_text)));
    });

    // CJK characters
    let cjk_text = (0..1000)
        .map(|i| format!("行 {} こんにちは 世界 你好世界", i))
        .collect::<Vec<_>>()
        .join(" ");

    group.throughput(Throughput::Bytes(cjk_text.len() as u64));
    group.bench_function("cjk_characters", |b| {
        b.iter(|| tokenizer.count_tokens(black_box(&cjk_text)));
    });

    // Mixed scripts
    let mixed_text = (0..1000)
        .map(|i| format!("Hello {} السلام עליכم résumé café", i))
        .collect::<Vec<_>>()
        .join(" ");

    group.throughput(Throughput::Bytes(mixed_text.len() as u64));
    group.bench_function("mixed_scripts", |b| {
        b.iter(|| tokenizer.count_tokens(black_box(&mixed_text)));
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_count_tokens_short_text,
    bench_count_tokens_medium_text,
    bench_count_tokens_large_text,
    bench_count_tokens_with_cache,
    bench_count_tokens_cache_miss,
    bench_encode,
    bench_decode,
    bench_encode_decode_roundtrip,
    bench_slice_to_token_limit_small,
    bench_slice_to_token_limit_large,
    bench_slice_to_token_limit_fast_path,
    bench_slice_to_token_limit_tight,
    bench_slice_long_line,
    bench_token_budget_remaining,
    bench_unicode_tokenization,
);
criterion_main!(benches);
