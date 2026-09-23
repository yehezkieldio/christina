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
  for (let i = 0; i < key.length; i += 1) {
    // SAFETY: `i` is bounded by `key.length` in the loop condition above.
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
): EnvKeyOf<K> =>
  // SAFETY: `toEnvSuffix` is the runtime mirror of the `SplitCamel` type,
  // so its output is exactly `EnvKeyOf<K>`'s suffix by construction.
  `CHARLOTTE_${toEnvSuffix(key)}` as EnvKeyOf<K>;

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
  // SAFETY: an out-of-range value is rejected downstream by
  // `configOverlaySchema`'s validation, not by this parser.
  commitValidationMode: (raw) => raw as Config["commitValidationMode"],
  ignorePatterns: (raw) => raw.split(",").map((pattern) => pattern.trim()),
  lockfileTokenLimit: (raw) => Math.trunc(Number(raw)),
  maxConcurrentRequests: (raw) => Math.trunc(Number(raw)),
  maxTokens: (raw) => Math.trunc(Number(raw)),
  model: (raw) => raw,
  partialFailureRate: Number,
  // SAFETY: an out-of-range value is rejected downstream by
  // `configOverlaySchema`'s validation, not by this parser.
  provider: (raw) => raw as Config["provider"],
  // SAFETY: an out-of-range value is rejected downstream by
  // `configOverlaySchema`'s validation, not by this parser.
  reasoningEffort: (raw) => raw as Config["reasoningEffort"],
  temperature: Number,
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
  // `Config`'s fields have unrelated value types, so building a `Partial`
  // from a dynamic key/parser pair inherently needs one boundary cast per
  // field; `ENV_PARSERS`'s own `satisfies` clause is what keeps each parser
  // honest against its field's real type.
  // oxlint-disable anti-slop/no-known-value-widening
  // oxlint-disable anti-slop/no-unsafe-dictionary-type
  const overlay: Record<string, unknown> = {};
  // SAFETY: `ENV_PARSERS`'s `satisfies` clause proves its keys are exactly
  // `keyof Config`; `Object.keys` only widens that to `string[]` at the
  // type level.
  for (const key of Object.keys(ENV_PARSERS) as (keyof Config)[]) {
    const raw = env[toEnvKey(key)];
    if (raw === undefined) {
      continue;
    }
    // SAFETY: `key` is one of `ENV_PARSERS`'s own keys, so `ENV_PARSERS[key]`
    // is always defined; the parser's specific input/output field types are
    // erased here only to let one loop iterate over all of them.
    const parse = ENV_PARSERS[key] as EnvParser<unknown>;
    overlay[key] = parse(raw);
  }
  // oxlint-enable anti-slop/no-unsafe-dictionary-type
  // oxlint-enable anti-slop/no-known-value-widening
  // SAFETY: every assignment above went through a parser typed against its
  // own `Config` field via `ENV_PARSERS`'s `satisfies` clause.
  return overlay as Partial<Config>;
};
