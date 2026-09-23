/**
 * A secret value that never leaks into `console.log`, `JSON.stringify`, or a
 * template literal by accident. `reveal()` is the one deliberate escape
 * hatch; every other access path prints the redaction marker.
 */
export class SecretString {
  static readonly REDACTED = "[redacted]" as const;

  readonly #value: string;

  private constructor(value: string) {
    this.#value = value;
  }

  static of(value: string): SecretString {
    return new SecretString(value);
  }

  reveal(): string {
    return this.#value;
  }

  toString(): string {
    return SecretString.REDACTED;
  }

  toJSON(): string {
    return SecretString.REDACTED;
  }

  [Symbol.for("nodejs.util.inspect.custom")](): string {
    return SecretString.REDACTED;
  }
}

/**
 * A secret config value is either a literal string or an `env:NAME`
 * reference resolved at load time. The template literal type narrows the
 * `env:` case without a regex at every call site.
 */
export type SecretRef = `env:${string}`;

export type RawSecret = string | SecretRef;

export type ResolvedSecret =
  | { readonly kind: "literal"; readonly value: SecretString }
  | {
      readonly kind: "env";
      readonly name: string;
      readonly value: SecretString;
    };

const isSecretRef = (raw: string): raw is SecretRef => {
  return raw.startsWith("env:") && raw.length > "env:".length;
};

export class MissingSecretEnvVarError extends Error {
  readonly variableName: string;

  constructor(variableName: string) {
    super(
      `environment variable "${variableName}" is not set for a secret reference`
    );
    this.name = "MissingSecretEnvVarError";
    this.variableName = variableName;
  }
}

/**
 * Resolve a raw config value into a redaction-safe secret. `env` defaults to
 * `process.env` and is only a parameter so tests can supply a fixed map
 * instead of mutating the real environment.
 */
export const resolveSecret = (
  raw: RawSecret,
  env: Readonly<Record<string, string | undefined>> = process.env
): ResolvedSecret => {
  if (isSecretRef(raw)) {
    const name = raw.slice("env:".length);
    const value = env[name];
    if (value === undefined) {
      throw new MissingSecretEnvVarError(name);
    }
    return { kind: "env", name, value: SecretString.of(value) };
  }
  return { kind: "literal", value: SecretString.of(raw) };
};
