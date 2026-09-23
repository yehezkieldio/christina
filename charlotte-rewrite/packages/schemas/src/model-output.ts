import { z } from "zod";

/**
 * Structured `generateObject` schemas for the map-reduce pipeline in
 * `07-orchestrator-pipeline.md`. Field shapes mirror Christina's
 * `christina/src/orchestrator/mod.rs` structs (`ChunkSummary`,
 * `SummaryResponse`, `SubTheme`, `ThemeItem`, `ThemeResponse`,
 * `CommitResponse`) exactly, so a Charlotte fixture built from a Christina
 * trace round-trips without reshaping.
 */

export const summaryResponseSchema = z.object({
  summary: z.string().min(1),
});
export type SummaryResponse = z.infer<typeof summaryResponseSchema>;

export const chunkSummarySchema = z.object({
  files: z.array(z.string()),
  summary: z.string().min(1),
});
export type ChunkSummary = z.infer<typeof chunkSummarySchema>;

export const subThemeSchema = z.object({
  description: z.string().min(1),
  fileCount: z.number().int().min(0),
  scope: z.string().nullable(),
  title: z.string().min(1),
});
export type SubTheme = z.infer<typeof subThemeSchema>;

export const themeItemSchema = z.object({
  description: z.string().min(1),
  fileCount: z.number().int().min(0),
  scope: z.string().nullable(),
  /** Present once the batch that produced this theme crossed
   * `MAX_SUMMARIES_PER_INTENT_BATCH`, per `07-orchestrator-pipeline.md`. */
  subThemes: z.array(subThemeSchema).optional(),
  title: z.string().min(1),
});
export type ThemeItem = z.infer<typeof themeItemSchema>;

export const themeResponseSchema = z.object({
  themes: z.array(themeItemSchema).default([]),
});
export type ThemeResponse = z.infer<typeof themeResponseSchema>;

export const commitResponseSchema = z.object({
  message: z.string().min(1),
});
export type CommitResponse = z.infer<typeof commitResponseSchema>;
