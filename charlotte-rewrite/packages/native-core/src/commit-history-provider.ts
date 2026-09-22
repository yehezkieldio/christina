/**
 * Matches Christina's `CommitHistoryProvider` trait
 * (`christina/src/generate.rs`). This file deliberately has no dependency
 * on `./index` (the `bun:ffi` loader): `@charlotte/orchestrator`'s tests
 * import `FakeCommitHistoryProvider` from here so they can run without
 * loading the native module at all, per `04-git-integration.md`. The
 * native-backed implementation lives in `./native-commit-history-provider`
 * instead, so importing it is an opt-in, not a side effect of this file.
 */
export interface CommitSummary {
  readonly sha: string;
  readonly subject: string;
}

export interface CommitHistoryProvider {
  getCommitHistory(repoPath: string, depth: number): CommitSummary[];
}

export class FakeCommitHistoryProvider implements CommitHistoryProvider {
  constructor(private readonly commits: readonly CommitSummary[] = []) {}

  getCommitHistory(_repoPath: string, depth: number): CommitSummary[] {
    return this.commits.slice(0, depth);
  }
}

export class FailingCommitHistoryProvider implements CommitHistoryProvider {
  getCommitHistory(): CommitSummary[] {
    throw new Error("Failed to retrieve commit history");
  }
}
