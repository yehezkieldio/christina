import { createInterface } from "node:readline/promises";

import {
  profilesFilePath,
  profilesSchema,
  providerProfileSchema,
} from "@charlotte/config";
import type { ProviderProfile } from "@charlotte/config";
import type { Command } from "commander";

import { readToml, writeToml } from "../toml-io";

interface ProfileOptions {
  readonly provider?: string;
  readonly model?: string;
  readonly apiKey?: string;
  readonly allowPlaintext?: boolean;
  readonly apiUrl?: string;
  readonly maxTokens?: string;
  readonly lockfileTokenLimit?: string;
  readonly azureApiVersion?: string;
  readonly azureDeploymentId?: string;
}

const loadProfiles = async (): Promise<{
  active: string | null;
  profiles: Record<string, ProviderProfile>;
}> => profilesSchema.parse(await readToml(profilesFilePath()));

const saveProfiles = async (data: {
  active: string | null;
  profiles: Record<string, ProviderProfile>;
}): Promise<void> => {
  await writeToml(
    profilesFilePath(),
    profilesSchema.parse(data) as unknown as Record<string, unknown>
  );
};

const warnPlaintextSecret = (apiKey: string, allowPlaintext: boolean): void => {
  if (!(apiKey.startsWith("env:") || allowPlaintext)) {
    console.error("Warning: storing plaintext API key. Consider env:VAR_NAME.");
  }
};

const applyOverrides = (
  profile: ProviderProfile,
  options: ProfileOptions
): ProviderProfile => {
  const next: Record<string, unknown> = { ...profile };
  if (options.provider !== undefined) {
    next["provider"] = options.provider;
  }
  if (options.model !== undefined) {
    next["model"] = options.model;
  }
  if (options.apiKey !== undefined) {
    warnPlaintextSecret(options.apiKey, options.allowPlaintext ?? false);
    next["apiKey"] = options.apiKey;
  }
  if (options.apiUrl !== undefined) {
    next["apiUrl"] = options.apiUrl;
  }
  if (options.maxTokens !== undefined) {
    next["maxTokens"] = Math.trunc(Number(options.maxTokens));
  }
  if (options.lockfileTokenLimit !== undefined) {
    next["lockfileTokenLimit"] = Math.trunc(Number(options.lockfileTokenLimit));
  }
  if (options.azureApiVersion !== undefined) {
    next["azureApiVersion"] = options.azureApiVersion;
  }
  if (options.azureDeploymentId !== undefined) {
    next["azureDeploymentId"] = options.azureDeploymentId;
  }
  return providerProfileSchema.parse(next);
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
  options: ProfileOptions
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

  warnPlaintextSecret(options.apiKey, options.allowPlaintext ?? false);
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
  options: ProfileOptions
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

const withProfileOptions = (command: Command): Command =>
  command
    .option("--provider <provider>", "Model provider")
    .option("--model <model>", "Model name")
    .option("--api-key <apiKey>", "API key")
    .option(
      "--allow-plaintext",
      "Allow storing plaintext API keys in config",
      false
    )
    .option("--api-url <apiUrl>", "API URL")
    .option("--max-tokens <maxTokens>", "Max tokens")
    .option(
      "--lockfile-token-limit <lockfileTokenLimit>",
      "Lockfile token limit"
    )
    .option("--azure-api-version <azureApiVersion>", "Azure API version")
    .option(
      "--azure-deployment-id <azureDeploymentId>",
      "Azure deployment ID"
    );

export const registerProfileCommand = (program: Command): void => {
  const profile = program.command("profile").description("Profile management");

  profile
    .command("list")
    .description("List all profiles")
    .action(async () => {
      await handleList();
    });

  profile
    .command("show <name>")
    .description("Show profile details")
    .action(async (name: string) => {
      await handleShow(name);
    });

  withProfileOptions(
    profile.command("create <name>").description("Create a new profile")
  ).action(async (name: string, options: ProfileOptions) => {
    await handleCreate(name, options);
  });

  withProfileOptions(
    profile.command("edit <name>").description("Edit a profile")
  ).action(async (name: string, options: ProfileOptions) => {
    await handleEdit(name, options);
  });

  profile
    .command("delete <name>")
    .description("Delete a profile")
    .option("--force", "Skip confirmation", false)
    .action(async (name: string, options: { force: boolean }) => {
      await handleDelete(name, options.force);
    });

  profile
    .command("switch <name>")
    .description("Switch active profile")
    .action(async (name: string) => {
      await handleSwitch(name);
    });

  profile
    .command("duplicate <source> <newName>")
    .description("Duplicate a profile")
    .action(async (source: string, newName: string) => {
      await handleDuplicate(source, newName);
    });
};
