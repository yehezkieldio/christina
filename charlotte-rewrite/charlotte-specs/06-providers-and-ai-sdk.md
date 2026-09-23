# Providers and the AI SDK

## Current state

Christina speaks to exactly one provider: Azure OpenAI, through `christina/src/engines/default/mod.rs`. The `Provider` enum wraps request construction, endpoint parsing (`parse_azure_endpoint`, `normalize_azure_endpoint`), and response handling for that one provider. Structured output comes from Azure's response-format parameter combined with roughly 250 lines of hand-written JSON repair in `christina/src/orchestrator/mod.rs`: `extract_json`, `extract_json_simplified`, `extract_from_markdown`, `extract_with_streaming_parser`, and `extract_json_with_escape_handling`. This repair logic exists because a model sometimes wraps JSON in markdown fencing, adds commentary before or after the JSON, or emits malformed escapes.

## Target state

Charlotte uses the Vercel AI SDK's `generateObject` function with a Zod schema for every structured call: chunk summary, hierarchical intent, sub-theme aggregation, and the final commit message. Each of Christina's response structs, `SummaryResponse`, `ThemeResponse`, `CommitResponse`, and `SubTheme`, becomes one Zod schema in `packages/schemas`. This removes the need for Christina's hand-written extraction functions, because `generateObject` validates the model's output against the schema and retries or repairs internally.

## Provider list

Charlotte's provider list widens past Azure OpenAI to match what the AI SDK ships providers for: OpenAI, Azure OpenAI, Anthropic, Google, and any OpenAI-compatible endpoint (for self-hosted or local models such as Ollama). A `ProviderProfile.provider` value selects one of these at runtime, matching the profile model in `03-config-and-profiles.md`.

## Open decision: structured output reliability

Christina's JSON repair logic is not incidental complexity. It exists because Azure OpenAI's structured output guarantee was not reliable enough on its own for Christina's authors. Before Charlotte drops this logic in favor of `generateObject`, run one step first. Send a deliberately non-compliant prompt against each target provider. Use a prompt that tempts the model into markdown fencing or commentary. Then make sure that the AI SDK's own repair path recovers a valid object. If any provider fails this step, Charlotte keeps a thin fallback repair layer for that provider instead of assuming schema validation alone is enough.

## Retry and rate limiting

Christina's retry policy (`christina/src/orchestrator/retry.rs`) uses exponential backoff with full jitter. Its throttle (`christina/src/orchestrator/throttle.rs`) uses a token-bucket limiter that bounds both concurrent requests and requests per second. Neither algorithm is provider-specific, so both become plain TypeScript functions in `packages/providers/retry.ts` and `packages/providers/throttle.ts`, independent of which AI SDK provider is active.

## Request identity

Christina tags each request with an incrementing counter (`REQUEST_ID_COUNTER`) for trace correlation. Charlotte replaces this with the session transcript's own event ID, described in `08-session-storage-and-stats.md`. Request correlation and the persistent log then use one identifier instead of two.
