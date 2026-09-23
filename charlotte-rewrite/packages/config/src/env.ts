import type { Config } from "./schema";

/**
 * Type-level camelCase -> SNAKE_CASE, so `EnvKeyOf<"apiKey">` is the literal
 * type `"CHARLOTTE_API_KEY"` and a typo in a hand-written env key map fails
 * to typecheck instead of failing silently at runtime.
 */
type SplitCamel<S extends string> = S extends `${infer Head}${infer Rest}`
  ? Head extends Uppercase<Head>
    ? Head extends Lowercase<Head>
      ? `${Head}${SplitCamel<Rest>}`
      : `_${Head}${SplitCamel<Rest>}`
    : `${Uppercase<Head>}${SplitCamel<Rest>}`
  : S;

export type EnvKeyOf<K extends string> = `CHARLOTTE_${SplitCamel<K>}`;

/** Runtime mirror of `SplitCamel`. A hand-rolled loop instead of a regex:
 * one pass over the string, no backtracking, no intermediate match array. */
const toEnvSuffix = (key: string): string => {
  let out = "";
  for (let i = 0; i < key.length; i++) {
    const ch = key[i] as string;
    const upper = ch.toUpperCase();
    if (ch === upper && ch !== ch.toLowerCase()) {
      out += i === 0 ? upper : `_${upper}`;
    } else {
      out += upper;
    }
  }
  return out;
};

export const toEnvKey = <K extends keyof Config & string>(
  key: K
): EnvKeyOf<K> => {
  return `CHARLOTTE_${toEnvSuffix(key)}` as EnvKeyOf<K>;
};

type EnvParser<V> = (raw: string) => V;

/**
 * One parser per field that can come from the environment. `satisfies`
 * checks this object against `Partial<{ [K in keyof Config]: EnvParser }>`
 * without widening the literal keys, so `ENV_PARSERS.model` still narrows to
 * `EnvParser<string> | undefined` at the call site.
 */
const ENV_PARSERS = {
  apiKey: (raw) => raw,
  apiUrl: (raw) => raw,
  azureApiVersion: (raw) => raw,
  azureDeploymentId: (raw) => raw,
  commitHistoryDepth: (raw) => Math.trunc(Number(raw)),
  commitMessageMaxLength: (raw) => Math.trunc(Number(raw)),
  commitValidationMode: (raw) => raw as Config["commitValidationMode"],
  ignorePatterns: (raw) => raw.split(",").map((pattern) => pattern.trim()),
  lockfileTokenLimit: (raw) => Math.trunc(Number(raw)),
  maxConcurrentRequests: (raw) => Math.trunc(Number(raw)),
  maxTokens: (raw) => Math.trunc(Number(raw)),
  model: (raw) => raw,
  partialFailureRate: (raw) => Number(raw),
  provider: (raw) => raw as Config["provider"],
  reasoningEffort: (raw) => raw as Config["reasoningEffort"],
  temperature: (raw) => Number.parseFloat(raw),
} satisfies { [K in keyof Config]: EnvParser<Config[K]> };

/**
 * Read every `CHARLOTTE_*` variable that has a matching config field. `env`
 * defaults to `process.env`, and is a parameter for the same reason
 * `resolveSecret` takes one: deterministic tests without mutating the real
 * environment.
 */
export const readEnvOverlay = (
  env: Readonly<Record<string, string | undefined>> = process.env
): Partial<Config> => {
  const overlay: Partial<Config> = {};
  for (const key of Object.keys(ENV_PARSERS) as (keyof Config)[]) {
    const raw = env[toEnvKey(key)];
    if (raw === undefined) {
      continue;
    }
    const parse = ENV_PARSERS[key] as EnvParser<unknown>;
    (overlay as Record<string, unknown>)[key] = parse(raw);
  }
  return overlay;
};
