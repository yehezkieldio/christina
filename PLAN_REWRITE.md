# Chirstina Rework and Rewrite

## Executive Summary

This document proposes a complete architectural consolidation of the Christina workspace from 4 fragmented crates into **exactly 2 crates** with Rust purist, opinionated boundaries. The current structure exhibits classic signs of over-engineering: mechanical module splits, leaky abstractions, duplicated concepts, over-fragmentation, heavy context switch and loss, incoherent codebase, and AI-generated ceremony.

IMPORTANT: This plan is designed for AI coding agents to do as a long-horizon task in matters of minutes to hours, with little to no human oversight, aside from keeping the machine alive. AI coding agents are given absolute freedom to choose how to implement the plan, as long as the end result matches the specifications herein.

This is a pre-alpha project, no backwards compatibility is required, you have full autonomy to redesign the entire codebase from scratch.

**Total Lines of Code**: ~18,500  
**Current Crates**: 4 (christina, christina-core, christina-git, christina-llm)  
**Proposed Crates**: 2 (christina, christina-core)  
**Target**: Hermetic, normalized, aesthetically coherent architecture

## The Two Crates

### 1.1 `christina` - The Application Crate

**Responsibility**: All user-facing code. CLI parsing, TUI rendering, event handling, and the main entry point.
**Key Rule**: This crate contains **no business logic**. It only orchestrates calls to `christina-core` and renders results.

### 1.2 `christina-core` - The Primary Library

**Responsibility**: All domain logic, git operations, LLM orchestration, and core types. Pure logic with no I/O except through explicit dependencies.
**Key Rule**: This crate knows **nothing about TUI, CLI, or terminal**. It operates on data structures and returns results.

## Proposed Directory Structure

Below is the proposed new directory structure with key files and modules. When working on the rewrite, files will be moved/merged according to this plan, make sure no files or functionality are lost. This is CRITICAL.

```
christina/
```

```
christina-core/
```

## Heuristics and Guidelines

### Anti-Patterns Being Eliminated

- Over-fragmentated codes
- Duplicated state representations
- Parallel abstractions
- Context dumping grounds
- Leaky layers and abstractions

## Migration Plan

1. Backup the four crates into `backup/` folder, where codes can be referenced on will to not lose context.
2. Create the two new crates with the proposed structure.
3. Migrate code from the old crates to the new crates, either migrate, rewrite, consolidate, or delete as necessary.
4. Skip on cargo check, clippy, and tests until the end, to avoid context switching.

## Important Notes

- Settings / Profile management is all over the place, simplify and consolidate it for easier addition of new settings in the future, or any settings/config related logic.
- Provider management (LLM providers) is also all over the place, consolidate it. For easier addition of new providers in the future.
- Git operations are scattered, consolidate them.
- Error handling must be consistent and idiomatic, consolidate them.
- Elm Architecture is Substantial and Well-Designed, keep it as is, but consolidate and adjust to fit the new architecture.
- Configuration TUI is Complex Multi-Screen Flow, Keep as dedicated module structure but flatten slightly.
- fix State Mutation Authority Was Distributed and StateMachine Didn't Own Screen States
- Invariant 1: Library Has Zero I/O
- Invariant 2: Type System Enforces Boundaries
- Invariant 3: Single Authority for State
- Invariant 4: Dependency Direction Is Strict
