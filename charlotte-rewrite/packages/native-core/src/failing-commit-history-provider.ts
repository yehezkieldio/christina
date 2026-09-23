import type {
  CommitHistoryProvider,
  CommitSummary,
} from "./commit-history-provider";

export class FailingCommitHistoryProvider implements CommitHistoryProvider {
  // The interface requires an instance method regardless of whether this
  // implementation happens to use `this`.
  // oxlint-disable-next-line class-methods-use-this
  getCommitHistory(): CommitSummary[] {
    throw new Error("Failed to retrieve commit history");
  }
}
