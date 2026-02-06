# Component Spec: AI Orchestrator

## Overview
The `AIOrchestrator` (located in `christina/src/io/llm/orchestrator.rs`) is the "brain" of the LLM interaction layer. It is responsible for taking a prepared request and ensuring it is fulfilled successfully by the configured provider.

## Responsibilities

### 1. Concurrency Management
The orchestrator uses `tokio` to handle requests asynchronously. While currently most interactions are sequential (one commit message per run), the architecture supports concurrent requests for features like "Generate 3 options" or "Simultaneous multi-model evaluation".

### 2. Retry Logic
It implements an exponential backoff retry strategy for transient errors (e.g., 503 Service Unavailable, 429 Rate Limit).
- **Default Retries**: 3
- **Base Delay**: 1s
- **Multiplier**: 2.0

### 3. Token Budgeting Enforcement
Before sending a request, the orchestrator validates that the payload fits within the `UsageTier` limits defined for the active profile. It works closely with the `DiffProcessor` to ensure budgets are respected.

### 4. Provider Dispatch
It maps the abstract `LlmRequest` to provider-specific payloads and parses the responses back into the domain `CommitMessage` type.

## Key Types
- `AIOrchestrator`: The main struct holding the provider instance and configuration.
- `LlmRequest`: A domain-neutral representation of the prompt, diff, and history.
- `LlmResponse`: A domain-neutral representation of the AI's output.

## Error Handling
The orchestrator translates provider-specific errors (e.g., `reqwest::Error`) into domain errors defined in `christina-core/src/error.rs`, allowing the UI to provide meaningful feedback (e.g., "API Key Invalid" instead of "401 Unauthorized").
