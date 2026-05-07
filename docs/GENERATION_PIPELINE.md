# Generation Pipeline

This document provides a technical walkthrough of the data transformation steps required to turn a Git diff into a validated commit message.

## Pipeline Overview

The pipeline follows a linear data flow: Ingestion -> Transformation -> Synthesis -> Consumption.

### Step 1: Ingestion
- **Module**: `christina/src/git/adapter.rs`
- **Action**: Staged changes are extracted from the index.
- **Output**: A collection of `GitFile` objects containing relative paths, status codes, and raw patch content.

### Step 2: Intelligent Chunking
- **Module**: `christina-core/src/processing/chunking.rs`
- **Action**: Recursive partitioning based on token limits.
- **Hierarchy**:
  1.  **File Level**: Try to keep entire files together.
  2.  **Hunk Level**: Split by `@@` markers if a single file is too large.
  3.  **Line Level**: Sliced as a last resort if a single hunk exceeds the context window.
- **Optimization**: Lockfiles (e.g., `Cargo.lock`) are truncated to 100 tokens to preserve context budget for source code.

### Step 3: Parallel Map Phase
- **Module**: `christina/src/orchestrator/mod.rs`
- **Action**: Each chunk is summarized independently.
- **Prompt**: `SUMMARY_PROMPT`
- **Goal**: Extract atomic technical facts from the diff fragment (e.g., "Added JWT validation middleware").

### Step 4: Intent Extraction
- **Module**: `christina/src/orchestrator/mod.rs`
- **Action**: Group atomic summaries into architectural themes.
- **Prompt**: `INTENT_EXTRACTION_PROMPT`
- **Scope Inference**: The AI calculates module dominance (e.g., if 70% of changes are in `auth/`, the scope is `auth`).

### Step 5: Reduce Phase (Synthesis)
- **Module**: `christina/src/orchestrator/mod.rs`
- **Action**: Synthesize themes into a cohesive Conventional Commit header.
- **Prompt**: `THEME_SYNTHESIS_PROMPT`
- **Rules**: Apply "anti-slop" verbiage rules (e.g., verboten words like "various", "improve", "ensure").

### Step 6: Validation & Salvage
- **Module**: `christina-core/src/types/commit.rs`
- **Action**: Verify the message against the regex pattern.
- **Salvage**: If the LLM includes a preamble (e.g., "Here is the commit message:"), the engine attempts to extract the valid header from the text.

## Fast-Path Optimization

For small commits (single chunk), the orchestrator skips the Map and Intent phases, proceeding directly to **Direct Generation**. This reduces total API calls and latency for trivial changes.

## Cross-References

- [Architecture](ARCHITECTURE.md): System decomposition.
- [Performance Optimization](PERFORMANCE_OPTIMIZATION.md): Hot-path details.
