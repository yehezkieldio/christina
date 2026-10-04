import { spinner as clackSpinner } from "@clack/prompts";

/**
 * Wraps `@clack/prompts`'s spinner. Clack already renders as a plain, static
 * line when stdout is not a TTY instead of emitting control codes, so this
 * needs no separate non-TTY branch.
 */
export interface Spinner {
  start: (message: string) => void;
  update: (message: string) => void;
  stop: (message?: string) => void;
}

export const createSpinner = (): Spinner => {
  const inner = clackSpinner();
  return {
    start: (message) => inner.start(message),
    stop: (message) => inner.stop(message),
    update: (message) => inner.message(message),
  };
};
