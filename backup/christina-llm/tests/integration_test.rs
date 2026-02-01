#![allow(unused_crate_dependencies)]
#![allow(clippy::panic)]

use christina_core::git::DiffChunk;
use christina_core::types::TokenCount;
use christina_llm::orchestrator::AIOrchestrator;
use christina_llm::provider::Provider;
use std::sync::Arc;

fn sample_chunk() -> DiffChunk {
    DiffChunk::new(
        Arc::from("diff --git a/lib.rs b/lib.rs\n+new line\n"),
        vec![christina_core::types::FilePath::from("lib.rs")],
        TokenCount::new_saturating(5),
    )
}

#[tokio::test]
async fn end_to_end_generation_with_mock_provider() {
    let provider = Arc::new(Provider::mock_sequence(vec![Ok(
        "feat(core): integrate pipeline".to_string(),
    )]));
    let orchestrator = AIOrchestrator::new(provider);

    let result = orchestrator
        .generate_commit_message(
            vec![sample_chunk()],
            None,
            christina_core::types::commit_message::ValidationMode::default(),
            None,
            None,
        )
        .await
        .unwrap_or_else(|e| panic!("generation should succeed: {}", e));

    assert_eq!(result.message.as_ref(), "feat(core): integrate pipeline");
    assert_eq!(result.total_chunks, 1);
}
