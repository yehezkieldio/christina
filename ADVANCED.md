# Advanced Usage Guide

This guide is for power users who want to fine-tune Christina's behavior or integrate it into complex workflows.

## Fine-Grained Pipeline Control

You can tweak the internal thresholds of the Map-Reduce pipeline in your `config.toml`.

### Partial Failure Rate
In the Map phase, some chunk summaries might fail (due to rate limits or malformed diffs).
- `max_partial_failure_rate` (Default: `0.10`): If more than 10% of chunks fail, Christina will abort the generation to prevent poor-quality output.
- `prompt_failure_rate_threshold` (Default: `0.05`): If failure exceeds 5%, a warning is shown in the trace summary.

### Concurrency
- `max_concurrent_requests` (Default: `4`): Controls parallel LLM calls. Increase this if you have high rate limits and want faster processing of massive (100+ file) commits. Capped at 10.

## Secure Secret Management

Christina supports three modes for API keys:

1.  **Plaintext** (Not Recommended):
    ```toml
    api_key = { value = "sk-..." }
    ```
2.  **Environment Variables**:
    ```toml
    api_key = { env = "AZURE_OPENAI_API_KEY" }
    ```
3.  **OS Keyring** (Recommended):
    ```toml
    api_key = { keyring = "christina.openai" }
    ```
    You can set keys via the CLI:
    ```bash
    christina profile create my-prod --api-key "keyring:christina.prod"
    ```

## Environment Overrides

Every configuration value can be overridden via environment variables. This is useful for temporary one-off runs or CI/CD pipelines.

| Setting | Env Var |
|:--- |:--- |
| Active Profile | `CHRISTINA_ACTIVE_PROFILE` |
| Model Name | `CHRISTINA_MODEL` |
| Concurrency Limit | `CHRISTINA_CONCURRENCY_LIMIT` |
| Max Input Tokens | `CHRISTINA_TOKENS_MAX_INPUT` |
| Temperature | `CHRISTINA_MODEL_TEMPERATURE` |

## CI/CD Integration

Christina can be used non-interactively using the `--yes` flag.

```bash
# In a CI environment
christina --yes --dry-run
```

**Note**: Ensure your environment has the `CHRISTINA_MODEL_API_KEY` set.

## Custom Token Budgets

If you are using a model with an unconventional context window (e.g., a massive 1M token window), you should adjust the profile:

```toml
[profiles.massive-context]
max_input_tokens = 1000000
max_output_tokens = 8192
```

Christina will automatically scale its `HISTORY_BUDGET_FRACTION` (15% of input) to accommodate more style reference commits.

## Cross-References

- [Technical Specification](SPECIFICATION.md): Full list of configuration keys.
- [Generation Pipeline](GENERATION_PIPELINE.md): Context for pipeline settings.
