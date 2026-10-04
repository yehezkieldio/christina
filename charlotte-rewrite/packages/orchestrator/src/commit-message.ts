import type { CommitValidationMode } from "@charlotte/config";

/** `type(scope)?!?: description` */
const CONVENTIONAL_COMMIT_PATTERN =
  /^[A-Za-z]+(?<scope>\([A-Za-z0-9._/@-]+\))?!?:\s*\S.*$/u;

const DEFAULT_MAX_LENGTH = 72;

export class InvalidCommitMessageError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "InvalidCommitMessageError";
  }
}

export interface ValidatedCommitMessage {
  readonly message: string;
  readonly warnings: readonly string[];
}

/** Validates that the message is non-empty, single line, matches the
 * Conventional Commits pattern, and respects `maxLength` according to `mode`
 * (`strict` rejects, `soft` warns, `disabled` skips the length check
 * entirely). */
export const validateCommitMessage = (
  value: string,
  mode: CommitValidationMode,
  maxLength: number = DEFAULT_MAX_LENGTH
): ValidatedCommitMessage => {
  const trimmed = value.trim();
  const warnings: string[] = [];

  if (trimmed.length === 0) {
    throw new InvalidCommitMessageError("Commit message cannot be empty");
  }

  if (trimmed.length > maxLength) {
    if (mode === "strict") {
      throw new InvalidCommitMessageError(
        `Commit message exceeds ${maxLength} characters`
      );
    }
    if (mode === "soft") {
      warnings.push(
        `Commit message exceeds recommended ${maxLength} character limit (${trimmed.length} chars)`
      );
    }
  }

  if (trimmed.includes("\n")) {
    throw new InvalidCommitMessageError("Commit message must be single line");
  }

  if (!CONVENTIONAL_COMMIT_PATTERN.test(trimmed)) {
    throw new InvalidCommitMessageError(
      "Commit message must follow conventional commits format: type(scope): description"
    );
  }

  return { message: trimmed, warnings };
};

/** When the raw message doesn't validate as-is, scans for a `type: desc`
 * shaped substring around each colon and returns the earliest one that
 * does validate. */
export const tryExtractValidCommit = (
  message: string,
  mode: CommitValidationMode,
  maxLength?: number
): string | undefined => {
  let earliest: { pos: number; candidate: string } | undefined;

  for (
    let pos = message.indexOf(":");
    pos !== -1;
    pos = message.indexOf(":", pos + 1)
  ) {
    // Look back at most 50 characters from each colon: enough to cover a
    // `type(scope)!` prefix, short enough that preamble prose before it is
    // not swallowed into the candidate.
    const start = Math.max(0, pos - 50);
    let candidate = message.slice(start).trimStart();
    const end = candidate.indexOf("\n");
    candidate = (end === -1 ? candidate : candidate.slice(0, end)).trim();

    try {
      validateCommitMessage(candidate, mode, maxLength);
    } catch {
      continue;
    }

    if (!earliest || pos < earliest.pos) {
      earliest = { candidate, pos };
    }
  }

  return earliest?.candidate;
};

/** Combines direct validation with the salvage fallback: try the raw
 * message first, then try to salvage a valid substring, then give up. */
export const validateOrSalvage = (
  message: string,
  mode: CommitValidationMode,
  maxLength?: number
): ValidatedCommitMessage & { readonly salvaged: boolean } => {
  const trimmed = message.trim();

  try {
    const validated = validateCommitMessage(trimmed, mode, maxLength);
    return { ...validated, salvaged: false };
  } catch {
    // fall through to salvage
  }

  const salvaged = tryExtractValidCommit(trimmed, mode, maxLength);
  if (salvaged !== undefined) {
    const validated = validateCommitMessage(salvaged, mode, maxLength);
    return { ...validated, salvaged: true };
  }

  throw new InvalidCommitMessageError(
    `Generated message does not follow Conventional Commits format: ${trimmed}`
  );
};
