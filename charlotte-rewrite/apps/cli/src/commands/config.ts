import {
  configFilePath,
  configOverlaySchema,
  loadConfig,
} from "@charlotte/config";
import type { Config } from "@charlotte/config";
import type { Command } from "commander";

import { readToml, writeToml } from "../toml-io";

type FieldParser<V> = (raw: string) => V;

/** One parser per settable `Config` field, keyed the same way the field
 * appears in `@charlotte/config`'s schema (camelCase) — the CLI's key
 * vocabulary matches the TypeScript config shape directly rather than
 * Christina's Rust field names, since the two do not line up one-to-one
 * (Charlotte merged `max_input_tokens`/`max_output_tokens` into one
 * `maxTokens` field per `03-config-and-profiles.md`'s clamp-policy work).
 * `satisfies` checks this against every `Config` field's real type without
 * widening the literal keys, so an out-of-range enum value below is a
 * compile error rather than a runtime surprise. */
const FIELD_PARSERS = {
  apiKey: (raw) => raw,
  apiUrl: (raw) => raw,
  azureApiVersion: (raw) => raw,
  azureDeploymentId: (raw) => raw,
  commitHistoryDepth: (raw) => Math.trunc(Number(raw)),
  commitMessageMaxLength: (raw) => Math.trunc(Number(raw)),
  // SAFETY: an out-of-range value is rejected downstream by
  // `configOverlaySchema`'s validation in `handleSet`, not by this parser.
  commitValidationMode: (raw) => raw as Config["commitValidationMode"],
  ignorePatterns: (raw) =>
    raw
      .split(",")
      .map((pattern) => pattern.trim())
      .filter((pattern) => pattern.length > 0),
  lockfileTokenLimit: (raw) => Math.trunc(Number(raw)),
  maxConcurrentRequests: (raw) => Math.trunc(Number(raw)),
  maxTokens: (raw) => Math.trunc(Number(raw)),
  model: (raw) => raw,
  partialFailureRate: Number,
  // SAFETY: an out-of-range value is rejected downstream by
  // `configOverlaySchema`'s validation in `handleSet`, not by this parser.
  provider: (raw) => raw as Config["provider"],
  // SAFETY: an out-of-range value is rejected downstream by
  // `configOverlaySchema`'s validation in `handleSet`, not by this parser.
  reasoningEffort: (raw) => raw as Config["reasoningEffort"],
  temperature: Number,
} satisfies { [K in keyof Config]: FieldParser<Config[K]> };

type FieldKey = keyof typeof FIELD_PARSERS;

const isFieldKey = (key: string): key is FieldKey => key in FIELD_PARSERS;

const isSecretLikeKey = (key: string): boolean =>
  key.toLowerCase().includes("key");

const formatValue = (value: Config[FieldKey] | undefined): string => {
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
  if (!isFieldKey(key)) {
    throw new Error(`Unknown configuration key '${key}'`);
  }
  const value = key === "apiKey" ? undefined : config[key];
  if (isSecretLikeKey(key)) {
    console.log(`${key}: <hidden>`);
    return;
  }
  console.log(`${key}: ${formatValue(value)}`);
};

const handleSet = async (key: string, value: string): Promise<void> => {
  if (!isFieldKey(key)) {
    throw new Error(`Unknown configuration key '${key}'`);
  }
  const parsed = FIELD_PARSERS[key](value);
  // `parsed`'s type is one of `Config`'s known field types (via
  // `FIELD_PARSERS`'s `satisfies` clause), not untrusted external input, so
  // there is no schema to parse this against instead.
  // oxlint-disable-next-line anti-slop/no-runtime-typeof
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

  await writeToml(path, result.data);
  console.log(`Set ${key} = ${value}`);
};

const handleList = async (): Promise<void> => {
  const config = await loadConfig();
  console.log("Configuration values:");
  for (const [key, value] of Object.entries(config)) {
    if (isSecretLikeKey(key)) {
      console.log(`  ${key}: <hidden>`);
      continue;
    }
    // SAFETY: `isSecretLikeKey` above already filtered out `apiKey`, the
    // only `ResolvedConfig` field whose value isn't a `Config` field type.
    console.log(`  ${key}: ${formatValue(value as Config[FieldKey])}`);
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
