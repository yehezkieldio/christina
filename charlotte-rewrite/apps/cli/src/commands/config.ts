import {
  configFilePath,
  configOverlaySchema,
  loadConfig,
} from "@charlotte/config";
import type { Command } from "commander";

import { readToml, writeToml } from "../toml-io";

type FieldParser = (raw: string) => unknown;

/** One parser per settable `Config` field, keyed the same way the field
 * appears in `@charlotte/config`'s schema (camelCase) — the CLI's key
 * vocabulary matches the TypeScript config shape directly rather than
 * Christina's Rust field names, since the two do not line up one-to-one
 * (Charlotte merged `max_input_tokens`/`max_output_tokens` into one
 * `maxTokens` field per `03-config-and-profiles.md`'s clamp-policy work). */
const FIELD_PARSERS: Record<string, FieldParser> = {
  apiKey: (raw) => raw,
  apiUrl: (raw) => raw,
  azureApiVersion: (raw) => raw,
  azureDeploymentId: (raw) => raw,
  commitHistoryDepth: (raw) => Math.trunc(Number(raw)),
  commitMessageMaxLength: (raw) => Math.trunc(Number(raw)),
  commitValidationMode: (raw) => raw,
  ignorePatterns: (raw) =>
    raw
      .split(",")
      .map((pattern) => pattern.trim())
      .filter((pattern) => pattern.length > 0),
  lockfileTokenLimit: (raw) => Math.trunc(Number(raw)),
  maxConcurrentRequests: (raw) => Math.trunc(Number(raw)),
  maxTokens: (raw) => Math.trunc(Number(raw)),
  model: (raw) => raw,
  partialFailureRate: (raw) => Number(raw),
  provider: (raw) => raw,
  reasoningEffort: (raw) => raw,
  temperature: (raw) => Number(raw),
};

const isSecretLikeKey = (key: string): boolean => key.toLowerCase().includes("key");

const formatValue = (value: unknown): string => {
  if (value === undefined) {
    return "<not set>";
  }
  if (Array.isArray(value)) {
    return value.length === 0 ? "<none>" : value.join(", ");
  }
  return String(value);
};

const handleGet = async (key: string): Promise<void> => {
  const config = await loadConfig();
  if (!(key in FIELD_PARSERS)) {
    throw new Error(`Unknown configuration key '${key}'`);
  }
  const value = (config as unknown as Record<string, unknown>)[key];
  if (isSecretLikeKey(key)) {
    console.log(`${key}: <hidden>`);
    return;
  }
  console.log(`${key}: ${formatValue(value)}`);
};

const handleSet = async (key: string, value: string): Promise<void> => {
  const parser = FIELD_PARSERS[key];
  if (!parser) {
    throw new Error(`Unknown configuration key '${key}'`);
  }
  const parsed = parser(value);
  if (typeof parsed === "number" && Number.isNaN(parsed)) {
    throw new TypeError(`Invalid value for '${key}': ${value}`);
  }

  const path = configFilePath();
  const existing = await readToml(path);
  const merged = { ...existing, [key]: parsed };
  const result = configOverlaySchema.safeParse(merged);
  if (!result.success) {
    throw new Error(
      `Invalid value for '${key}': ${result.error.issues.map((issue) => issue.message).join("; ")}`
    );
  }

  await writeToml(path, result.data as Record<string, unknown>);
  console.log(`Set ${key} = ${value}`);
};

const handleList = async (): Promise<void> => {
  const config = await loadConfig();
  console.log("Configuration values:");
  for (const [key, value] of Object.entries(config)) {
    console.log(
      `  ${key}: ${isSecretLikeKey(key) ? "<hidden>" : formatValue(value)}`
    );
  }
};

const handlePath = (): void => {
  console.log(configFilePath());
};

export const registerConfigCommand = (program: Command): void => {
  const config = program
    .command("config")
    .description("Configuration management");

  config
    .command("get <key>")
    .description("Get a configuration value")
    .action(async (key: string) => {
      await handleGet(key);
    });

  config
    .command("set <key> <value>")
    .description("Set a configuration value")
    .action(async (key: string, value: string) => {
      await handleSet(key, value);
    });

  config
    .command("list")
    .description("List all configuration values")
    .action(async () => {
      await handleList();
    });

  config
    .command("path")
    .description("Show configuration file path")
    .action(() => {
      handlePath();
    });
};
