# Charlotte specs

Charlotte is a new project, built on the ideas in Christina. Christina is a terminal commit assistant that turns a staged Git diff into a Conventional Commit message, written in Rust. Charlotte targets Bun and TypeScript for the application layer. A native Rust core handles the algorithmic hot paths. A foreign function interface (FFI, a mechanism that lets one language call functions compiled in another language) connects the two, through Bun's built-in `bun:ffi` module.

This folder holds the specs and plans for Charlotte. Read them in order.

1. `00-overview.md`: goals, non-goals, and guiding principles.
2. `01-architecture.md`: the Bun workspace and Turborepo package layout, and the TypeScript/Rust split.
3. `02-native-core-and-ffi.md`: the Rust native core and the FFI boundary.
4. `03-config-and-profiles.md`: configuration file, profiles, and secrets.
5. `04-git-integration.md`: reading the staged diff and commit history.
6. `05-diff-processing-and-chunking.md`: binary detection, chunking, and tokenization.
7. `06-providers-and-ai-sdk.md`: multi-provider support through the AI SDK.
8. `07-orchestrator-pipeline.md`: the map-reduce generation pipeline.
9. `08-session-storage-and-stats.md`: session transcripts and token-usage stats.
10. `09-cli-and-ui.md`: commands and the interactive terminal UI.
11. `10-testing.md`: the test strategy and the pre-release scenario checklist.
12. `11-build-phases.md`: the build order for a first working version.

Each spec names open decisions that need a look before implementation starts. Search for "Open decision" in a file to find them.
