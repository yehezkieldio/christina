# ADR 003: Provider-Agnostic LLM Interface

## Status
Accepted

## Context
The LLM ecosystem is fragmented. Users may want to use OpenAI, Azure, Groq, Anthropic, or local models (via Ollama/Llama.cpp). Hardcoding a specific provider's API would limit the tool's adoption and flexibility.

## Decision
We defined a `Provider` trait (and corresponding enum dispatch) in `christina-core`. This interface abstracts away:
- Request/Response formats.
- Authentication mechanisms (API Keys vs. Bearer Tokens).
- Endpoint construction.
- Concurrency and retry logic (managed by the `AIOrchestrator`).

## Consequences
- **Pros**:
    - Extensibility: Adding a new provider only requires implementing the `Provider` trait.
    - Consistency: The rest of the application interacts with a unified interface.
    - Portability: Users can switch providers by simply changing a configuration profile.
- **Cons**:
    - Least Common Denominator: Some provider-specific features might be hard to expose through a generic interface.
    - Maintenance: Changes in any provider's API require updates in the core.
