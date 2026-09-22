import { z } from "zod";
import { budgetInt, clampedNumber } from "./clamp";
import { providerSchema } from "./provider";

export const reasoningEffortSchema = z.enum(["low", "medium", "high"]);
export type ReasoningEffort = z.infer<typeof reasoningEffortSchema>;

/** Matches Christina's `ValidationMode` (`christina-core/src/types/commit.rs`):
 * `strict` rejects an over-length message, `soft` warns but allows it,
 * `disabled` skips the length check entirely. */
export const commitValidationModeSchema = z.enum(["strict", "soft", "disabled"]);
export type CommitValidationMode = z.infer<typeof commitValidationModeSchema>;

/**
 * Mirrors Christina's `Config` field set (`christina/src/config/settings.rs`),
 * defined as a Zod schema instead of a `schemars`-derived JSON Schema per
 * `03-config-and-profiles.md`. The generated JSON Schema is a build output
 * of this schema, not a hand-maintained file.
 */
export const configSchema = z.object({
  provider: providerSchema,
  model: z.string().min(1),
  apiKey: z.string().min(1),
  apiUrl: z.url().optional(),
  azureApiVersion: z.string().optional(),
  azureDeploymentId: z.string().optional(),

  temperature: clampedNumber(0, 2).default(1),
  reasoningEffort: reasoningEffortSchema.default("medium"),

  maxTokens: budgetInt(),
  lockfileTokenLimit: budgetInt(),
  ignorePatterns: z.array(z.string()).default([]),

  commitMessageMaxLength: budgetInt().default(72),
  commitValidationMode: commitValidationModeSchema.default("strict"),
  commitHistoryDepth: budgetInt(0).default(5),

  maxConcurrentRequests: clampedNumber(1, 32).default(4),
  /** Matches Christina's `MIN_PARTIAL_FAILURE_RATE`/`MAX_PARTIAL_FAILURE_RATE`. */
  partialFailureRate: clampedNumber(0.01, 0.5).default(0.2),
});

export type Config = z.infer<typeof configSchema>;

/** Every field a config layer may override; each layer supplies a subset. */
export const configOverlaySchema = configSchema.partial();
export type ConfigOverlay = z.infer<typeof configOverlaySchema>;

export const providerProfileSchema = configSchema
  .pick({
    provider: true,
    model: true,
    apiKey: true,
    apiUrl: true,
    azureApiVersion: true,
    azureDeploymentId: true,
    maxTokens: true,
    lockfileTokenLimit: true,
  })
  .partial()
  .required({ provider: true, model: true, apiKey: true });

export type ProviderProfile = z.infer<typeof providerProfileSchema>;

export const profilesSchema = z.object({
  active: z.string().nullable().default(null),
  profiles: z.record(z.string(), providerProfileSchema).default({}),
});

export type Profiles = z.infer<typeof profilesSchema>;
