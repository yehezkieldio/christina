import { createAnthropic } from "@ai-sdk/anthropic";
import { createAzure } from "@ai-sdk/azure";
import { createGoogle } from "@ai-sdk/google";
import { createGroq } from "@ai-sdk/groq";
import { createOpenAI } from "@ai-sdk/openai";
import { createOpenAICompatible } from "@ai-sdk/openai-compatible";
import { assertUnreachable } from "@charlotte/config";
import type { ResolvedConfig } from "@charlotte/config";
import { createOpenRouter } from "@openrouter/ai-sdk-provider";
import type { LanguageModel } from "ai";

import { optional } from "./optional";

/**
 * Builds an AI SDK `LanguageModel` for the resolved config's provider.
 * `config.apiKey.reveal()` is called exactly once, right here — this is the
 * one place in `@charlotte/providers` allowed to hold the plaintext key,
 * and only for the duration of this call.
 */
export const resolveModel = (config: ResolvedConfig): LanguageModel => {
  const apiKey = config.apiKey.reveal();

  switch (config.provider) {
    case "openai": {
      return createOpenAI({ apiKey, ...optional("baseURL", config.apiUrl) })(
        config.model
      );
    }
    case "azure-openai": {
      return createAzure({
        apiKey,
        ...optional("baseURL", config.apiUrl),
        ...optional("apiVersion", config.azureApiVersion),
        useDeploymentBasedUrls: config.azureDeploymentId !== undefined,
      })(config.azureDeploymentId ?? config.model);
    }
    case "anthropic": {
      return createAnthropic({ apiKey, ...optional("baseURL", config.apiUrl) })(
        config.model
      );
    }
    case "google": {
      return createGoogle({ apiKey, ...optional("baseURL", config.apiUrl) })(
        config.model
      );
    }
    case "groq": {
      return createGroq({ apiKey, ...optional("baseURL", config.apiUrl) })(
        config.model
      );
    }
    case "openrouter": {
      return createOpenRouter({
        apiKey,
        ...optional("baseURL", config.apiUrl),
      })(config.model);
    }
    case "openai-compatible": {
      if (!config.apiUrl) {
        throw new Error(
          'provider "openai-compatible" requires apiUrl to be set'
        );
      }
      return createOpenAICompatible({
        apiKey,
        baseURL: config.apiUrl,
        name: "charlotte",
      })(config.model);
    }
    default: {
      return assertUnreachable(config.provider);
    }
  }
};
