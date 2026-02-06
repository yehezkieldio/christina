# Proposal: Comprehensive Project Reorganization for Christina

This document outlines a structural rewrite for Christina, optimized for modularity, extreme performance, and a clear distinction between generic LLM providers and specialized AI integrations (like Copilot).

It adheres strictly to the Rust mental model defined in [AGENTS.md](AGENTS.md): **Data-oriented, linear pipelines, and types as invariants.**

---

## High-Level Architecture: The Two-Crate Split

### 1. `christina-core` (The Domain & Pure Logic)
**Role**: A headless library containing the "grammar" and "mechanics" of the application. It has zero awareness of CLI arguments or terminal state.

```text
christina-core/
├── src/
│   ├── lib.rs              # Re-exports stable public API
│   ├── error.rs            # Unified domain error (thiserror)
│   ├── types/              # Zero-cost domain invariants
│   │   ├── mod.rs
│   │   ├── commit.rs       # Conventional Commit validation & parsing
│   │   ├── diff.rs         # Structured diff representation
│   │   ├── path.rs         # Sanitized relative file paths
│   │   ├── tokens.rs       # Budget-aware token counts
│   │   └── backend_id.rs   # Unique identifiers for profiles/engines
│   ├── git/                # Headless Git protocols
│   │   ├── mod.rs
│   │   ├── repository.rs   # Repo root discovery & health checks
│   │   ├── stage.rs        # Analysis of staged index
│   │   └── diff_gen.rs     # Ownership-aware diff generation
│   ├── processing/         # Data transformation (Hot Path)
│   │   ├── mod.rs
│   │   ├── chunking.rs     # Map-Reduce strategy for huge diffs
│   │   ├── tokenizer.rs    # tiktoken-rs wrap with caching
│   │   └── context.rs      # User-provided context merging
│   ├── pipeline/           # The "Generation Engine" Protocol
│   │   ├── mod.rs
│   │   ├── backend.rs      # TRAIT: The abstract interface for AI
│   │   └── state.rs        # Pipeline states (Empty -> Analyzing -> Synthesizing)
│   └── prompt/             # Pure template generation
│       ├── mod.rs
│       ├── templates.rs    # Embedded prompt assets
│       └── builder.rs      # Prompt composition from diff/summaries
```

### 2. `christina` (The Implementation & CLI)
**Role**: The "orchestrator" and "user interface". It implements the concrete AI engines and handles I/O (files, network, terminal).

```text
christina/
├── src/
│   ├── main.rs             # Entry: Allocator, env, signals, main loop
│   ├── cli/                # Terminal interface (clap)
│   │   ├── mod.rs          # CLI Router
│   │   ├── commit.rs       # `christina` (default command)
│   │   ├── config.rs       # `christina config`
│   │   └── profile.rs      # `christina profile`
│   ├── config/             # App-specific persistent state
│   │   ├── mod.rs          # Resolver (XDG paths, env vars)
│   │   ├── settings.rs     # Global settings
│   │   ├── profiles.rs     # Profile definitions
│   │   └── secrets/        # Secure storage (Keyring integration)
│   ├── engines/            # CONCRETE AI IMPLEMENTATIONS
│   │   ├── mod.rs          # Engine Factory & Feature Switching
│   │   ├── standard_llm/   # [feature = "engine-llm"]
│   │   │   ├── mod.rs      # Based on the `llm` crate
│   │   │   ├── openai.rs
│   │   │   ├── azure.rs
│   │   │   └── groq.rs
│   │   └── copilot/        # [feature = "engine-copilot"]
│   │       ├── mod.rs      # Experimental Copilot SDK engine
│   │       └── auth.rs     # Specialized Copilot auth flow
│   ├── orchestrator/       # The Pipeline Runner
│   │   ├── mod.rs          # Async Map-Reduce logic
│   │   ├── throttle.rs     # Rate limiting & concurrency
│   │   └── retry.rs        # Backoff logic
│   ├── telemetry/          # Observability (tracing)
│   │   ├── mod.rs          # Subscriber setup
│   │   └── filters.rs      # Privacy-aware log filtering
│   └── ui/                 # Terminal feedback
│       ├── mod.rs          # UI Root
│       ├── events.rs       # Event bus (Processing -> UI)
│       └── components/     # Spinners, diff views, message editor
```

---

## Refined AI Engine Integration (Feature Gated)

We decouple the "AI Engine" (the high-level library/SDK integration) from the "Provider" (the specific endpoint).

### 1. The Strategy: Hierarchical Selection
1.  **Engine Level**: Choose between `standard-llm` (using standard API keys) or `copilot` (using GitHub token/OAuth).
2.  **Implementation Level**: Within `standard-llm`, choose the provider (OpenAI vs Azure).

### 2. Feature Gating in `Cargo.toml`
```toml
[features]
default = ["engine-llm", "openai"]

# High-level engine selection
engine-llm = ["dep:llm", "dep:reqwest"]
engine-copilot = ["dep:copilot-sdk"] # Mutually exclusive possibility

# Providers for the 'engine-llm'
openai = ["llm/openai"]
azure = ["llm/azure_openai"]
groq = ["llm/groq"]
```

### 3. The `AiBackend` Trait in `core`
The core only cares that something can turn a request into a result.
```rust
// christina-core/src/pipeline/backend.rs
pub trait AiBackend: Send + Sync {
    async fn generate(&self, request: GenerationRequest) -> Result<String, BackendError>;
}
```

### 4. Engine Resolution in `christina`
```rust
// christina/src/engines/mod.rs
pub enum Engine {
    #[cfg(feature = "engine-llm")]
    Llm(LlmEngine),

    #[cfg(feature = "engine-copilot")]
    Copilot(CopilotEngine),
}

impl AiBackend for Engine {
    async fn generate(&self, req: GenerationRequest) -> Result<String, BackendError> {
        match self {
            #[cfg(feature = "engine-llm")]
            Self::Llm(e) => e.generate(req).await,
            #[cfg(feature = "engine-copilot")]
            Self::Copilot(e) => e.generate(req).await,
        }
    }
}
```

---

## Detailed Component Responsibilities

### Processing & Chunking (`core`)
*   Optimized for **data-oriented design**.
*   The `DiffChunk` is a cheap reference or `Arc` wrapper to avoid copying large files during the Map phase.
*   `ChunkingStrategy` is a pure function that takes `StagedFiles` and `TokenLimits` and returns a plan for parallelization.

### Secret Management (`christina/config/secrets/`)
*   Keyring interaction is isolated.
*   Secrets are never stored in plain text configuration files.
*   Uses a "Late-binding" approach: the CLI only fetches the secret from the keyring right before the engine needs it.

### UI Event Bus (`christina/ui/events.rs`)
*   The `Orchestrator` sends progress events (e.g., `ChunkProcessed(3/10)`, `IntentExtracted`) to the UI.
*   This keeps the generation logic non-blocking and decoupled from specific `indicatif` or `ratatui` implementations.

---

## Philosophy & Quality Constraints

1.  **No cyclic dependencies**: Flow is always `christina` -> `christina-core`.
2.  **Concrete over Abstract**: Traits are only used for the AI Backend (where polymorphism is required) or Git provider (to allow mocking). Everything else is concrete data structures.
3.  **Linear Pipelines**: The `main` run loop looks like a simple functional chain:
    *   `load_config() -> gather_diff() -> plan_chunks() -> run_map_reduce() -> finalize_message()`.
4.  **Ownership moves forward**: Diffs are converted into Chunk Summaries, which are converted into Intent Themes, which are converted into a Final Message. Older data is dropped as soon as its information is consumed.

---

## Migration Roadmap & Gap Analysis

Based on the current codebase state, the following gaps must be addressed to reach the target architecture.

### Phase 1: Core Logic Extraction (The "Hot Path")
*   **Move `chunking.rs`**: Relocate from `christina/src/io/git/chunking.rs` to `christina-core/src/processing/chunking.rs`.
*   **Move `tokenizer.rs`**: Relocate from `christina/src/io/llm/tokenizer.rs` to `christina-core/src/processing/tokenizer.rs`.
*   **Establish `AiBackend`**: Define the trait in `christina-core/src/pipeline/backend.rs`.
*   **Prompt Restructuring**: Break down `christina-core/src/prompt.rs` into the `src/prompt/` directory structure with modular templates.

### Phase 2: Orchestration & Engines Refactor
*   **Flatten `io/`**: Eliminate `christina/src/io/` by promoting `llm/orchestrator.rs` to `christina/src/orchestrator/`.
*   **Engine Decoupling**: Implement the `engines/` hierarchy in `christina`, moving existing OpenAI/Azure/Groq logic into `engines/standard_llm/`.
*   **Secret Migration**: Move secret management from `christina-core/src/config/secret.rs` to `christina/src/config/secrets/`.

### Phase 3: CLI & UI Alignment
*   **UI Promotion**: Move `christina/src/cli/ui.rs` to a top-level `christina/src/ui/` module and implement the UI Event Bus.
*   **Telemetry**: Implement `christina/src/telemetry/` to centralize tracing and logging initialization currently scattered in `main.rs`.
*   **Feature Gating**: Update `christina/Cargo.toml` with the proposed `engine-llm` and `engine-copilot` feature markers.

### Phase 4: Experimental Features
*   **Copilot Support**: Implement the `engines/copilot/` module and the necessary OAuth/GitHub token authentication flow.
*   **JSON Schema**: Finalize the transition of `generate_json_schema.rs` to a dedicated maintenance tool in `core`.
