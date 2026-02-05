#![allow(unused_crate_dependencies, clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use christina::io::git::chunking::{
    split_by_hunks, split_by_lines, split_recursive, truncate_to_token_limit,
};

use christina_core::{
    Tokenizer, git::FileDiff, test_helpers::DeterministicTokenizer, types::FilePath,
    types::TokenCount,
};
use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};

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

/// Generate a realistic diff header.
fn diff_header(file: &str) -> String {
    format!(
        "diff --git a/{} b/{}\nindex 1111111..2222222 100644\n--- a/{}\n+++ b/{}",
        file, file, file, file
    )
}

/// Generate a diff hunk with specified lines.
fn diff_hunk(start_line: usize, line_count: usize) -> String {
    let mut hunk = format!(
        "\n@@ -{},{} +{},{} @@\n",
        start_line, line_count, start_line, line_count
    );
    for i in 0..line_count {
        hunk.push_str(&format!("+Added line {}\n", i));
    }
    hunk
}

/// Generate a complete file diff with multiple hunks.
fn generate_file_diff(filename: &str, hunk_count: usize, lines_per_hunk: usize) -> String {
    let mut diff = diff_header(filename);
    for i in 0..hunk_count {
        diff.push_str(&diff_hunk(i * lines_per_hunk, lines_per_hunk));
    }
    diff
}

/// Generate multiple file diffs.
fn generate_multi_file_diff(
    file_count: usize,
    hunks_per_file: usize,
    lines_per_hunk: usize,
) -> Vec<FileDiff> {
    let tokenizer = DeterministicTokenizer;
    (0..file_count)
        .map(|i| {
            let filename = format!("file{}.rs", i);
            let content = generate_file_diff(&filename, hunks_per_file, lines_per_hunk);
            FileDiff {
                path: FilePath::from(filename),
                content: Arc::from(content.as_str()),
                token_count: tokenizer.count_tokens(&content),
                truncated: false,
            }
        })
        .collect()
}

/// Generate a very long line (for oversized line benchmarks).
fn generate_long_line(words: usize) -> String {
    (0..words)
        .map(|i| format!("word{}", i))
        .collect::<Vec<_>>()
        .join(" ")
}

fn bench_split_recursive_small_files(c: &mut Criterion) {
    let tokenizer = DeterministicTokenizer;
    let mut group = c.benchmark_group("split_recursive/small_files");

    for file_count in [5, 10, 20, 50].iter() {
        let files = generate_multi_file_diff(*file_count, 3, 10);
        let total_size: usize = files.iter().map(|f| f.content.len()).sum();
        group.throughput(Throughput::Bytes(total_size as u64));

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}_files", file_count)),
            &files,
            |b, files| {
                b.iter(|| {
                    split_recursive(
                        black_box(files.clone()),
                        black_box(TokenCount::new_at_least_one(1000)),
                        black_box(&[]),
                        black_box(&tokenizer),
                    )
                });
            },
        );
    }

    group.finish();
}

fn bench_split_recursive_large_files(c: &mut Criterion) {
    let tokenizer = DeterministicTokenizer;
    let mut group = c.benchmark_group("split_recursive/large_files");

    for hunks in [10, 50, 100, 200].iter() {
        let files = generate_multi_file_diff(5, *hunks, 20);
        let total_size: usize = files.iter().map(|f| f.content.len()).sum();
        group.throughput(Throughput::Bytes(total_size as u64));

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}_hunks_per_file", hunks)),
            &files,
            |b, files| {
                b.iter(|| {
                    split_recursive(
                        black_box(files.clone()),
                        black_box(TokenCount::new_at_least_one(500)),
                        black_box(&[]),
                        black_box(&tokenizer),
                    )
                });
            },
        );
    }

    group.finish();
}

fn bench_split_recursive_with_lockfiles(c: &mut Criterion) {
    let tokenizer = DeterministicTokenizer;
    let mut group = c.benchmark_group("split_recursive/with_lockfiles");

    let mut files = generate_multi_file_diff(5, 10, 20);

    // Add a large lockfile
    let lockfile_content = (0..500)
        .map(|i| format!("package-{} = \"1.0.0\"\n", i))
        .collect::<String>();
    files.push(FileDiff {
        path: FilePath::from("Cargo.lock"),
        content: Arc::from(lockfile_content.as_str()),
        token_count: tokenizer.count_tokens(&lockfile_content),
        truncated: false,
    });

    let total_size: usize = files.iter().map(|f| f.content.len()).sum();
    group.throughput(Throughput::Bytes(total_size as u64));

    group.bench_function("lockfile_truncation", |b| {
        b.iter(|| {
            split_recursive(
                black_box(files.clone()),
                black_box(TokenCount::new_at_least_one(1000)),
                black_box(&["Cargo.lock".to_string()]),
                black_box(&tokenizer),
            )
        });
    });

    group.finish();
}

fn bench_split_by_hunks(c: &mut Criterion) {
    let tokenizer = DeterministicTokenizer;
    let mut group = c.benchmark_group("split_by_hunks");

    for hunk_count in [5, 20, 50, 100].iter() {
        let content = generate_file_diff("test.rs", *hunk_count, 15);
        group.throughput(Throughput::Bytes(content.len() as u64));

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}_hunks", hunk_count)),
            &content,
            |b, content| {
                b.iter(|| {
                    split_by_hunks(
                        black_box(&FilePath::from("test.rs")),
                        black_box(content),
                        black_box(TokenCount::new_at_least_one(100)),
                        black_box(&tokenizer),
                    )
                });
            },
        );
    }

    group.finish();
}

fn bench_split_by_lines(c: &mut Criterion) {
    let tokenizer = DeterministicTokenizer;
    let mut group = c.benchmark_group("split_by_lines");

    for line_count in [100, 500, 1000, 5000].iter() {
        let content = (0..*line_count)
            .map(|i| format!("+This is line number {}\n", i))
            .collect::<String>();
        group.throughput(Throughput::Bytes(content.len() as u64));

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}_lines", line_count)),
            &content,
            |b, content| {
                b.iter(|| {
                    split_by_lines(
                        black_box(&FilePath::from("test.rs")),
                        black_box(content),
                        black_box(TokenCount::new_at_least_one(50)),
                        black_box(&tokenizer),
                    )
                });
            },
        );
    }

    group.finish();
}

fn bench_split_oversized_line(c: &mut Criterion) {
    let tokenizer = DeterministicTokenizer;
    let mut group = c.benchmark_group("split_by_lines/oversized_line");

    for word_count in [100, 500, 1000, 5000].iter() {
        let line = generate_long_line(*word_count);
        let content = format!("+{}\n", line);
        group.throughput(Throughput::Bytes(content.len() as u64));

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}_words", word_count)),
            &content,
            |b, content| {
                b.iter(|| {
                    split_by_lines(
                        black_box(&FilePath::from("test.rs")),
                        black_box(content),
                        black_box(TokenCount::new_at_least_one(10)),
                        black_box(&tokenizer),
                    )
                });
            },
        );
    }

    group.finish();
}

fn bench_truncate_to_token_limit(c: &mut Criterion) {
    let tokenizer = DeterministicTokenizer;
    let mut group = c.benchmark_group("truncate_to_token_limit");

    for line_count in [100, 500, 1000, 5000].iter() {
        let content = (0..*line_count)
            .map(|i| format!("This is line number {}\n", i))
            .collect::<String>();
        group.throughput(Throughput::Bytes(content.len() as u64));

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}_lines", line_count)),
            &content,
            |b, content| {
                b.iter(|| {
                    truncate_to_token_limit(
                        black_box(content),
                        black_box(TokenCount::new_at_least_one((*line_count as u32) / 2)),
                        black_box(&tokenizer),
                    )
                });
            },
        );
    }

    group.finish();
}

fn bench_truncate_to_token_limit_fast_path(c: &mut Criterion) {
    let tokenizer = DeterministicTokenizer;
    let mut group = c.benchmark_group("truncate_to_token_limit/fast_path");

    for line_count in [100, 500, 1000].iter() {
        let content = (0..*line_count)
            .map(|i| format!("This is line number {}\n", i))
            .collect::<String>();
        group.throughput(Throughput::Bytes(content.len() as u64));

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}_lines", line_count)),
            &content,
            |b, content| {
                b.iter(|| {
                    truncate_to_token_limit(
                        black_box(content),
                        black_box(TokenCount::new_at_least_one((*line_count as u32) * 10)), // Already under limit
                        black_box(&tokenizer),
                    )
                });
            },
        );
    }

    group.finish();
}

fn bench_unicode_handling(c: &mut Criterion) {
    let tokenizer = DeterministicTokenizer;
    let mut group = c.benchmark_group("split_by_lines/unicode");

    // Test with emoji and multi-byte characters
    let emoji_content = (0..1000)
        .map(|i| format!("+Line {} with emoji 🚀 👋 🌍 and unicode résumé café\n", i))
        .collect::<String>();

    group.throughput(Throughput::Bytes(emoji_content.len() as u64));
    group.bench_function("emoji_and_accents", |b| {
        b.iter(|| {
            split_by_lines(
                black_box(&FilePath::from("test.rs")),
                black_box(&emoji_content),
                black_box(TokenCount::new_at_least_one(50)),
                black_box(&tokenizer),
            )
        });
    });

    // Test with CJK characters
    let cjk_content = (0..1000)
        .map(|i| format!("+行 {} こんにちは 世界 你好世界\n", i))
        .collect::<String>();

    group.throughput(Throughput::Bytes(cjk_content.len() as u64));
    group.bench_function("cjk_characters", |b| {
        b.iter(|| {
            split_by_lines(
                black_box(&FilePath::from("test.rs")),
                black_box(&cjk_content),
                black_box(TokenCount::new_at_least_one(50)),
                black_box(&tokenizer),
            )
        });
    });

    group.finish();
}

fn bench_token_limit_variations(c: &mut Criterion) {
    let tokenizer = DeterministicTokenizer;
    let mut group = c.benchmark_group("split_recursive/token_limits");

    let files = generate_multi_file_diff(10, 20, 15);
    let total_size: usize = files.iter().map(|f| f.content.len()).sum();
    group.throughput(Throughput::Bytes(total_size as u64));

    for limit in [100, 500, 1000, 5000, 10000].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("limit_{}", limit)),
            &files,
            |b, files| {
                b.iter(|| {
                    split_recursive(
                        black_box(files.clone()),
                        black_box(TokenCount::new_at_least_one(*limit)),
                        black_box(&[]),
                        black_box(&tokenizer),
                    )
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_split_recursive_small_files,
    bench_split_recursive_large_files,
    bench_split_recursive_with_lockfiles,
    bench_split_by_hunks,
    bench_split_by_lines,
    bench_split_oversized_line,
    bench_truncate_to_token_limit,
    bench_truncate_to_token_limit_fast_path,
    bench_unicode_handling,
    bench_token_limit_variations,
);
criterion_main!(benches);
