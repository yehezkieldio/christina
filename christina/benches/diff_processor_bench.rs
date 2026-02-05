#![allow(unused_crate_dependencies)]

use christina::io::git::diff_processor::DiffProcessor;
use christina_core::{test_helpers::DeterministicTokenizer, types::TokenCount, Tokenizer};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::sync::Arc;

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

/// Generate a realistic text diff.
fn generate_text_diff(file_count: usize, lines_per_file: usize) -> String {
    let mut diff = String::new();
    for i in 0..file_count {
        diff.push_str(&format!(
            "diff --git a/file{}.rs b/file{}.rs\n",
            i, i
        ));
        diff.push_str(&format!("index 1234567..abcdefg 100644\n"));
        diff.push_str(&format!("--- a/file{}.rs\n", i));
        diff.push_str(&format!("+++ b/file{}.rs\n", i));
        diff.push_str(&format!("@@ -1,{0} +1,{0} @@\n", lines_per_file));
        for j in 0..lines_per_file {
            diff.push_str(&format!("+    println!(\"Line {} in file {}\");\n", j, i));
        }
    }
    diff
}

/// Generate a diff with binary content (contains NUL bytes).
fn generate_binary_diff(file_count: usize) -> String {
    let mut diff = String::new();
    for i in 0..file_count {
        diff.push_str(&format!(
            "diff --git a/image{}.png b/image{}.png\n",
            i, i
        ));
        diff.push_str("index 1234567..abcdefg 100644\n");
        diff.push_str(&format!("--- a/image{}.png\n", i));
        diff.push_str(&format!("+++ b/image{}.png\n", i));
        diff.push_str("@@ -0,0 +1,1 @@\n");
        diff.push('\0'); // NUL byte for binary detection
        diff.push_str("binary content\n");
    }
    diff
}

/// Generate a diff with binary extensions.
fn generate_binary_extension_diff(file_count: usize) -> String {
    let extensions = ["png", "jpg", "pdf", "zip", "woff", "mp4"];
    let mut diff = String::new();
    for i in 0..file_count {
        let ext = extensions[i % extensions.len()];
        diff.push_str(&format!(
            "diff --git a/file{}.{} b/file{}.{}\n",
            i, ext, i, ext
        ));
        diff.push_str("index 1234567..abcdefg 100644\n");
        diff.push_str(&format!("--- a/file{}.{}\n", i, ext));
        diff.push_str(&format!("+++ b/file{}.{}\n", i, ext));
        diff.push_str("@@ -0,0 +1,1 @@\n");
        diff.push_str("+file content\n");
    }
    diff
}

/// Generate a very large text diff (for size limit testing).
fn generate_large_text_diff(lines: usize) -> String {
    let mut diff = String::new();
    diff.push_str("diff --git a/large.rs b/large.rs\n");
    diff.push_str("index 1234567..abcdefg 100644\n");
    diff.push_str("--- a/large.rs\n");
    diff.push_str("+++ b/large.rs\n");
    diff.push_str(&format!("@@ -1,{0} +1,{0} @@\n", lines));
    for i in 0..lines {
        diff.push_str(&format!("+// This is line number {} with some content\n", i));
    }
    diff
}

/// Generate a diff with NUL byte late in the file (beyond 8KB).
fn generate_late_nul_byte_diff() -> String {
    let mut diff = String::new();
    diff.push_str("diff --git a/file.bin b/file.bin\n");
    diff.push_str("index 1234567..abcdefg 100644\n");
    diff.push_str("--- a/file.bin\n");
    diff.push_str("+++ b/file.bin\n");
    diff.push_str("@@ -1 +1 @@\n");
    diff.push_str(&"a".repeat(9000)); // 9KB of text
    diff.push('\0'); // NUL byte after 8KB scan window
    diff.push_str("more content\n");
    diff
}

/// Generate a very large file for sampling benchmark (>1MB).
fn generate_large_file_for_sampling() -> String {
    let mut content = String::new();
    content.push_str("diff --git a/large.bin b/large.bin\n");
    content.push_str("index 1234567..abcdefg 100644\n");
    content.push_str("--- a/large.bin\n");
    content.push_str("+++ b/large.bin\n");
    content.push_str("@@ -1 +1 @@\n");
    // Create 2MB of content with NUL byte early in sampling
    let sampling_interval = 16;
    content.push_str(&"a".repeat(sampling_interval));
    content.push('\0');
    content.push_str(&"b".repeat(2_000_000 - content.len()));
    content
}

fn create_processor(token_limit: u32) -> DiffProcessor {
    let tokenizer: Arc<dyn Tokenizer> = Arc::new(DeterministicTokenizer);
    DiffProcessor::new(tokenizer, TokenCount::new_saturating(token_limit))
}

fn bench_binary_detection_text(c: &mut Criterion) {
    let mut group = c.benchmark_group("binary_detection/text");

    for line_count in [10, 100, 1000, 5000].iter() {
        let diff = generate_text_diff(1, *line_count);
        group.throughput(Throughput::Bytes(diff.len() as u64));

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}_lines", line_count)),
            &diff,
            |b, diff| {
                let processor = create_processor(10_000);
                b.iter(|| processor.process(black_box(diff)));
            },
        );
    }

    group.finish();
}

fn bench_binary_detection_nul_bytes(c: &mut Criterion) {
    let mut group = c.benchmark_group("binary_detection/nul_bytes");

    for file_count in [1, 5, 10, 20].iter() {
        let diff = generate_binary_diff(*file_count);
        group.throughput(Throughput::Bytes(diff.len() as u64));

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}_files", file_count)),
            &diff,
            |b, diff| {
                let processor = create_processor(10_000);
                b.iter(|| processor.process_safe(black_box(diff)));
            },
        );
    }

    group.finish();
}

fn bench_binary_detection_extensions(c: &mut Criterion) {
    let mut group = c.benchmark_group("binary_detection/extensions");

    for file_count in [1, 5, 10, 20, 50].iter() {
        let diff = generate_binary_extension_diff(*file_count);
        group.throughput(Throughput::Bytes(diff.len() as u64));

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}_files", file_count)),
            &diff,
            |b, diff| {
                let processor = create_processor(10_000);
                b.iter(|| processor.process_safe(black_box(diff)));
            },
        );
    }

    group.finish();
}

fn bench_binary_detection_late_nul_byte(c: &mut Criterion) {
    let diff = generate_late_nul_byte_diff();
    let mut group = c.benchmark_group("binary_detection/late_nul_byte");
    group.throughput(Throughput::Bytes(diff.len() as u64));

    group.bench_function("9kb_before_nul", |b| {
        let processor = create_processor(10_000);
        b.iter(|| processor.process(black_box(&diff)));
    });

    group.finish();
}

fn bench_binary_detection_large_file_sampling(c: &mut Criterion) {
    let diff = generate_large_file_for_sampling();
    let mut group = c.benchmark_group("binary_detection/large_file_sampling");
    group.throughput(Throughput::Bytes(diff.len() as u64));

    group.bench_function("2mb_with_early_nul", |b| {
        let processor = create_processor(10_000);
        b.iter(|| processor.process(black_box(&diff)));
    });

    group.finish();
}

fn bench_process_small_diff(c: &mut Criterion) {
    let mut group = c.benchmark_group("process/small_diff");

    for file_count in [1, 5, 10, 20].iter() {
        let diff = generate_text_diff(*file_count, 50);
        group.throughput(Throughput::Bytes(diff.len() as u64));

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}_files", file_count)),
            &diff,
            |b, diff| {
                let processor = create_processor(10_000);
                b.iter(|| processor.process(black_box(diff)));
            },
        );
    }

    group.finish();
}

fn bench_process_large_diff(c: &mut Criterion) {
    let mut group = c.benchmark_group("process/large_diff");

    for line_count in [1000, 5000, 10_000, 50_000].iter() {
        let diff = generate_large_text_diff(*line_count);
        group.throughput(Throughput::Bytes(diff.len() as u64));

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}_lines", line_count)),
            &diff,
            |b, diff| {
                let processor = create_processor(1_000);
                b.iter(|| processor.process(black_box(diff)));
            },
        );
    }

    group.finish();
}

fn bench_process_with_token_limits(c: &mut Criterion) {
    let diff = generate_text_diff(10, 100);
    let mut group = c.benchmark_group("process/token_limits");
    group.throughput(Throughput::Bytes(diff.len() as u64));

    for limit in [100, 500, 1000, 5000, 10_000].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("limit_{}", limit)),
            &diff,
            |b, diff| {
                let processor = create_processor(*limit);
                b.iter(|| processor.process(black_box(diff)));
            },
        );
    }

    group.finish();
}

fn bench_process_safe_mixed_content(c: &mut Criterion) {
    let mut diff = generate_text_diff(5, 50);
    diff.push_str(&generate_binary_diff(2));
    diff.push_str(&generate_binary_extension_diff(3));

    let mut group = c.benchmark_group("process_safe/mixed_content");
    group.throughput(Throughput::Bytes(diff.len() as u64));

    group.bench_function("5_text_2_binary_3_extensions", |b| {
        let processor = create_processor(10_000);
        b.iter(|| processor.process_safe(black_box(&diff)));
    });

    group.finish();
}

fn bench_process_deletion_only(c: &mut Criterion) {
    let mut diff = String::new();
    diff.push_str("diff --git a/deleted.txt b/deleted.txt\n");
    diff.push_str("deleted file mode 100644\n");
    diff.push_str("--- a/deleted.txt\n");
    diff.push_str("+++ /dev/null\n");
    diff.push_str("@@ -1,1000 +0,0 @@\n");
    for i in 0..1000 {
        diff.push_str(&format!("-Line to be deleted {}\n", i));
    }

    let mut group = c.benchmark_group("process/deletion_only");
    group.throughput(Throughput::Bytes(diff.len() as u64));

    group.bench_function("1000_deletions", |b| {
        let processor = create_processor(10_000);
        b.iter(|| processor.process(black_box(&diff)));
    });

    group.finish();
}

fn bench_process_with_ignore_patterns(c: &mut Criterion) {
    let mut diff = generate_text_diff(5, 50);
    // Add lockfiles
    for ext in ["Cargo.lock", "package-lock.json", "yarn.lock"].iter() {
        diff.push_str(&format!("diff --git a/{} b/{}\n", ext, ext));
        diff.push_str("index 1234567..abcdefg 100644\n");
        diff.push_str(&format!("--- a/{}\n", ext));
        diff.push_str(&format!("+++ b/{}\n", ext));
        diff.push_str("@@ -1,100 +1,100 @@\n");
        for i in 0..100 {
            diff.push_str(&format!("+package-{} = \"1.0.0\"\n", i));
        }
    }

    let mut group = c.benchmark_group("process/with_ignore_patterns");
    group.throughput(Throughput::Bytes(diff.len() as u64));

    group.bench_function("with_lockfiles", |b| {
        let tokenizer: Arc<dyn Tokenizer> = Arc::new(DeterministicTokenizer);
        let processor = DiffProcessor::new(tokenizer, TokenCount::new_saturating(10_000))
            .with_ignore_files(vec![
                "Cargo.lock".to_string(),
                "package-lock.json".to_string(),
                "yarn.lock".to_string(),
            ]);
        b.iter(|| processor.process(black_box(&diff)));
    });

    group.finish();
}

fn bench_unicode_in_diffs(c: &mut Criterion) {
    let mut diff = String::new();
    diff.push_str("diff --git a/unicode.txt b/unicode.txt\n");
    diff.push_str("index 1234567..abcdefg 100644\n");
    diff.push_str("--- a/unicode.txt\n");
    diff.push_str("+++ b/unicode.txt\n");
    diff.push_str("@@ -1,1000 +1,1000 @@\n");
    for i in 0..1000 {
        diff.push_str(&format!("+Line {} with emoji 🚀👋🌍 and résumé café\n", i));
    }

    let mut group = c.benchmark_group("process/unicode");
    group.throughput(Throughput::Bytes(diff.len() as u64));

    group.bench_function("emoji_and_accents", |b| {
        let processor = create_processor(10_000);
        b.iter(|| processor.process(black_box(&diff)));
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_binary_detection_text,
    bench_binary_detection_nul_bytes,
    bench_binary_detection_extensions,
    bench_binary_detection_late_nul_byte,
    bench_binary_detection_large_file_sampling,
    bench_process_small_diff,
    bench_process_large_diff,
    bench_process_with_token_limits,
    bench_process_safe_mixed_content,
    bench_process_deletion_only,
    bench_process_with_ignore_patterns,
    bench_unicode_in_diffs,
);
criterion_main!(benches);
