import type { CommitValidationMode } from "@charlotte/config";
import { generateStructured, optional } from "@charlotte/providers";
import type { GenerateStructuredOptions } from "@charlotte/providers";
import { commitResponseSchema } from "@charlotte/schemas";
import type { ThemeItem } from "@charlotte/schemas";

import { validateOrSalvage } from "./commit-message";
import type { ValidatedCommitMessage } from "./commit-message";
import { buildReducePrompt, buildSystemPrompt } from "./prompt";
import type { PromptContext } from "./prompt";

const CODE_FENCE_PREAMBLES = [
  "here is the commit message:",
  "here's the commit message:",
  "commit message:",
  "the commit message is:",
] as const;

/** Strips markdown code-fencing, a known preamble phrase, and everything
 * after the first line. Ported from Christina's `clean_response` — pure
 * string logic, no model call, used as a fallback when the model's
 * structured `message` field itself still carries noise. */
export const cleanResponse = (response: string): string => {
  let message = response.trim();

  if (message.startsWith("```")) {
    const closeIndex = message.slice(3).indexOf("```");
    if (closeIndex !== -1) {
      message = message.slice(3, 3 + closeIndex).trim();
    }
  }

  const lower = message.toLowerCase();
  for (const preamble of CODE_FENCE_PREAMBLES) {
    const pos = lower.indexOf(preamble);
    if (pos !== -1) {
      message = message.slice(pos + preamble.length).trim();
      break;
    }
  }

  const newlineIndex = message.indexOf("\n");
  if (newlineIndex !== -1) {
    message = message.slice(0, newlineIndex).trim();
  }

  return message;
};

export interface ReducePhaseOptions {
  readonly model: GenerateStructuredOptions<unknown>["model"];
  readonly context?: PromptContext;
  readonly validationMode: CommitValidationMode;
  readonly maxLength?: number;
  readonly signal?: AbortSignal;
}

export interface ReducePhaseResult extends ValidatedCommitMessage {
  readonly salvaged: boolean;
  readonly promptTokens: number;
  readonly completionTokens: number;
}

/** Synthesizes the final commit message from themes, then validates it
 * against the configured Conventional Commit mode. Ported from Christina's
 * `reduce_phase`. */
export const reducePhase = async (
  themes: readonly ThemeItem[],
  options: ReducePhaseOptions
): Promise<ReducePhaseResult> => {
  options.signal?.throwIfAborted();

  const prompt = `${buildSystemPrompt()}\n\n${buildReducePrompt(themes, options.context)}`;
  const result = await generateStructured({
    model: options.model,
    prompt,
    schema: commitResponseSchema,
    ...optional("signal", options.signal),
  });

  const cleaned = cleanResponse(result.object.message);
  const validated = validateOrSalvage(
    cleaned,
    options.validationMode,
    options.maxLength
  );

  return {
    ...validated,
    completionTokens: result.completionTokens,
    promptTokens: result.promptTokens,
  };
};
