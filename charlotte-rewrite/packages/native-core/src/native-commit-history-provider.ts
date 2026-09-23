import type {
  CommitHistoryProvider,
  CommitSummary,
} from "./commit-history-provider";
import { readCommitHistory } from "./index";

export class NativeCommitHistoryProvider implements CommitHistoryProvider {
  getCommitHistory(repoPath: string, depth: number): CommitSummary[] {
    return readCommitHistory(repoPath, depth);
  }
}
