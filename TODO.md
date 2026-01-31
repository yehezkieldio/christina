5. Error Handling (Architecture)
Uses thiserror for unified error handling in christina-core.
Ensure all new modules strictly adhere to this pattern. Specifically, check if anyhow is being used in library code (which should be avoided in favor of thiserror for better downstream handling).
         
6. TUI Output Corruption (`eprintln!` usage)
The AIOrchestrator and Config modules use eprintln! for logging warnings (e.g., "direct generation completed", "Failed to persist default profile"). In a TUI application, writing directly to stderr/stdout can corrupt the terminal rendering, causing visual artifacts or "ghosting".
Replace eprintln! with a proper implementation to channel these messages into the TUI's event loop (AppMsg::Log or AppMsg::ShowToast).

7. Small Batch Fragility (LLM Orchestrator)
The AIOrchestrator enforces a strict 10% partial failure threshold (MAX_PARTIAL_FAILURE_RATE = 0.10).
If a diff is split into fewer than 10 chunks (e.g., 5 chunks), a single failure results in a 20% failure rate, triggering an immediate abort. This makes the tool brittle for small-to-medium changes on flaky connections.
Adjust the logic to allow at least 1 chunk failure regardless of the percentage (e.g., if failed_count > 1 && failure_rate > 0.10), or prompt the user for confirmation instead of aborting.

8. JSON Parsing Robustness
AIOrchestrator::extract_balanced_json implements a custom brace-counting parser to extract JSON from LLM output.
Custom parsers are notorious for edge cases (e.g., braces inside comments, escaped quotes in weird contexts).
While currently tested, it's safer to rely on established patterns. Consider simplified extraction (finding the first { and last }) combined with a "repair" library or simply relying on the LLM to output valid JSON more strictly via "json mode" parameters (if supported by the provider) or improved system prompts.

9. Deletion Truncation vs. Moves
DiffProcessor aggressively truncates diffs that are "only deletions" to save tokens.
If a file is moved (renamed) but Git detects it as a separate "delete" and "add" (due to low similarity or config), the "delete" part might be heavily truncated. This deprives the LLM of the context needed to recognize that the new file is essentially the old file, potentially leading to "Create X" messages instead of "Refactor/Move X".
