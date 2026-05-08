#![allow(unused_crate_dependencies, clippy::unwrap_used)]

use std::hint::black_box;
use std::sync::Arc;

use christina_core::test_helpers::DeterministicTokenizer;
use christina_core::types::{FileDiff, FilePath, TokenCount};
use christina_core::{Tokenizer, processing};
use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};

fn file_diff(index: usize, lines: usize, tokenizer: &DeterministicTokenizer) -> FileDiff {
    let path = FilePath::from(format!("src/file_{index}.rs"));
    let mut content = String::with_capacity(lines * 48);
    content.push_str(&format!(
        "diff --git a/src/file_{index}.rs b/src/file_{index}.rs\n"
    ));
    content.push_str("index 1111111..2222222 100644\n");
    content.push_str(&format!("--- a/src/file_{index}.rs\n"));
    content.push_str(&format!("+++ b/src/file_{index}.rs\n"));
    content.push_str("@@ -1,3 +1,3 @@\n");

    for line in 0..lines {
        content.push_str("-let old_value = ");
        content.push_str(&line.to_string());
        content.push_str(";\n");
        content.push_str("+let new_value = ");
        content.push_str(&(line + index).to_string());
        content.push_str(";\n");
    }

    let content: Arc<str> = Arc::from(content);
    let token_count = tokenizer.count_tokens(&content);
    FileDiff {
        path,
        content,
        token_count,
        truncated: false,
    }
}

fn bench_split_recursive(c: &mut Criterion) {
    let tokenizer = DeterministicTokenizer;
    let mut group = c.benchmark_group("split_recursive");

    for file_count in [10usize, 100, 500] {
        let files = (0..file_count)
            .map(|index| file_diff(index, 12, &tokenizer))
            .collect::<Vec<_>>();
        let bytes = files.iter().map(|file| file.content.len()).sum::<usize>();

        group.throughput(Throughput::Bytes(bytes as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{file_count}_files")),
            &files,
            |b, files| {
                b.iter(|| {
                    processing::split_recursive(
                        black_box(files.clone()),
                        TokenCount::new_at_least_one(2_000),
                        black_box(&[]),
                        TokenCount::new_at_least_one(processing::LOCKFILE_TOKEN_LIMIT),
                        black_box(&tokenizer),
                    )
                });
            },
        );
    }

    group.finish();
}

criterion_group!(benches, bench_split_recursive);
criterion_main!(benches);
