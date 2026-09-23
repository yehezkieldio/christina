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
