# Advanced Usage Guide

This guide is for power users who want to fine-tune Christina's behavior or integrate it into complex workflows.

## Fine-Grained Pipeline Control

You can tweak the internal thresholds of the Map-Reduce pipeline in your `config.toml`.

### Partial Failure Rate
In the Map phase, some chunk summaries might fail (due to rate limits or malformed diffs).
- `max_partial_failure_rate` (Default: `0.10`): If more than 10% of chunks fail, Christina will abort the generation to prevent poor-quality output.
- `prompt_failure_rate_threshold` (Default: `0.05`): If failure exceeds 5%, a warning is shown in the trace summary.

### Concurrency
- `max_concurrent_requests`: Controls parallel LLM calls. The default is hardware-aware and capped conservatively; explicit config and env overrides are clamped to `1..=20`.

## Secure Secret Management

Christina supports two modes for API keys:

1.  **Plaintext** (Not Recommended):
    ```toml
    api_key = { value = "sk-..." }
    ```
2.  **Environment Variables**:
    ```toml
    api_key = { env = "AZURE_OPENAI_API_KEY" }
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
