# Native core and FFI

## What lives in the native core

The Rust crate `native/charlotte-core` carries over four pieces of logic from Christina almost unchanged. Christina's own codebase graph marks these as the highest-fan-in functions in the whole project.

- Staged diff reading and commit history, using `git2` (from `christina/src/git/`).
- Binary content detection by null-byte sampling and file extension (from `christina/src/git/diff_processor.rs`).
- Diff chunking: recursive splitting by hunk, then by line, then by raw token span, with a separate token cap for lockfiles (from `christina-core/src/processing/chunking.rs`).
- Token counting with `tiktoken-rs` (from `christina-core/src/processing/tokenizer.rs` and `christina-core/src/types/tokens.rs`).

Christina's existing test suite for these modules, including the property tests, moves into `native/charlotte-core` largely as-is. The chunking module alone carries over 20 dedicated tests in Christina today. That coverage is the reason this code is worth keeping in Rust instead of a rewrite in TypeScript.

## What does not live in the native core

Everything performance-neutral stays in TypeScript, even where Christina happens to implement it in Rust today. This includes config loading, the orchestrator's retry and throttle logic, prompt building, structured output handling, and the CLI. Moving these to Rust adds an FFI hop for no measured benefit.

## FFI mechanism: `bun:ffi` first

Charlotte uses `bun:ffi`, Bun's built-in module for calling native code, as the default FFI mechanism. `bun:ffi` calls a compiled `cdylib` (a dynamically loaded native library) directly through `dlopen`. There is no separate code-generation step and no per-platform native-module build pipeline beyond compiling the Rust crate itself.

This is a deliberate acceptance of risk. Bun's own documentation marks `bun:ffi` as experimental and names known bugs and limits. Charlotte accepts this risk. It is a side project, not a product with an install base to protect. `bun:ffi`'s simplicity, one `dlopen` call, no generated bindings, no separate build toolchain, suits fast iteration better than a more elaborate setup.

`packages/native-core` wraps every `dlopen` call and every `FFIType` declaration behind a plain TypeScript function per native operation. Nothing outside this one package touches `bun:ffi` directly. This keeps the blast radius of any `bun:ffi` bug contained to one package.

## Fallback: Node-API through `napi-rs`

`bun:ffi` bugs can block real progress: a crash, a memory corruption, or a platform-specific failure that is not a Charlotte code issue. When that happens, `packages/native-core` switches its native binding to Node-API through the `napi-rs` crate instead. Bun supports Node-API natively, and `napi-rs` is the path Bun's own documentation names as production-stable.

Because `packages/native-core` is the only package that imports the native module, this switch does not touch any other package. The public TypeScript function signatures in `packages/native-core` stay the same either way. Only the loading mechanism inside that one package changes.

## Boundary signatures

Each native function takes plain arguments and returns plain data. No struct crosses the boundary as a live object. A struct that needs to travel serializes to a byte buffer or a JSON string at the edge.

```
read_staged_diff(repo_path: string) -> { diff: string, files: string[] }
read_commit_history(repo_path: string, depth: number) -> { sha: string, subject: string }[]
is_binary_content(bytes: Uint8Array, path: string) -> boolean
count_tokens(text: string) -> number
chunk_diff(diff: string, token_limit: number, lockfile_token_limit: number) -> Chunk[]
```

`Chunk` is `{ content: string, filePaths: string[] }`, matching the shape of Christina's `ChunkBuffer`.

## Open decision

`git2` performs disk I/O and can block the thread it runs on. A long-running native call blocks Bun's JavaScript thread for its whole duration, since `bun:ffi` calls are synchronous by default. Large repositories with deep history make `read_commit_history` a candidate for a worker-thread wrapper. Decide this once a real repository benchmark shows whether the synchronous call causes a visible stall in the CLI.
