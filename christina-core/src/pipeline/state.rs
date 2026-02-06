//! Pipeline state machine for commit message generation.
//!
//! Tracks the progression: Empty -> Analyzing -> Synthesizing -> Complete.

/// Represents the current state of the generation pipeline.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum PipelineState {
    /// No diff loaded yet.
    #[default]
    Empty,
    /// Diff loaded and being analyzed (chunking, summarizing).
    Analyzing {
        /// Total number of chunks to process.
        total_chunks: usize,
        /// Number of chunks completed so far.
        completed_chunks: usize,
    },
    /// Summaries collected, synthesizing final commit message.
    Synthesizing,
    /// Generation complete.
    Complete,
    /// Generation failed.
    Failed {
        /// Error description.
        reason: String,
    },
}

impl PipelineState {
    /// Create a new Analyzing state.
    pub fn analyzing(total_chunks: usize) -> Self {
        Self::Analyzing {
            total_chunks,
            completed_chunks: 0,
        }
    }

    /// Advance the analyzing state by one chunk.
    ///
    /// # Panics
    ///
    /// Panics if not in the Analyzing state (invariant violation).
    pub fn advance_chunk(&mut self) {
        match self {
            Self::Analyzing {
                completed_chunks,
                total_chunks,
            } => {
                assert!(
                    *completed_chunks < *total_chunks,
                    "Cannot advance beyond total chunks"
                );
                *completed_chunks += 1;
            }
            _ => unreachable!("advance_chunk called on non-Analyzing state"),
        }
    }

    /// Check if all chunks have been processed.
    pub fn all_chunks_done(&self) -> bool {
        matches!(
            self,
            Self::Analyzing {
                total_chunks,
                completed_chunks,
            } if completed_chunks == total_chunks
        )
    }

    /// Check if the pipeline is in a terminal state.
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Complete | Self::Failed { .. })
    }
}



#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_empty() {
        assert_eq!(PipelineState::default(), PipelineState::Empty);
    }

    #[test]
    fn analyzing_state_creation() {
        let state = PipelineState::analyzing(5);
        assert_eq!(
            state,
            PipelineState::Analyzing {
                total_chunks: 5,
                completed_chunks: 0,
            }
        );
    }

    #[test]
    fn advance_chunk_increments() {
        let mut state = PipelineState::analyzing(3);
        state.advance_chunk();
        assert_eq!(
            state,
            PipelineState::Analyzing {
                total_chunks: 3,
                completed_chunks: 1,
            }
        );
    }

    #[test]
    fn all_chunks_done_when_complete() {
        let mut state = PipelineState::analyzing(2);
        assert!(!state.all_chunks_done());
        state.advance_chunk();
        assert!(!state.all_chunks_done());
        state.advance_chunk();
        assert!(state.all_chunks_done());
    }

    #[test]
    fn is_terminal_states() {
        assert!(!PipelineState::Empty.is_terminal());
        assert!(!PipelineState::analyzing(1).is_terminal());
        assert!(!PipelineState::Synthesizing.is_terminal());
        assert!(PipelineState::Complete.is_terminal());
        assert!(PipelineState::Failed {
            reason: "test".to_string()
        }
        .is_terminal());
    }

    #[test]
    #[should_panic(expected = "Cannot advance beyond total chunks")]
    fn advance_chunk_panics_when_done() {
        let mut state = PipelineState::analyzing(1);
        state.advance_chunk();
        state.advance_chunk(); // Should panic
    }

    #[test]
    #[should_panic(expected = "advance_chunk called on non-Analyzing state")]
    fn advance_chunk_panics_on_wrong_state() {
        let mut state = PipelineState::Empty;
        state.advance_chunk();
    }
}
