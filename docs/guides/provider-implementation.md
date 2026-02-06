# Developer Guide: Implementing a New LLM Provider

Adding a new provider (e.g., Anthropic, Ollama) involves three main steps.

## Step 1: Define the Provider Kind
In `christina-core/src/types/provider_kind.rs`, add your provider to the `ProviderKind` enum:

```rust
pub enum ProviderKind {
    OpenAI,
    Azure,
    Groq,
    Anthropic, // Add this
}
```

## Step 2: Implement the Provider Interface
Create a new file in `christina/src/io/llm/` (e.g., `anthropic.rs`). Implement the necessary logic to:
1. **Construct the Request**: Map `LlmRequest` to the provider's JSON format.
2. **Execute the HTTP Call**: Use `reqwest` to send the payload.
3. **Parse the Response**: Extract the generated text and token usage.

```rust
pub struct AnthropicProvider {
    config: ResolvedProfile,
}

impl Provider for AnthropicProvider {
    async fn generate(&self, request: LlmRequest) -> Result<LlmResponse> {
        // Implementation here
    }
}
```

## Step 3: Register in the Orchestrator
Update the `AIOrchestrator` factory method in `christina/src/io/llm/orchestrator.rs` to instantiate your new provider based on the profile configuration.

## Step 4: Configuration and Secrets
If the provider requires specific configuration (like a `resource_name` for Azure), add it to the `Profile` struct in `christina-core/src/config/`.

Ensure any new secrets (API keys) are handled via the `Secret` type to remain secure in the system keyring.

## Testing
1. Add a unit test in your `anthropic.rs` using a mocked HTTP client.
2. Run `just check` to ensure type safety.
3. Use the CLI with a temporary profile to verify integration:
   `christina profile create my-anthropic --provider anthropic --api-key env:ANTHROPIC_API_KEY`
