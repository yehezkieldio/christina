import { createInterface } from "node:readline/promises";

import {
  profilesFilePath,
  profilesSchema,
  providerProfileSchema,
} from "@charlotte/config";
import type { ProviderProfile } from "@charlotte/config";
import { optional } from "@charlotte/providers";
import {
  argument,
  command,
  constant,
  message,
  object,
  optional as optionalParser,
  option,
  or,
  string,
} from "@optique/core";
import type { InferValue } from "@optique/core";

import { readToml, writeToml } from "../toml-io";

const loadProfiles = async (): Promise<{
  active: string | null;
  profiles: Record<string, ProviderProfile>;
}> => profilesSchema.parse(await readToml(profilesFilePath()));

const saveProfiles = async (data: {
  active: string | null;
  profiles: Record<string, ProviderProfile>;
}): Promise<void> => {
  await writeToml(profilesFilePath(), profilesSchema.parse(data));
};

const warnPlaintextSecret = (apiKey: string, allowPlaintext: boolean): void => {
  if (!(apiKey.startsWith("env:") || allowPlaintext)) {
    console.error("Warning: storing plaintext API key. Consider env:VAR_NAME.");
  }
};

interface ProfileOverrides {
  readonly provider: string | undefined;
  readonly model: string | undefined;
  readonly apiKey: string | undefined;
  readonly allowPlaintext: boolean;
  readonly apiUrl: string | undefined;
  readonly maxTokens: string | undefined;
  readonly lockfileTokenLimit: string | undefined;
  readonly azureApiVersion: string | undefined;
  readonly azureDeploymentId: string | undefined;
}

const applyOverrides = (
  profile: ProviderProfile,
  options: ProfileOverrides
): ProviderProfile => {
  if (options.apiKey !== undefined) {
    warnPlaintextSecret(options.apiKey, options.allowPlaintext);
  }
  return providerProfileSchema.parse({
    ...profile,
    ...optional("provider", options.provider),
    ...optional("model", options.model),
    ...optional("apiKey", options.apiKey),
    ...optional("apiUrl", options.apiUrl),
    ...optional(
      "maxTokens",
      options.maxTokens === undefined
        ? undefined
        : Math.trunc(Number(options.maxTokens))
    ),
    ...optional(
      "lockfileTokenLimit",
      options.lockfileTokenLimit === undefined
        ? undefined
        : Math.trunc(Number(options.lockfileTokenLimit))
    ),
    ...optional("azureApiVersion", options.azureApiVersion),
    ...optional("azureDeploymentId", options.azureDeploymentId),
  });
};

const handleList = async (): Promise<void> => {
  const { active, profiles } = await loadProfiles();
  const names = Object.keys(profiles);
  if (names.length === 0) {
    console.log("No profiles configured.");
    console.log("Use 'charlotte profile create <name>' to create one.");
    return;
  }
  console.log("Profiles:");
  for (const name of names) {
    console.log(`  ${name}${active === name ? " *" : ""}`);
  }
  if (active === null) {
    console.log("\nNo active profile set.");
  }
};

const handleShow = async (name: string): Promise<void> => {
  const { active, profiles } = await loadProfiles();
  const profile = profiles[name];
  if (!profile) {
    throw new Error(`Profile '${name}' not found`);
  }
  console.log(`Profile: ${name}`);
  console.log(`  Provider: ${profile.provider}`);
  console.log(`  Model: ${profile.model}`);
  console.log(
    `  API Key: ${profile.apiKey.startsWith("env:") ? `<${profile.apiKey}>` : "<set>"}`
  );
  console.log(`  API URL: ${profile.apiUrl ?? "<not set>"}`);
  console.log(`  Max Tokens: ${profile.maxTokens ?? "<not set>"}`);
  console.log(
    `  Lockfile Token Limit: ${profile.lockfileTokenLimit ?? "<not set>"}`
  );
  console.log(`  Azure API Version: ${profile.azureApiVersion ?? "<not set>"}`);
  console.log(
    `  Azure Deployment ID: ${profile.azureDeploymentId ?? "<not set>"}`
  );
  if (active === name) {
    console.log("\n  [Active Profile]");
  }
};

const handleCreate = async (
  name: string,
  options: ProfileOverrides
): Promise<void> => {
  if (!(options.provider && options.model && options.apiKey)) {
    throw new Error(
      "--provider, --model, and --api-key are required to create a profile"
    );
  }
  const data = await loadProfiles();
  if (data.profiles[name]) {
    throw new Error(`Profile '${name}' already exists`);
  }

  warnPlaintextSecret(options.apiKey, options.allowPlaintext);
  const profile = providerProfileSchema.parse({
    apiKey: options.apiKey,
    apiUrl: options.apiUrl,
    azureApiVersion: options.azureApiVersion,
    azureDeploymentId: options.azureDeploymentId,
    lockfileTokenLimit: options.lockfileTokenLimit
      ? Math.trunc(Number(options.lockfileTokenLimit))
      : undefined,
    maxTokens: options.maxTokens
      ? Math.trunc(Number(options.maxTokens))
      : undefined,
    model: options.model,
    provider: options.provider,
  });

  data.profiles[name] = profile;
  await saveProfiles(data);
  console.log(`Created profile: ${name}`);
};

const handleEdit = async (
  name: string,
  options: ProfileOverrides
): Promise<void> => {
  const data = await loadProfiles();
  const existing = data.profiles[name];
  if (!existing) {
    throw new Error(`Profile '${name}' not found`);
  }

  data.profiles[name] = applyOverrides(existing, options);
  await saveProfiles(data);
  console.log(`Updated profile: ${name}`);
};

const confirm = async (question: string): Promise<boolean> => {
  const rl = createInterface({ input: process.stdin, output: process.stdout });
  try {
    const answer = await rl.question(question);
    return answer.trim().toLowerCase() === "y";
  } finally {
    rl.close();
  }
};

const handleDelete = async (name: string, force: boolean): Promise<void> => {
  const data = await loadProfiles();
  if (!data.profiles[name]) {
    throw new Error(`Profile '${name}' not found`);
  }

  if (!force) {
    const confirmed = await confirm(`Delete profile '${name}' ? [y/N]: `);
    if (!confirmed) {
      console.log("Cancelled.");
      return;
    }
  }

  const { [name]: _removed, ...remainingProfiles } = data.profiles;
  data.profiles = remainingProfiles;
  if (data.active === name) {
    data.active = null;
  }
  await saveProfiles(data);
  console.log(`Deleted profile: ${name}`);
};

const handleSwitch = async (name: string): Promise<void> => {
  const data = await loadProfiles();
  if (!data.profiles[name]) {
    throw new Error(`Profile '${name}' not found`);
  }
  data.active = name;
  await saveProfiles(data);
  console.log(`Switched to profile: ${name}`);
};

const handleDuplicate = async (
  source: string,
  newName: string
): Promise<void> => {
  const data = await loadProfiles();
  const sourceProfile = data.profiles[source];
  if (!sourceProfile) {
    throw new Error(`Source profile '${source}' not found`);
  }
  if (data.profiles[newName]) {
    throw new Error(`Profile '${newName}' already exists`);
  }
  data.profiles[newName] = { ...sourceProfile };
  await saveProfiles(data);
  console.log(`Duplicated '${source}' to '${newName}'`);
};

/** Shared option fields for `profile create`/`profile edit` — Optique has no
 * imperative command builder to mutate and reuse the way `commander`'s
 * `Command` did, so this is spread into each command's own `object({...})`
 * instead. */
const profileOverrideFields = {
  allowPlaintext: option("--allow-plaintext", {
    description: message`Allow storing plaintext API keys in config`,
  }),
  apiKey: optionalParser(option("--api-key", string())),
  apiUrl: optionalParser(option("--api-url", string())),
  azureApiVersion: optionalParser(option("--azure-api-version", string())),
  azureDeploymentId: optionalParser(option("--azure-deployment-id", string())),
  lockfileTokenLimit: optionalParser(
    option("--lockfile-token-limit", string())
  ),
  maxTokens: optionalParser(option("--max-tokens", string())),
  model: optionalParser(option("--model", string())),
  provider: optionalParser(option("--provider", string())),
};

export const profileParser = command(
  "profile",
  object({
    action: or(
      command("list", object({ action: constant("list") }), {
        description: message`List all profiles`,
      }),
      command(
        "show",
        object({
          action: constant("show"),
          name: argument(string({ metavar: "NAME" })),
        }),
        { description: message`Show profile details` }
      ),
      command(
        "create",
        object({
          action: constant("create"),
          name: argument(string({ metavar: "NAME" })),
          ...profileOverrideFields,
        }),
        { description: message`Create a new profile` }
      ),
      command(
        "edit",
        object({
          action: constant("edit"),
          name: argument(string({ metavar: "NAME" })),
          ...profileOverrideFields,
        }),
        { description: message`Edit a profile` }
      ),
      command(
        "delete",
        object({
          action: constant("delete"),
          force: option("--force", {
            description: message`Skip confirmation`,
          }),
          name: argument(string({ metavar: "NAME" })),
        }),
        { description: message`Delete a profile` }
      ),
      command(
        "switch",
        object({
          action: constant("switch"),
          name: argument(string({ metavar: "NAME" })),
        }),
        { description: message`Switch active profile` }
      ),
      command(
        "duplicate",
        // Field order here is the CLI's positional order (SOURCE before
        // NEW_NAME) — alphabetizing these two `argument()` fields would
        // silently swap what each positional token binds to.
        // oxlint-disable-next-line sort-keys
        object({
          action: constant("duplicate"),
          source: argument(string({ metavar: "SOURCE" })),
          newName: argument(string({ metavar: "NEW_NAME" })),
        }),
        { description: message`Duplicate a profile` }
      )
    ),
    group: constant("profile"),
  }),
  { description: message`Profile management` }
);

export type ProfileAction = InferValue<typeof profileParser>["action"];

export const runProfileAction = async (
  action: ProfileAction
): Promise<void> => {
  switch (action.action) {
    case "list": {
      await handleList();
      return;
    }
    case "show": {
      await handleShow(action.name);
      return;
    }
    case "create": {
      await handleCreate(action.name, action);
      return;
    }
    case "edit": {
      await handleEdit(action.name, action);
      return;
    }
    case "delete": {
      await handleDelete(action.name, action.force);
      return;
    }
    case "switch": {
      await handleSwitch(action.name);
      return;
    }
    case "duplicate": {
      await handleDuplicate(action.source, action.newName);
      return;
    }
    default: {
      action satisfies never;
    }
  }
};
