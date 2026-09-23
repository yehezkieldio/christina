import { TOML } from "bun";
import type { z } from "zod";

import { readEnvOverlay } from "./env";
import { configFilePath, profilesFilePath } from "./paths";
import { configOverlaySchema, configSchema, profilesSchema } from "./schema";
import type { Config, ConfigOverlay } from "./schema";
import { resolveSecret } from "./secret";
import type { SecretString } from "./secret";

export class ConfigValidationError extends Error {
  readonly issues: z.core.$ZodIssue[];

  constructor(issues: z.core.$ZodIssue[]) {
    super(
      `config failed validation:\n${issues.map((issue) => `  - ${issue.path.join(".")}: ${issue.message}`).join("\n")}`
    );
    this.name = "ConfigValidationError";
    this.issues = issues;
  }
}

/** `Config` with `apiKey` resolved to a redaction-safe secret. This is the
 * shape every other package should hold onto; only the loader ever sees a
 * raw `env:NAME` reference or a plaintext key. */
export type ResolvedConfig = Omit<Config, "apiKey"> & {
  readonly apiKey: SecretString;
};

export interface LoadConfigOptions {
  readonly profile?: string;
  readonly env?: Readonly<Record<string, string | undefined>>;
  readonly configPath?: string;
  readonly profilesPath?: string;
}

// `readTomlFile`/`parseOverlay` sit at the true I/O boundary: raw parsed TOML
// has no shape yet, and `configOverlaySchema`/`profilesSchema` are what give
// it one a few lines below. A dictionary type here is the boundary itself,
// not a shortcut around it.
// oxlint-disable anti-slop/no-unsafe-dictionary-type
const readTomlFile = async (path: string): Promise<Record<string, unknown>> => {
  const file = Bun.file(path);
  if (!(await file.exists())) {
    return {};
  }
  // SAFETY: `TOML.parse` returns `any`; the result is validated immediately
  // by every caller via `configOverlaySchema`/`profilesSchema`.
  return TOML.parse(await file.text()) as Record<string, unknown>;
};

const parseOverlay = (raw: Record<string, unknown>): ConfigOverlay =>
  configOverlaySchema.parse(raw);
// oxlint-enable anti-slop/no-unsafe-dictionary-type

const deriveActiveProfile = (
  env: Readonly<Record<string, string | undefined>>
): string | undefined => env["CHARLOTTE_PROFILE"];

const readProfileOverlay = async (
  path: string,
  profileName: string
): Promise<ConfigOverlay> => {
  const raw = await readTomlFile(path);
  const parsed = profilesSchema.parse(raw);
  const profile = parsed.profiles[profileName];
  if (!profile) {
    throw new Error(`profile "${profileName}" is not defined in ${path}`);
  }
  return profile;
};

/** Layers a config from lowest to highest precedence: zod schema defaults,
 * the global config file, the active profile overlay, then the
 * `CHARLOTTE_*` environment overlay. Later layers win. */
export const loadConfig = async (
  options: LoadConfigOptions = {}
): Promise<ResolvedConfig> => {
  const env = options.env ?? process.env;
  const fileOverlay = parseOverlay(
    await readTomlFile(options.configPath ?? configFilePath(env))
  );

  const profileName = options.profile ?? deriveActiveProfile(env);
  const profileOverlay = profileName
    ? await readProfileOverlay(
        options.profilesPath ?? profilesFilePath(env),
        profileName
      )
    : {};

  const envOverlay = readEnvOverlay(env);

  const merged: ConfigOverlay = {
    ...fileOverlay,
    ...profileOverlay,
    ...envOverlay,
  };
  const result = configSchema.safeParse(merged);
  if (!result.success) {
    throw new ConfigValidationError(result.error.issues);
  }

  const { value: apiKey } = resolveSecret(result.data.apiKey, env);
  return { ...result.data, apiKey };
};
