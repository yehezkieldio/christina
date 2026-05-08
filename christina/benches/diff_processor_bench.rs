#![allow(unused_crate_dependencies, clippy::unwrap_used)]

use std::hint::black_box;
use std::sync::Arc;

use christina::git::{diff_processor::DiffProcessor, parsing};
use christina_core::test_helpers::DeterministicTokenizer;
use christina_core::types::TokenCount;
use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};

fn generate_diff(files: usize, lines_per_file: usize) -> String {
    let mut diff = String::with_capacity(files * lines_per_file * 64);

    for file in 0..files {
        diff.push_str(&format!(
            "diff --git a/src/file_{file}.rs b/src/file_{file}.rs\n"
        ));
        diff.push_str("index 1111111..2222222 100644\n");
        diff.push_str(&format!("--- a/src/file_{file}.rs\n"));
        diff.push_str(&format!("+++ b/src/file_{file}.rs\n"));
        diff.push_str("@@ -1,3 +1,3 @@\n");

        for line in 0..lines_per_file {
            diff.push_str("-let old_value = ");
            diff.push_str(&line.to_string());
            diff.push_str(";\n");
            diff.push_str("+let new_value = ");
            diff.push_str(&(line + file).to_string());
            diff.push_str(";\n");
        }
    }

    diff
}

fn bench_split_by_files(c: &mut Criterion) {
    let tokenizer = DeterministicTokenizer;
    let mut group = c.benchmark_group("split_by_files");

    for files in [10usize, 100, 500] {
        let diff = generate_diff(files, 12);
        group.throughput(Throughput::Bytes(diff.len() as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{files}_files")),
            &diff,
            |b, diff| {
                b.iter(|| parsing::split_by_files(black_box(diff), black_box(&tokenizer)));
            },
        );
    }

    group.finish();
}

fn bench_process_safe(c: &mut Criterion) {
    let tokenizer = Arc::new(DeterministicTokenizer);
    let processor = DiffProcessor::new(tokenizer, TokenCount::new_at_least_one(2_000));
    let mut group = c.benchmark_group("diff_processor_process_safe");

    for files in [10usize, 100, 500] {
        let diff = generate_diff(files, 12);
        group.throughput(Throughput::Bytes(diff.len() as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{files}_files")),
            &diff,
            |b, diff| {
                b.iter(|| processor.process_safe(black_box(diff)));
            },
        );
    }

    group.finish();
}

criterion_group!(benches, bench_split_by_files, bench_process_safe);
criterion_main!(benches);
