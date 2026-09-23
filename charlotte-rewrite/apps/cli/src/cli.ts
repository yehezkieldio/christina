#!/usr/bin/env bun
import { printError } from "@charlotte/ui";
import { Command } from "commander";
import packageJson from "../package.json" with { type: "json" };
import { registerCompletionsCommand } from "./commands/completions";
import { registerConfigCommand } from "./commands/config";
import { registerProfileCommand } from "./commands/profile";
import { registerStatsCommand } from "./commands/stats";
import { runGenerate } from "./commands/generate";

/** Commander accepts a repeated `-v` but not a stacked `-vvv` the way
 * clap's `ArgAction::Count` does — christina/src/cli/mod.rs relies on that
 * stacking, so expand it before Commander ever sees the token. */
function expandStackedVerbosity(argv: readonly string[]): string[] {
  return argv.flatMap((arg) => (/^-v{2,}$/.test(arg) ? Array.from(arg.slice(1), () => "-v") : [arg]));
}

const program = new Command();

program
  .name("charlotte")
  .description("Automated Conventional Commit Generator Powered By LLMs")
  .version(packageJson.version)
  .option("-v, --verbose", "increase logging verbosity (repeatable)", (_value: string, previous: number) => previous + 1, 0)
  .option("--trace", "enable full pipeline tracing with detailed telemetry output", false)
  .option("--yes", "skip interactive confirmations (non-interactive mode)", false)
  .option("-c, --context <text>", "additional user-provided context appended to prompts")
  .option("--dry-run", "generate commit message without creating the commit (preview mode)", false);

registerConfigCommand(program);
registerProfileCommand(program);
registerStatsCommand(program);
registerCompletionsCommand(program);

/** `isDefault: true` is Commander's documented way to run a subcommand when
 * none is named on the command line (the deprecated `.command('*')` form
 * did the same thing pre-v8.3). Reading options off the closed-over
 * `program` reference, rather than off this action's own `this`, sidesteps
 * any question of whether a subcommand inherits the root's option values. */
program
  .command("generate", { isDefault: true, hidden: true })
  .description("Generate a commit message from staged changes")
  .action(async () => {
    const options = program.opts<{ yes: boolean; trace: boolean; dryRun: boolean; context?: string }>();
    await runGenerate({ yes: options.yes, context: options.context, dryRun: options.dryRun, trace: options.trace });
  });

try {
  await program.parseAsync(expandStackedVerbosity(process.argv));
} catch (error) {
  printError(error instanceof Error ? error.message : String(error));
  process.exit(1);
}
