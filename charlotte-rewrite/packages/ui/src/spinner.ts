import { spinner as clackSpinner } from "@clack/prompts";

/**
 * Wraps `@clack/prompts`'s spinner, matching `create_spinner` from
 * `christina/src/ui/mod.rs`. Clack already renders as a plain, static line
 * when stdout is not a TTY instead of emitting control codes, so this
 * needs no separate non-TTY branch the way `indicatif::ProgressBar::hidden`
 * did.
 */
export interface Spinner {
  start(message: string): void;
  update(message: string): void;
  stop(message?: string): void;
}

export function createSpinner(): Spinner {
  const inner = clackSpinner();
  return {
    start: (message) => inner.start(message),
    update: (message) => inner.message(message),
    stop: (message) => inner.stop(message),
  };
}
