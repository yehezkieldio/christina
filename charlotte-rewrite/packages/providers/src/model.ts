import { createAnthropic } from "@ai-sdk/anthropic";
import { createAzure } from "@ai-sdk/azure";
import { createGoogle } from "@ai-sdk/google";
import { createOpenAI } from "@ai-sdk/openai";
import { createOpenAICompatible } from "@ai-sdk/openai-compatible";
import { assertUnreachable, type ResolvedConfig } from "@charlotte/config";
import type { LanguageModel } from "ai";

/**
 * Builds an AI SDK `LanguageModel` for the resolved config's provider.
 * `config.apiKey.reveal()` is called exactly once, right here — this is the
 * one place in `@charlotte/providers` allowed to hold the plaintext key,
 * and only for the duration of this call.
 */
export function resolveModel(config: ResolvedConfig): LanguageModel {
  const apiKey = config.apiKey.reveal();

  switch (config.provider) {
    case "openai":
      return createOpenAI({ apiKey, baseURL: config.apiUrl })(config.model);
    case "azure-openai":
      return createAzure({
        apiKey,
        baseURL: config.apiUrl,
        apiVersion: config.azureApiVersion,
        useDeploymentBasedUrls: config.azureDeploymentId !== undefined,
      })(config.azureDeploymentId ?? config.model);
    case "anthropic":
      return createAnthropic({ apiKey, baseURL: config.apiUrl })(config.model);
    case "google":
      return createGoogle({ apiKey, baseURL: config.apiUrl })(config.model);
    case "openai-compatible": {
      if (!config.apiUrl) {
        throw new Error('provider "openai-compatible" requires apiUrl to be set');
      }
      return createOpenAICompatible({ apiKey, baseURL: config.apiUrl, name: "charlotte" })(config.model);
    }
    default:
      return assertUnreachable(config.provider);
  }
}
