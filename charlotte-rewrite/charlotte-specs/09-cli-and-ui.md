# CLI and UI

## Command surface

Charlotte's default command (no subcommand) runs the generation pipeline,
matching Christina's `christina/src/cli/mod.rs`. The top-level flags carry
over unchanged: `--verbose` (repeatable, for log level), `--trace`, `--yes`
(skip confirmation), `--context` (free text passed into the prompt), and
`--dry-run` (stop before Git creates the commit). The `config` and
`profile` subcommand groups carry over as specified in
`03-config-and-profiles.md`. Charlotte adds one new subcommand group,
`stats`, specified in `08-session-storage-and-stats.md`.

## Interactive confirmation

Christina's `christina/src/ui/mod.rs` shows the generated message, then
offers accept, edit inline, regenerate, or decline
(`select_action`, `edit_commit_message_inline`). Charlotte keeps this same
option set. The terminal prompt library changes: Christina uses `dialoguer`
and `console`, Charlotte uses a Bun-compatible prompt library such as
`@clack/prompts`. `--yes` skips this step entirely, the same way it does in
Christina today.

## Styling

Christina's `christina/src/ui/mod.rs` defines five named styles: header,
error, warning, accent, and muted. It also defines nine print helpers,
covering success, error, warning, info, trace, section, divider, file
list, and commit message output. A spinner (`create_spinner`) marks
in-progress stages. Charlotte keeps this same named style set and print
helper list as `packages/ui`. The visual language already works well, and
that is the only reason it carries over.

## The `ui-extractable` question

Christina's `ui-extractable` crate is a separate binary holding table and
styled-line rendering primitives, independent of the rest of the
workspace. Charlotte folds this logic into `packages/ui` instead of
keeping a second compiled artifact. `bun build --compile` already produces
one executable for the whole CLI, so a second binary serves no purpose
Charlotte needs.

## Progress reporting

Christina's `generate.rs` sends progress events during generation
(`send_generation_progress`) so the spinner text can update per stage.
Charlotte keeps this event stream. Per `08-session-storage-and-stats.md`,
it also writes each progress event to the session transcript. The same
event then powers both the live spinner and the persistent record.

## Open decision

Christina's inline editor binds Ctrl-word navigation
(`bind_ctrl_word_navigation`) as a custom key handler. Charlotte needs a
look at whether the chosen prompt library exposes this binding natively.
If it does not, Charlotte has two options. It can drop the binding and
match the library's own default behavior. It can also reimplement the
binding to match Christina's current editing behavior exactly.
