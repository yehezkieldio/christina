# Session storage and stats

## Why this exists

Christina has a `--trace` flag that prints stage output during one run, but it keeps no record after the process exits. Charlotte adds a persistent record, in the style of the transcript files used by Claude Code, Codex, and opencode. This record backs a stats command and replaces `--trace` as the source of debugging detail.

## Storage location

Each run writes one file under the XDG data directory: `~/.local/share/charlotte/sessions/<session-id>.jsonl` on Linux, with the matching OS-appropriate path elsewhere. The file format is JSON Lines: one JSON object per line, appended as the run proceeds, never rewritten in place. The process can stop before the next line, and every line already written still stays valid. This format survives a crash mid-run.

## Event shape

Every line carries a `type` field and a `timestamp` field. Seven event types exist. `run_start` carries a config summary and the repository path. `stage_start` and `stage_end` mark one of the six pipeline stages named in `01-architecture.md`. `request` carries a `requestId`, the provider, model, and prompt token count; `response` carries the same `requestId`, the completion token count, latency, and — when the provider reported either — two subsets of the paired request's prompt tokens: `cacheReadTokens` (served from that provider's prompt cache, e.g. Anthropic's cache reads) and `cacheWriteTokens` (written into it on this call, e.g. Anthropic's cache creation). The two are kept apart rather than folded into one "cached tokens" count because they price in opposite directions — a cache write costs more than a fresh input token, a cache read costs much less — so collapsing them would make a later cost pass wrong in both directions at once. `requestId` is the request-correlation id `06-providers-and-ai-sdk.md` calls for: the map phase issues several calls concurrently, so a `request`/`response` pair cannot be matched by adjacency alone. `retry` carries the attempt number and reason. `warning` carries the structured warning data from `07-orchestrator-pipeline.md`. `run_end` carries the outcome, total token usage (including total cache read/write tokens, when reported), and final message length.

## Secret handling

No event can carry an API key, a resolved secret value, or the raw diff content by default. `03-config-and-profiles.md` sets the same rule for config output. A future verbose mode can opt into including diff content in the transcript. The default excludes it, because the transcript file persists on disk after the run ends.

## Stats command

`charlotte stats` reads the session files and reports token usage grouped by day, by provider, and by model. A first version needs only total input tokens, total output tokens, cache read/write tokens, and a run count per group. Cost estimation depends on per-provider pricing data, which changes over time, so cost display stays a fast-follow rather than part of the first stats release — but the cache read/write split is captured now, not deferred with it, because it cannot be reconstructed after the fact once a session file is written without it: Anthropic bills a cache write above, and a cache read far below, a plain input token, so a later cost pass needs this split already sitting in the transcript.

## Retention

Session files accumulate without bound unless something prunes them. Charlotte needs a retention policy: a maximum file count, a maximum age, or a manual `charlotte sessions clean` command. Pick one default before the first release, since an unbounded log directory is a predictable complaint from a long-time user.

## Open decision

The event schema above is a starting shape, not a frozen one. Before implementation, write the event types as a Zod schema in `packages/schemas`, alongside the structured model output schemas from `06-providers-and-ai-sdk.md`. Then make sure that every value the orchestrator already produces, per `07-orchestrator-pipeline.md`, has a field to land in. This keeps the schema from being redesigned mid-implementation.
