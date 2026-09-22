# Overview

## What Charlotte is

Charlotte is a new project, built on the ideas in Christina. Christina
reads the staged Git diff in a repository and sends it to an AI model. It
proposes a Conventional Commit message for the operator to accept, edit,
or decline. Charlotte keeps this same core behavior and this same scope.
Charlotte does not become a general coding agent.

Charlotte is not a migration target for Christina. It carries forward Christina's proven design decisions, such as the
map-reduce generation pipeline and the chunking algorithm. It owes no
compatibility promise to an existing Christina installation. This is a
side project, and it is free to diverge from Christina wherever divergence
serves the project better.

## Why build Charlotte

Christina works today. Charlotte exists to explore three ideas Christina's
current design does not reach.

1. Provider support. Christina only speaks to Azure OpenAI. Charlotte
   supports multiple providers (OpenAI, Anthropic, Google, and
   OpenAI-compatible endpoints) through the AI SDK.
2. Structured output. Christina hand-writes JSON extraction and repair
   logic for models that do not return clean structured output. This logic
   is roughly 250 lines of the orchestrator and is hard to extend.
   Charlotte replaces it with schema-validated structured output.
3. Observability. Christina has a `--trace` flag but no persistent record
   of past runs, token usage, or cost. Charlotte adds a session transcript
   and a stats command, in the style of Claude Code, Codex, and opencode.

## Guiding principles

Performance is a design-time property, not a cleanup phase. Charlotte
places Rust at the algorithmic hot paths that Christina's own codebase
graph identifies as high fan-in: diff chunking, tokenization, and binary
detection. Everything else runs in TypeScript, where iteration speed and
access to the AI SDK ecosystem matter more than raw throughput.

The FFI (foreign function interface, a mechanism that lets one language
call functions compiled in another language) boundary follows evidence,
not symmetry. Charlotte does not split work between Rust and TypeScript
because a 50/50 split feels balanced. Charlotte puts a function in Rust in
two cases only: the function sits on a measured hot path, or the function
needs a Rust-only library such as `git2` for repository reading.

Structured output replaces manual parsing. Where Christina repairs
malformed JSON by hand, Charlotte defines a Zod schema and lets the AI SDK
enforce it. If a provider's structured output guarantee turns out weaker
than Christina's own repair heuristics, this decision needs review before
the heuristics are dropped. See `06-providers-and-ai-sdk.md`.

Session data is a first-class artifact. Every generation run writes an
append-only transcript. The transcript is the source of truth for the trace
output, the stats command, and future debugging.

Packages stay small and single-purpose. `01-architecture.md` lays out a
Bun workspace with Turborepo, and no package becomes a dumping ground for
unrelated logic. A library that grows a second responsibility splits into
two packages.

## Non-goals

Charlotte targets Bun as the runtime and build tool, compiled with
`bun build --compile` into a single executable. It does not aim for a
Node.js-only fallback.

Charlotte does not expand scope into a general chat interface, a coding
agent, or a plugin system. The product is a commit message generator with
a session log attached.

Charlotte does not commit to config-file or CLI-flag compatibility with
Christina. Where Christina's shape is good, Charlotte reuses it because it
is good, not because compatibility is a goal in itself.

## Source of truth

Every spec in this folder that names a Christina file path or function
describes the state of the `christina` repository as indexed in this
session. Christina is a reference for proven ideas, not a spec Charlotte
must track going forward.
