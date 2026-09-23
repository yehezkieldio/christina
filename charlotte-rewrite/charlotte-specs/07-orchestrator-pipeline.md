# Orchestrator pipeline

## Shape

Christina's `AIOrchestrator` (`christina/src/orchestrator/mod.rs`) runs a map-reduce pipeline over a chunked diff. Charlotte keeps this shape and rewrites it in TypeScript. The shape itself is proven. Most of Christina's complexity sits in the JSON-repair layer, and `06-providers-and-ai-sdk.md` replaces that layer.

## Direct path

When a diff is small enough to skip chunking, Christina calls `direct_generation`. This path sends one prompt, built by `PromptBuilder::build_direct_prompt`, and returns a commit message from that single call. Charlotte keeps this short path as a plain function. It skips the map-reduce machinery entirely for small diffs.

## Map phase

Christina's `map_phase` summarizes each chunk concurrently. The concurrency bound comes from `MAX_CONCURRENT_REQUESTS` and the throttle described in `06-providers-and-ai-sdk.md`. Each chunk summary becomes a `ChunkSummary` with a `summary` string and a `files` list. Charlotte's map phase keeps the same bound and the same output shape. It uses a small concurrency-limited `Promise.all` in place of a Rust semaphore.

## Intent extraction

Christina's `extract_intent` and `extract_intent_hierarchical` build a theme structure from the chunk summaries. A `ThemeItem` carries a title, description, file count, and scope. Each `ThemeItem` can hold a set of `SubTheme` entries once the summary count crosses `MAX_SUMMARIES_PER_INTENT_BATCH`. `detect_contradictions` flags themes that describe conflicting changes to the same files. Charlotte keeps this two-level theme structure. Each level becomes one Zod schema, per `06-providers-and-ai-sdk.md`. The contradiction check stays as plain TypeScript logic over the returned theme list, because it needs no model call.

## Reduce phase

Christina's `reduce_phase` takes the themes and produces the final `CommitResponse`. When intent extraction is skipped, it takes a fallback summary list instead. `clean_response` strips leading and trailing noise before the message is matched against the Conventional Commit rules. Charlotte keeps `reduce_phase` and `clean_response` as direct ports. Both are pure string and structural logic with no dependency on the JSON-repair layer being replaced.

## Fallback paths

When a chunk summary call fails, Christina falls back to a summary built directly from file names (`fallback_summary_from_files`). When the model returns no parseable result, it falls back to a synthesized sub-theme or theme list (`fallback_sub_themes_from_summaries`, `fallback_themes_from_summaries`). Charlotte keeps every fallback path Christina defines. These paths keep a generation run usable under partial provider failure, and that requirement does not change with the rewrite.

## Partial failure tolerance

Christina tracks `failed_chunks` and `failed_files` against `max_partial_failure_rate`. The failure rate is bounded between `MIN_PARTIAL_FAILURE_RATE` and `MAX_PARTIAL_FAILURE_RATE`. Christina aborts the whole run only once the failure rate crosses that threshold. Charlotte's `GenerationResult` keeps this same field set and the same threshold check, unchanged from Christina.

## Cancellation

Christina threads a cancellation token through the pipeline (`generate_commit_message_with_trace_and_cancellation`, `ensure_not_cancelled`). A shutdown signal can then stop generation between stages instead of only at process exit. Charlotte uses an `AbortSignal` for the same purpose, the direct TypeScript equivalent. It passes through every orchestrator function the same way Christina passes its cancellation token.

## Open decision

Christina's `warning_summary` on `GenerationResult` formats truncation, salvage, and fallback warnings as user-facing text, written inline in the orchestrator module. Charlotte needs a decision on where this formatting lives. The orchestrator package can keep it, or the CLI's UI package (`09-cli-and-ui.md`) can own it instead. If Charlotte wants the same warning data usable in the session transcript, the orchestrator must return structured warning data, not pre-formatted display strings.
