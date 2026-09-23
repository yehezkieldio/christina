import { z } from "zod";

/**
 * The provider list from `06-providers-and-ai-sdk.md`: everything the AI SDK
 * ships a provider for, plus a catch-all for self-hosted OpenAI-compatible
 * endpoints (Ollama and similar).
 */
export const PROVIDERS = [
  "openai",
  "azure-openai",
  "anthropic",
  "google",
  "openai-compatible",
] as const;

export type Provider = (typeof PROVIDERS)[number];

export const providerSchema = z.enum(PROVIDERS);

/** Exhaustiveness guard: a new `Provider` variant fails this at compile time
 * until every switch over `Provider` adds a matching case. */
export const assertUnreachable = (value: never): never => {
  throw new Error(`unreachable provider variant: ${JSON.stringify(value)}`);
};

/** Fields a provider requires beyond the shared `model` and `apiKey`. Only
 * `azure-openai` needs anything extra, and the type system reflects that:
 * every other provider's extra-fields type is `undefined`. */
export type ProviderExtraFields<P extends Provider> = P extends "azure-openai"
  ? { readonly apiVersion: string; readonly deploymentId: string }
  : undefined;

export const requiresAzureFields = (
  provider: Provider
): provider is "azure-openai" => provider === "azure-openai";

export const describeProvider = (provider: Provider): string => {
  switch (provider) {
    case "openai": {
      return "OpenAI";
    }
    case "azure-openai": {
      return "Azure OpenAI";
    }
    case "anthropic": {
      return "Anthropic";
    }
    case "google": {
      return "Google";
    }
    case "openai-compatible": {
      return "OpenAI-compatible endpoint";
    }
    default: {
      return assertUnreachable(provider);
    }
  }
};
